use crate::client::SearchOutcome;
use crate::model::Paper;
use anyhow::Result;

/// Print one search's results (single-query mode). JSON stays a flat array of
/// papers so existing consumers keep working.
pub fn print_list(papers: &[Paper], json: bool, ids_only: bool) -> Result<()> {
    if ids_only {
        for p in papers {
            println!("{}", p.id);
        }
        return Ok(());
    }
    if json {
        println!("{}", serde_json::to_string_pretty(papers)?);
        return Ok(());
    }
    if papers.is_empty() {
        eprintln!("no results");
        return Ok(());
    }
    for (i, p) in papers.iter().enumerate() {
        if i > 0 {
            println!();
        }
        print_entry(p);
    }
    Ok(())
}

/// Print several searches at once (multi-query mode). JSON is an array of
/// `{query, fallback, results}` objects, one per query, so agents can run a
/// batch in a single process without shell loops.
pub fn print_multi(sets: &[(String, SearchOutcome)], json: bool, ids_only: bool) -> Result<()> {
    if ids_only {
        for (_, outcome) in sets {
            for p in &outcome.papers {
                println!("{}", p.id);
            }
        }
        return Ok(());
    }
    if json {
        let arr: Vec<_> = sets
            .iter()
            .map(|(query, outcome)| {
                serde_json::json!({
                    "query": query,
                    "fallback": outcome.fallback,
                    "results": &outcome.papers,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
        return Ok(());
    }
    for (i, (query, outcome)) in sets.iter().enumerate() {
        if i > 0 {
            println!();
        }
        let label = if query.is_empty() { "(filters only)" } else { query };
        println!("== {label} ==");
        if outcome.papers.is_empty() {
            println!("  no results");
            continue;
        }
        for p in &outcome.papers {
            println!();
            print_entry(p);
        }
    }
    Ok(())
}

fn print_entry(p: &Paper) {
    println!("{}  [{}]", p.id, p.primary_category);
    println!("  {}", p.title);
    println!("  {}", format_authors(&p.authors, 4));
    println!("  {}  {}", date_only(&p.published), p.abs_url);
}

pub fn print_full(papers: &[Paper], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(papers)?);
        return Ok(());
    }
    for (i, p) in papers.iter().enumerate() {
        if i > 0 {
            println!("\n---\n");
        }
        println!("id:        {}", p.id);
        println!("title:     {}", p.title);
        println!("authors:   {}", p.authors.join(", "));
        println!("category:  {} ({})", p.primary_category, p.categories.join(", "));
        println!("published: {}", date_only(&p.published));
        println!("updated:   {}", date_only(&p.updated));
        if let Some(c) = &p.comment {
            println!("comment:   {c}");
        }
        if let Some(d) = &p.doi {
            println!("doi:       {d}");
        }
        if let Some(j) = &p.journal_ref {
            println!("journal:   {j}");
        }
        println!("abs:       {}", p.abs_url);
        println!("pdf:       {}", p.pdf_url);
        println!("\n{}", wrap(&p.summary, 88));
    }
    Ok(())
}

pub fn print_summary(p: &Paper, json: bool) -> Result<()> {
    if json {
        let obj = serde_json::json!({
            "id": p.id,
            "title": p.title,
            "summary": p.summary,
        });
        println!("{}", serde_json::to_string_pretty(&obj)?);
        return Ok(());
    }
    println!("{}\n", p.title);
    println!("{}", wrap(&p.summary, 88));
    Ok(())
}

fn format_authors(authors: &[String], max: usize) -> String {
    if authors.len() <= max {
        authors.join(", ")
    } else {
        format!("{} et al. ({} authors)", authors[..max].join(", "), authors.len())
    }
}

fn date_only(ts: &str) -> &str {
    ts.split('T').next().unwrap_or(ts)
}

fn wrap(text: &str, width: usize) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut line_len = 0;
    for word in text.split_whitespace() {
        let wlen = word.chars().count();
        if line_len > 0 && line_len + 1 + wlen > width {
            out.push('\n');
            line_len = 0;
        } else if line_len > 0 {
            out.push(' ');
            line_len += 1;
        }
        out.push_str(word);
        line_len += wlen;
    }
    out
}
