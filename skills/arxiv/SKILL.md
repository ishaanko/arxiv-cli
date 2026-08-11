---
name: arxiv
description: Search arXiv, fetch paper metadata/abstracts, download PDFs, and get BibTeX citations using the local `arxiv` CLI. Use whenever the task involves arXiv papers, paper IDs (e.g. 1706.03762, 2301.10945), arxiv.org URLs, finding/summarizing research papers, or recent submissions in a category. ALWAYS prefer this over WebFetch/WebSearch for arxiv.org — it is faster, returns structured JSON, and avoids scraping HTML.
---

# arXiv CLI

Use the `arxiv` binary (Homebrew-installed, on PATH) for all arXiv interactions instead of WebFetch on arxiv.org URLs. It talks to the official arXiv API and emits clean, parseable output.

If `arxiv` is not on PATH, install with:
`brew tap ishaanko/tap https://github.com/ishaanko/arxiv-cli && brew install arxiv`
(or fall back to WebFetch if Homebrew is unavailable).

## Commands

All ID arguments accept bare IDs (`1706.03762`, `2301.10945v2`, `cs/0301012`), `arXiv:`-prefixed IDs, and any arxiv.org URL (abs/pdf/html) interchangeably.

```sh
# Search — multi-word queries are phrase-matched; use --json for structured output
arxiv search "diffusion models" --max 10 --json
arxiv search "test-time compute" --category cs.LG --sort date --json
arxiv search --author "Yoshua Bengio" --title "generative" --json

# Advanced arXiv query syntax passes through untouched
arxiv search 'ti:"mamba" AND cat:cs.LG' --json

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

- Always pass `--json` when you will parse the output (`search`, `get`, `summary`, `latest`). JSON fields: `id`, `title`, `authors[]`, `summary`, `published`, `updated`, `categories[]`, `primary_category`, `abs_url`, `pdf_url`, plus `comment`/`doi`/`journal_ref` when present.
- `--ids-only` on `search`/`latest` prints one ID per line for piping into `xargs arxiv get --json`.
- Exit code 0 = success, 1 = failure; errors go to stderr, stdout stays parseable.
- Sort options: `--sort relevance` (default) | `date` | `updated`. Paginate with `--max` and `--start`.
- To read a paper's full text, download with `arxiv pdf` and read the file — do not WebFetch arxiv.org PDF URLs.
- The arXiv API asks clients to keep request rates modest; batch lookups into one call (`arxiv get id1 id2 id3 --json`) instead of one call per ID.
