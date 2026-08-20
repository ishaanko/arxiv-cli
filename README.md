# arxiv-cli

A fast, minimal arXiv CLI built for humans *and* agents. Single ~1.4 MB static binary, no runtime dependencies, blocking I/O (no async runtime bloat).

## Install

macOS / Linux (Homebrew):

```sh
brew tap ishaanko/tap https://github.com/ishaanko/arxiv-cli
brew install arxiv
```

Windows (Scoop):

```powershell
scoop bucket add ishaanko https://github.com/ishaanko/arxiv-cli
scoop install arxiv
```

Or build from source on any platform:

```sh
cargo install --path .
```

## Usage

```sh
# Search — use short, title-like queries in quotes (alias: `arxiv query`)
arxiv search "synthetic visual reasoning" --max 5
arxiv search "diffusion models" --category cs.LG --sort date
arxiv search --author "Yoshua Bengio" --title "generative" --max 10

# Each argument is a separate query; run several in one paced process
arxiv search "mixture of experts" "state space models" --json

# Full metadata for one or more papers — accepts IDs, arXiv:… or URLs
arxiv get 1706.03762
arxiv get https://arxiv.org/abs/2301.10945 arXiv:2104.04692v3

# Just the abstract
arxiv summary 1706.03762

# Download the PDF (prints the saved path)
arxiv pdf 1706.03762 -o attention.pdf
arxiv pdf https://arxiv.org/abs/1706.03762 --dir ~/papers

# Official BibTeX citation
arxiv bibtex 1706.03762

# Most recent submissions in a category
arxiv latest cs.CL --max 10
```

## How search matches

Give short, title-like queries and quote each one. Within a query the terms
are ANDed: every word must appear somewhere in the paper. Each argument is a
separate query, so `arxiv search "graph neural networks" "protein folding"`
runs two searches, not one.

If a multi-word query finds nothing, `arxiv` retries it once with the terms
ORed and sorted by relevance, and prints a note that the fallback ran. Pass
`--strict` to keep pure AND and skip the fallback. The default sort is
relevance. For an exact phrase or raw arXiv syntax, pass it through directly:

```sh
arxiv search 'all:"chain of thought"'
arxiv search 'ti:"mamba" AND cat:cs.LG'
```

## Reliability

- The client keeps a 3-second gap between requests within one process, so a
  burst of searches never trips arXiv's rate limit.
- Each API request has a hard overall deadline (10 s, set with `--timeout`), so
  a slow trickle can never run past it. PDF downloads use the same value as a
  per-read stall timeout, so a large file on a slow link is not cut off midway.
  Empty, `429`, and `5xx` responses are retried with backoff.
- Failures are loud: a one-line message on stderr and a nonzero exit code.
  With `--json`, stdout is always valid JSON — either the result or
  `{"error": "..."}` — and never empty.

## Agent-friendly design

- `--json` on `search`, `get`, `summary`, and `latest` emits clean structured JSON.
- One query with `--json` returns a flat array of papers. Several queries return
  an array of `{query, fallback, results}` objects, one per query.
- `--ids-only` on `search`/`latest` prints one ID per line for piping:
  `arxiv search "state space models" --ids-only | xargs arxiv get --json`
- Exit code `0` on success, `1` on failure, with errors on stderr — stdout stays parseable.
- `arxiv pdf` prints exactly one line on success: the path of the saved file.
- Advanced arXiv query syntax passes through untouched:
  `arxiv search 'ti:"mamba" AND cat:cs.LG'`

## Search options

| Flag | Meaning |
|------|---------|
| `--category`, `-c` | Restrict to a category (`cs.LG`, `math.CO`, `hep-th`, …) |
| `--author`, `-a` | Restrict to an author name |
| `--title`, `-t` | Restrict to words in the title |
| `--abstract` | Restrict to words in the abstract |
| `--sort`, `-s` | `relevance` (default), `date`, `updated` |
| `--strict` | Require every term (pure AND); skip the OR fallback |
| `--max`, `-m` / `--start` | Page size / offset for pagination |
| `--timeout` | Hard timeout per HTTP request, in seconds (default 10) |

## Agent skill

`skills/arxiv/SKILL.md` teaches coding agents (Claude Code, Codex, Gemini CLI, Zed, and any SKILL.md-compatible agent) to use this CLI instead of fetching arxiv.org pages. Install via [skills.sh](https://skills.sh):

```sh
npx skills add ishaanko/arxiv-cli
```

This detects the agents on your machine and installs the skill for each. Manual alternative for Claude Code:

```sh
mkdir -p ~/.claude/skills/arxiv
curl -fsSL https://raw.githubusercontent.com/ishaanko/arxiv-cli/main/skills/arxiv/SKILL.md -o ~/.claude/skills/arxiv/SKILL.md
```

## Releasing

This repo doubles as its own Homebrew tap (`Formula/arxiv.rb`) and Scoop bucket (`bucket/arxiv.json`). Every release must publish **both** binary assets, or the package manager for the missing platform silently stops updating:

- `arxiv-X.Y.Z-arm64-darwin.tar.gz` — required by the Homebrew formula
- `arxiv-X.Y.Z-x86_64-windows.zip` — required by the Scoop bucket's `autoupdate`

To release: tag `vX.Y.Z`, build both assets and attach them to the GitHub release, then bump `url`/`sha256`/`version` in `Formula/arxiv.rb` and `bucket/arxiv.json`. Apple Silicon and Windows install the prebuilt binaries; other platforms build from the source tarball via Homebrew.
