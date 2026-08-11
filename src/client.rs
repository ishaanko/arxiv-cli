use crate::model::Paper;
use anyhow::{bail, Context, Result};
use std::io::Read;
use std::path::{Path, PathBuf};

const API: &str = "https://export.arxiv.org/api/query";
const USER_AGENT: &str = concat!("arxiv-cli/", env!("CARGO_PKG_VERSION"));

pub struct SearchQuery {
    pub terms: String,
    pub category: Option<String>,
    pub author: Option<String>,
    pub title: Option<String>,
    pub abstract_: Option<String>,
    pub max: usize,
    pub start: usize,
    pub sort: &'static str,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .user_agent(USER_AGENT)
        .redirects(4)
        .build()
}

/// GET with automatic retry on rate limiting (429) and transient
/// unavailability (5xx). arXiv's API sheds load with 503s and asks clients
/// to back off; honor Retry-After when present, else use a growing delay.
fn get_with_retry(url: &str) -> Result<ureq::Response> {
    const MAX_ATTEMPTS: u32 = 4;
    const BACKOFF_SECS: [u64; 3] = [5, 15, 30];
    let agent = agent();
    let mut attempt = 0;
    loop {
        attempt += 1;
        let err = match agent.get(url).call() {
            Ok(resp) => return Ok(resp),
            Err(e) => e,
        };
        let (retryable, retry_after) = match &err {
            ureq::Error::Status(code, resp) if *code == 429 || *code >= 500 => (
                true,
                resp.header("Retry-After").and_then(|v| v.parse::<u64>().ok()),
            ),
            ureq::Error::Transport(_) => (true, None),
            _ => (false, None),
        };
        if !retryable || attempt >= MAX_ATTEMPTS {
            return Err(err).with_context(|| format!("request failed: {url}"));
        }
        // arXiv sends "Retry-After: 0" on 503s; retrying instantly just
        // escalates to a 429, so treat the header as a floor-raiser only
        // and never wait less than our own schedule.
        let delay = retry_after
            .unwrap_or(0)
            .max(BACKOFF_SECS[(attempt - 1) as usize % BACKOFF_SECS.len()])
            .min(120);
        eprintln!("arxiv: {err}; retrying in {delay}s (attempt {}/{MAX_ATTEMPTS})", attempt + 1);
        std::thread::sleep(std::time::Duration::from_secs(delay));
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn build_search_expr(q: &SearchQuery) -> String {
    let mut parts: Vec<String> = Vec::new();
    let terms = q.terms.trim();
    if !terms.is_empty() {
        // Quote multi-word queries as a phrase unless the user is already
        // using arXiv query syntax (field prefixes or boolean operators).
        let has_syntax = terms.contains(':')
            || terms.contains(" AND ")
            || terms.contains(" OR ")
            || terms.contains(" ANDNOT ");
        if has_syntax {
            parts.push(terms.to_string());
        } else {
            parts.push(format!("all:\"{terms}\""));
        }
    }
    if let Some(c) = &q.category {
        parts.push(format!("cat:{c}"));
    }
    if let Some(a) = &q.author {
        parts.push(format!("au:\"{a}\""));
    }
    if let Some(t) = &q.title {
        parts.push(format!("ti:\"{t}\""));
    }
    if let Some(ab) = &q.abstract_ {
        parts.push(format!("abs:\"{ab}\""));
    }
    parts.join(" AND ")
}

pub fn search(q: &SearchQuery) -> Result<Vec<Paper>> {
    let expr = build_search_expr(q);
    if expr.is_empty() {
        bail!("empty query: provide search terms or a filter (--category, --author, --title, --abstract)");
    }
    let url = format!(
        "{API}?search_query={}&start={}&max_results={}&sortBy={}&sortOrder=descending",
        urlencode(&expr),
        q.start,
        q.max,
        q.sort
    );
    fetch_feed(&url)
}

pub fn by_ids(ids: &[String]) -> Result<Vec<Paper>> {
    let url = format!(
        "{API}?id_list={}&max_results={}",
        urlencode(&ids.join(",")),
        ids.len()
    );
    let papers = fetch_feed(&url)?;
    if papers.is_empty() {
        bail!("no papers found for id(s): {}", ids.join(", "));
    }
    Ok(papers)
}

fn fetch_feed(url: &str) -> Result<Vec<Paper>> {
    let body = get_with_retry(url)
        .context("arXiv API request failed")?
        .into_string()
        .context("failed to read arXiv API response")?;
    parse_feed(&body)
}

fn parse_feed(xml: &str) -> Result<Vec<Paper>> {
    let doc = roxmltree::Document::parse(xml).context("failed to parse arXiv API response XML")?;
    let root = doc.root_element();

    let mut papers = Vec::new();
    for entry in root.children().filter(|n| n.has_tag_name("entry")) {
        let text = |tag: &str| -> String {
            entry
                .children()
                .find(|n| n.has_tag_name(tag))
                .and_then(|n| n.text())
                .map(clean_ws)
                .unwrap_or_default()
        };

        let raw_id = text("id");
        // Entry id is a URL like http://arxiv.org/abs/1706.03762v7
        let id = raw_id.rsplit("/abs/").next().unwrap_or(&raw_id).to_string();
        if id.is_empty() {
            continue;
        }
        // The API returns a stub entry with an error link when an id_list
        // item is malformed; those have an empty title and no authors.
        let title = text("title");
        if title.is_empty() && raw_id.contains("api/errors") {
            let msg = text("summary");
            bail!("arXiv API error: {}", if msg.is_empty() { raw_id } else { msg });
        }

        let authors = entry
            .children()
            .filter(|n| n.has_tag_name("author"))
            .filter_map(|a| {
                a.children()
                    .find(|n| n.has_tag_name("name"))
                    .and_then(|n| n.text())
                    .map(clean_ws)
            })
            .collect();

        let categories = entry
            .children()
            .filter(|n| n.has_tag_name("category"))
            .filter_map(|c| c.attribute("term"))
            .map(str::to_string)
            .collect();

        let primary_category = entry
            .children()
            .find(|n| n.has_tag_name("primary_category"))
            .and_then(|n| n.attribute("term"))
            .unwrap_or_default()
            .to_string();

        let opt = |tag: &str| -> Option<String> {
            let v = text(tag);
            (!v.is_empty()).then_some(v)
        };

        let base_id = id.clone();
        papers.push(Paper {
            abs_url: format!("https://arxiv.org/abs/{base_id}"),
            pdf_url: format!("https://arxiv.org/pdf/{base_id}"),
            id,
            title,
            authors,
            summary: text("summary"),
            published: text("published"),
            updated: text("updated"),
            categories,
            primary_category,
            comment: opt("comment"),
            doi: opt("doi"),
            journal_ref: opt("journal_ref"),
        });
    }
    Ok(papers)
}

fn clean_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn download_pdf(id: &str, output: Option<&str>, dir: Option<&str>) -> Result<PathBuf> {
    let url = format!("https://arxiv.org/pdf/{id}");
    let path = match output {
        Some(o) => PathBuf::from(o),
        None => {
            let fname = format!("{}.pdf", id.replace('/', "_"));
            match dir {
                Some(d) => Path::new(d).join(fname),
                None => PathBuf::from(fname),
            }
        }
    };
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }
    }

    let resp = get_with_retry(&url).context("failed to download PDF")?;

    let mut reader = resp.into_reader();
    let mut buf = Vec::with_capacity(1 << 20);
    reader
        .read_to_end(&mut buf)
        .context("failed while reading PDF stream")?;
    if !buf.starts_with(b"%PDF") {
        bail!(
            "response from {url} is not a PDF (paper may not exist or has no PDF version)"
        );
    }
    std::fs::write(&path, &buf)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

pub fn bibtex(id: &str) -> Result<String> {
    let url = format!("https://arxiv.org/bibtex/{id}");
    let body = get_with_retry(&url)
        .context("failed to fetch BibTeX")?
        .into_string()
        .context("failed to read BibTeX response")?;
    if !body.trim_start().starts_with('@') {
        bail!("unexpected BibTeX response for id {id}");
    }
    Ok(body)
}
