use crate::model::Paper;
use anyhow::{bail, Context, Result};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const API: &str = "https://export.arxiv.org/api/query";
const USER_AGENT: &str = concat!("arxiv-cli/", env!("CARGO_PKG_VERSION"));

/// arXiv asks clients to leave at least 3 seconds between requests. We enforce
/// it process-wide so a burst of commands in one run never trips the throttle.
const MIN_REQUEST_GAP: Duration = Duration::from_secs(3);

/// Hard per-request timeout in seconds; set once from the `--timeout` flag.
static TIMEOUT_SECS: AtomicU64 = AtomicU64::new(10);

/// Start time of the most recent request, used to pace the next one.
static LAST_REQUEST: Mutex<Option<Instant>> = Mutex::new(None);

/// Set the hard per-request timeout (seconds). A value of 0 is clamped to 1.
pub fn set_timeout(secs: u64) {
    TIMEOUT_SECS.store(secs.max(1), Ordering::Relaxed);
}

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

/// Result of a search, including whether the relaxed OR fallback ran.
pub struct SearchOutcome {
    pub papers: Vec<Paper>,
    pub fallback: bool,
}

/// How long to wait before the next request, given the last request time and
/// the current instant. Pure so it can be tested without real sleeps.
fn gap_delay(last: Option<Instant>, now: Instant, min_gap: Duration) -> Duration {
    match last {
        Some(l) => {
            let elapsed = now.saturating_duration_since(l);
            min_gap.saturating_sub(elapsed)
        }
        None => Duration::ZERO,
    }
}

/// Block until at least `MIN_REQUEST_GAP` has passed since the last request,
/// then record now as the start of this one. The lock is held across the sleep
/// so concurrent callers serialize behind the same gap.
fn pace() {
    let mut guard = LAST_REQUEST.lock().unwrap();
    let delay = gap_delay(*guard, Instant::now(), MIN_REQUEST_GAP);
    if !delay.is_zero() {
        std::thread::sleep(delay);
    }
    *guard = Some(Instant::now());
}

/// Build an agent for the given transfer size.
///
/// `Small` API responses get an overall deadline (`.timeout()` covers connect,
/// reads, and the whole body), so a slow trickle can never run past `--timeout`.
/// A `Large` PDF instead gets a per-read stall timeout: a steady download of a
/// big file must not be killed at the deadline, but a stalled socket still is.
fn agent(size: Transfer) -> ureq::Agent {
    let t = Duration::from_secs(TIMEOUT_SECS.load(Ordering::Relaxed));
    let builder = ureq::AgentBuilder::new()
        .user_agent(USER_AGENT)
        .timeout_connect(t)
        .redirects(4);
    let builder = match size {
        Transfer::Small => builder.timeout(t),
        Transfer::Large => builder.timeout_read(t).timeout_write(t),
    };
    builder.build()
}

#[derive(Clone, Copy)]
enum Transfer {
    /// A small API or text response; bounded by an overall deadline.
    Small,
    /// A large body (a PDF); bounded by a per-read stall timeout.
    Large,
}

/// GET the URL and return the raw body, retrying on rate limiting (429),
/// transient unavailability (5xx), transport errors (including timeouts), and
/// empty bodies. arXiv sheds load with 503s and sometimes returns an empty
/// body under stress; both must be retried or callers get "Expecting value"
/// from a downstream JSON parse. Every attempt is paced by `MIN_REQUEST_GAP`.
fn request_bytes(url: &str, size: Transfer) -> Result<Vec<u8>> {
    const MAX_ATTEMPTS: u32 = 4;
    const BACKOFF_SECS: [u64; 3] = [5, 15, 30];
    let mut attempt = 0;
    loop {
        attempt += 1;
        pace();

        let retry_after: Option<u64>;
        let reason: String;
        match agent(size).get(url).call() {
            Ok(resp) => {
                let mut buf = Vec::with_capacity(1 << 16);
                match resp.into_reader().read_to_end(&mut buf) {
                    Ok(_) if !buf.is_empty() => return Ok(buf),
                    Ok(_) => {
                        retry_after = None;
                        reason = "empty response body".to_string();
                    }
                    Err(e) => {
                        retry_after = None;
                        reason = format!("read error: {e}");
                    }
                }
            }
            Err(ureq::Error::Status(code, resp)) if code == 429 || code >= 500 => {
                retry_after = resp.header("Retry-After").and_then(|v| v.parse::<u64>().ok());
                reason = format!("HTTP {code}");
            }
            // A non-retryable HTTP status (e.g. 400/404): fail immediately.
            Err(e @ ureq::Error::Status(..)) => {
                return Err(anyhow::Error::new(e)).with_context(|| format!("request failed: {url}"));
            }
            // Transport error, which includes connect/read timeouts.
            Err(e) => {
                retry_after = None;
                reason = e.to_string();
            }
        }

        if attempt >= MAX_ATTEMPTS {
            bail!("request failed after {MAX_ATTEMPTS} attempts ({reason}): {url}");
        }
        // arXiv sends "Retry-After: 0" on 503s; retrying instantly just
        // escalates to a 429, so treat the header as a floor-raiser only and
        // never wait less than our own schedule.
        let delay = retry_after
            .unwrap_or(0)
            .max(BACKOFF_SECS[(attempt - 1) as usize % BACKOFF_SECS.len()])
            .min(120);
        eprintln!("arxiv: {reason}; retrying in {delay}s (attempt {}/{MAX_ATTEMPTS})", attempt + 1);
        std::thread::sleep(Duration::from_secs(delay));
    }
}

