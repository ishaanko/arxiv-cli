# Changelog

## Unreleased

## 0.1.2 (2026-08-20)

### Added

- `arxiv search` accepts more than one query. Each argument is a separate
  query, and the queries run in order under one shared request pace. With
  `--json` the result is an array of `{query, fallback, results}` objects, one
  per query, so an agent does not need a shell loop.
- `--strict` on `search` keeps a pure AND query and turns off the relaxed
  fallback.
- `--timeout` sets the request timeout in seconds (default 10). It is a
  whole-call deadline for an API request, or a per-read stall timeout for a PDF
  download, which can be large.

### Changed

- `search` matches terms with AND by default: every word must appear. The
  default sort is relevance. When a multi-word query finds nothing, `search`
  retries it once with the terms ORed and sorted by relevance, and prints a
  note that the fallback ran. Give a query as a short, quoted phrase.
- Requests keep a 3 second gap within one process, so a burst of calls does not
  trip the arXiv rate limit.

### Fixed

- A failed command exits with a nonzero code and prints a one-line error. With
  `--json`, standard output is always valid JSON, either the result or
  `{"error": "..."}`, and never empty. Before, a burst of calls could leave
  standard output empty and break a downstream JSON parse.
- Empty and `5xx` responses are retried with backoff, so a request that would
  return nothing now recovers instead of failing.
- `--json` and `--ids-only` can no longer be combined, because the two output
  modes would make standard output ambiguous.

## 0.1.1 (2026-08-11)

### Added

- Retry with backoff on arXiv rate limiting (`429` and `503`) and transient
  network errors.
- A Claude Code skill so agents use the CLI instead of fetching arxiv.org
  pages. Install it with skills.sh.

## 0.1.0 (2026-08-11)

### Added

- First release. Commands: `search`, `get`, `summary`, `pdf`, `bibtex`, and
  `latest`. `--json` and `--ids-only` output for agents. A single static binary
  with no runtime dependencies, installable with Homebrew.
