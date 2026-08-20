---
name: arxiv
description: Search arXiv, fetch paper metadata/abstracts, download PDFs, and get BibTeX citations using the local `arxiv` CLI. Use whenever the task involves arXiv papers, paper IDs (e.g. 1706.03762, 2301.10945), arxiv.org URLs, finding/summarizing research papers, or recent submissions in a category. ALWAYS prefer this over WebFetch/WebSearch for arxiv.org — it is faster, returns structured JSON, and avoids scraping HTML.
---

# arXiv CLI

Use the `arxiv` binary (on PATH) for all arXiv interactions instead of WebFetch on arxiv.org URLs. It talks to the official arXiv API and emits clean, parseable output.

If `arxiv` is not on PATH, install it:
- macOS/Linux: `brew tap ishaanko/tap https://github.com/ishaanko/arxiv-cli && brew install arxiv`
- Windows: `scoop bucket add ishaanko https://github.com/ishaanko/arxiv-cli; scoop install arxiv`
- Any platform with Rust: `cargo install --git https://github.com/ishaanko/arxiv-cli`

(Fall back to WebFetch only if none of these package managers are available.)

## Commands

All ID arguments accept bare IDs (`1706.03762`, `2301.10945v2`, `cs/0301012`), `arXiv:`-prefixed IDs, and any arxiv.org URL (abs/pdf/html) interchangeably.

```sh
# Search — use short, title-like queries in quotes; --json for structured output
arxiv search "diffusion models" --max 10 --json
arxiv search "test-time compute" --category cs.LG --sort date --json
arxiv search --author "Yoshua Bengio" --title "generative" --json

# Each argument is a separate query. Batch several in one paced process
# instead of calling arxiv in a shell loop.
arxiv search "mixture of experts" "state space models" --json

# Advanced arXiv query syntax and exact phrases pass through untouched
arxiv search 'ti:"mamba" AND cat:cs.LG' --json
arxiv search 'all:"chain of thought"' --json

# Full metadata (title, authors, abstract, categories, dates, DOI, links)
arxiv get 1706.03762 --json
arxiv get https://arxiv.org/abs/2301.10945 arXiv:2104.04692v3 --json

# Abstract only
arxiv summary 1706.03762 --json

# Download PDF — prints exactly one line on success: the saved file path
arxiv pdf 1706.03762 -o paper.pdf
arxiv pdf https://arxiv.org/abs/1706.03762 --dir /tmp

# Official BibTeX citation
arxiv bibtex 1706.03762

# Most recent submissions in a category
arxiv latest cs.CL --max 10 --json
```

## Agent conventions

- Always pass `--json` when you will parse the output (`search`, `get`, `summary`, `latest`). Paper JSON fields: `id`, `title`, `authors[]`, `summary`, `published`, `updated`, `categories[]`, `primary_category`, `abs_url`, `pdf_url`, plus `comment`/`doi`/`journal_ref` when present.
- Search JSON shape: one query returns a flat array of papers; several queries return an array of `{query, fallback, results}` objects, one per query.
- Search matches with AND (every word must appear). A multi-word query that finds nothing is retried once with the terms ORed by relevance; `fallback: true` (or a stderr note) flags this. Pass `--strict` to force pure AND.
- `--ids-only` on `search`/`latest` prints one ID per line for piping into `xargs arxiv get --json`.
- Exit code 0 = success, nonzero = failure; the error goes to stderr as one line. With `--json`, stdout is always valid JSON even on failure (`{"error": "..."}`) and never empty.
- Requests are paced (3 s apart). Each API attempt has a 10 s whole-call deadline (`--timeout` to change); PDF downloads use it as a per-read stall timeout so large files are not cut off. Failed requests retry with backoff, so total time can exceed one timeout. Batch queries and IDs into one call rather than looping.
- Sort options: `--sort relevance` (default) | `date` | `updated`. Paginate with `--max` and `--start`.
- To read a paper's full text, download with `arxiv pdf` and read the file — do not WebFetch arxiv.org PDF URLs.
- The arXiv API asks clients to keep request rates modest; batch lookups into one call (`arxiv get id1 id2 id3 --json`) instead of one call per ID.