fn request_string(url: &str) -> Result<String> {
    let bytes = request_bytes(url, Transfer::Small)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
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

/// Build the `search_query` expression. Returns the expression plus two flags:
/// whether the free-text query has more than one term (so an OR fallback could
/// help) and whether it uses raw arXiv syntax (field prefixes or booleans, in
/// which case we pass it through untouched and never rewrite it).
///
/// Free-text terms are ANDed by default (every word must appear somewhere). In
/// `relaxed` mode they are ORed instead, which is the zero-result fallback.
fn build_search_expr(q: &SearchQuery, relaxed: bool) -> (String, bool, bool) {
    let mut parts: Vec<String> = Vec::new();
    let terms = q.terms.trim();
    let mut multi_term = false;
    let mut has_syntax = false;

    if !terms.is_empty() {
        has_syntax = terms.contains(':')
            || terms.contains(" AND ")
            || terms.contains(" OR ")
            || terms.contains(" ANDNOT ");
        if has_syntax {
            // Raw arXiv query syntax: hand it to the API as written.
            parts.push(terms.to_string());
        } else {
            let words: Vec<&str> = terms.split_whitespace().collect();
            multi_term = words.len() > 1;
            let joiner = if relaxed { " OR " } else { " AND " };
            let clause = words
                .iter()
                .map(|w| format!("all:\"{w}\""))
                .collect::<Vec<_>>()
                .join(joiner);
            // Parenthesize a multi-word clause so it stays intact when ANDed
            // with the field filters below.
            parts.push(if multi_term { format!("({clause})") } else { clause });
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
    (parts.join(" AND "), multi_term, has_syntax)
}

fn search_url(expr: &str, sort: &str, start: usize, max: usize) -> String {
    format!(
        "{API}?search_query={}&start={start}&max_results={max}&sortBy={sort}&sortOrder=descending",
        urlencode(expr)
    )
}

/// Run a search with the AND-then-OR fallback. Unless `strict` is set, a
/// multi-term free-text query that returns nothing is retried with the terms
/// ORed and sorted by relevance. The `fetch` closure is injectable for tests.
fn search_with<F>(q: &SearchQuery, strict: bool, mut fetch: F) -> Result<SearchOutcome>
where
    F: FnMut(&str, &str) -> Result<Vec<Paper>>,
{
    let (expr, multi_term, has_syntax) = build_search_expr(q, false);
    if expr.is_empty() {
        bail!("empty query: provide search terms or a filter (--category, --author, --title, --abstract)");
    }
    let papers = fetch(&expr, q.sort)?;

    // Fall back only when a plain multi-term query came back empty.
    if !papers.is_empty() || strict || !multi_term || has_syntax {
        return Ok(SearchOutcome { papers, fallback: false });
    }
    let (relaxed_expr, _, _) = build_search_expr(q, true);
    let papers = fetch(&relaxed_expr, "relevance")?;
    Ok(SearchOutcome { papers, fallback: true })
}

pub fn search(q: &SearchQuery, strict: bool) -> Result<SearchOutcome> {
    search_with(q, strict, |expr, sort| {
        fetch_feed(&search_url(expr, sort, q.start, q.max))
    })
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
    let body = request_string(url).context("arXiv API request failed")?;
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

    let buf = request_bytes(&url, Transfer::Large).context("failed to download PDF")?;
    if !buf.starts_with(b"%PDF") {
        bail!("response from {url} is not a PDF (paper may not exist or has no PDF version)");
    }
    std::fs::write(&path, &buf)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

pub fn bibtex(id: &str) -> Result<String> {
    let url = format!("https://arxiv.org/bibtex/{id}");
    let body = request_string(&url).context("failed to fetch BibTeX")?;
    if !body.trim_start().starts_with('@') {
        bail!("unexpected BibTeX response for id {id}");
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(terms: &str) -> SearchQuery {
        SearchQuery {
            terms: terms.to_string(),
            category: None,
            author: None,
            title: None,
            abstract_: None,
            max: 10,
            start: 0,
            sort: "relevance",
        }
    }

    fn dummy_paper(id: &str) -> Paper {
        Paper {
            id: id.to_string(),
            title: "t".into(),
            authors: vec![],
            summary: String::new(),
            published: String::new(),
            updated: String::new(),
            categories: vec![],
            primary_category: String::new(),
            comment: None,
            doi: None,
            journal_ref: None,
            abs_url: String::new(),
            pdf_url: String::new(),
        }
    }

    #[test]
    fn multi_term_ands_by_default_and_ors_when_relaxed() {
        let (and_expr, multi, syntax) = build_search_expr(&q("synthetic visual reasoning"), false);
        assert_eq!(and_expr, "(all:\"synthetic\" AND all:\"visual\" AND all:\"reasoning\")");
        assert!(multi);
        assert!(!syntax);

        let (or_expr, _, _) = build_search_expr(&q("synthetic visual reasoning"), true);
        assert_eq!(or_expr, "(all:\"synthetic\" OR all:\"visual\" OR all:\"reasoning\")");
    }

    #[test]
    fn single_term_is_not_multi() {
        let (expr, multi, _) = build_search_expr(&q("mamba"), false);
        assert_eq!(expr, "all:\"mamba\"");
        assert!(!multi);
    }

    #[test]
    fn raw_syntax_passes_through_untouched() {
        let (expr, multi, syntax) = build_search_expr(&q("ti:\"mamba\" AND cat:cs.LG"), false);
        assert_eq!(expr, "ti:\"mamba\" AND cat:cs.LG");
        assert!(syntax);
        assert!(!multi);
    }

    #[test]
    fn zero_results_triggers_or_fallback() {
        let mut calls = Vec::new();
        let outcome = search_with(&q("a b c"), false, |expr, sort| {
            calls.push((expr.to_string(), sort.to_string()));
            if expr.contains(" OR ") {
                Ok(vec![dummy_paper("1234.5678")])
            } else {
                Ok(vec![]) // AND attempt finds nothing
            }
        })
        .unwrap();
        assert!(outcome.fallback);
        assert_eq!(outcome.papers.len(), 1);
        assert_eq!(calls.len(), 2);
        // The fallback is always sorted by relevance.
        assert_eq!(calls[1].1, "relevance");
    }

    #[test]
    fn strict_never_falls_back() {
        let mut calls = 0;
        let outcome = search_with(&q("a b c"), true, |_expr, _sort| {
            calls += 1;
            Ok(vec![])
        })
        .unwrap();
        assert!(!outcome.fallback);
        assert!(outcome.papers.is_empty());
        assert_eq!(calls, 1, "strict search must not retry");
    }

    #[test]
    fn single_term_zero_results_does_not_fall_back() {
        let mut calls = 0;
        let outcome = search_with(&q("mamba"), false, |_expr, _sort| {
            calls += 1;
            Ok(vec![])
        })
        .unwrap();
        assert!(!outcome.fallback);
        assert_eq!(calls, 1);
    }

    #[test]
    fn non_empty_first_attempt_does_not_fall_back() {
        let outcome = search_with(&q("a b c"), false, |_expr, _sort| {
            Ok(vec![dummy_paper("1")])
        })
        .unwrap();
        assert!(!outcome.fallback);
    }

    #[test]
    fn gap_delay_waits_for_the_remaining_gap() {
        let base = Instant::now();
        // Only 1s elapsed of a 3s gap -> wait the remaining 2s.
        let d = gap_delay(Some(base), base + Duration::from_secs(1), Duration::from_secs(3));
        assert_eq!(d, Duration::from_secs(2));
        // Enough time already passed -> no wait.
        let d = gap_delay(Some(base), base + Duration::from_secs(5), Duration::from_secs(3));
        assert_eq!(d, Duration::ZERO);
        // No prior request -> no wait.
        let d = gap_delay(None, base, Duration::from_secs(3));
        assert_eq!(d, Duration::ZERO);
    }
}
