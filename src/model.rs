use anyhow::{bail, Result};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Paper {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub summary: String,
    pub published: String,
    pub updated: String,
    pub categories: Vec<String>,
    pub primary_category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal_ref: Option<String>,
    pub abs_url: String,
    pub pdf_url: String,
}

/// Accepts a bare ID ("1706.03762", "2301.10945v2", "cs/0301012"), an
/// "arXiv:"-prefixed ID, or an arxiv.org URL (abs, pdf, or html), and
/// returns the bare ID (version suffix preserved if given).
pub fn normalize_id(input: &str) -> Result<String> {
    let s = input.trim();
    let s = s.strip_prefix("arXiv:").or_else(|| s.strip_prefix("arxiv:")).unwrap_or(s);

    // URL forms
    if let Some(rest) = s
        .strip_prefix("https://arxiv.org/")
        .or_else(|| s.strip_prefix("http://arxiv.org/"))
        .or_else(|| s.strip_prefix("https://www.arxiv.org/"))
        .or_else(|| s.strip_prefix("http://www.arxiv.org/"))
        .or_else(|| s.strip_prefix("https://export.arxiv.org/"))
        .or_else(|| s.strip_prefix("arxiv.org/"))
        .or_else(|| s.strip_prefix("www.arxiv.org/"))
    {
        let rest = rest
            .strip_prefix("abs/")
            .or_else(|| rest.strip_prefix("pdf/"))
            .or_else(|| rest.strip_prefix("html/"))
            .unwrap_or(rest);
        let id = rest
            .trim_end_matches(".pdf")
            .split(['?', '#'])
            .next()
            .unwrap_or("")
            .trim_matches('/');
        if id.is_empty() {
            bail!("could not extract an arXiv ID from URL: {input}");
        }
        return validate(id, input);
    }

    validate(s, input)
}

fn validate(id: &str, original: &str) -> Result<String> {
    // New style: NNNN.NNNNN[vN]; old style: archive[.sub]/NNNNNNN[vN]
    let ok = {
        // Strip a trailing version suffix (vN) if present; archives like
        // "solv-int" legitimately contain 'v', so only strip when the tail
        // after the last 'v' is all digits.
        let base = match id.rsplit_once('v') {
            Some((b, ver)) if !b.is_empty() && !ver.is_empty() && ver.chars().all(|c| c.is_ascii_digit()) => b,
            _ => id,
        };
        if let Some((l, r)) = base.split_once('.') {
            !id.contains('/')
                && l.len() == 4
                && l.chars().all(|c| c.is_ascii_digit())
                && (4..=5).contains(&r.len())
                && r.chars().all(|c| c.is_ascii_digit())
        } else if let Some((archive, num)) = base.split_once('/') {
            !archive.is_empty()
                && archive.chars().all(|c| c.is_ascii_alphabetic() || c == '-' || c == '.')
                && num.len() == 7
                && num.chars().all(|c| c.is_ascii_digit())
        } else {
            false
        }
    };
    if !ok {
        bail!("'{original}' does not look like an arXiv ID or URL (expected e.g. 1706.03762 or cs/0301012)");
    }
    Ok(id.to_string())
}
