# arxiv-cli

A fast, minimal arXiv CLI built for humans *and* agents. Single ~1.4 MB static binary, no runtime dependencies, blocking I/O (no async runtime bloat).

## Install

```sh
brew tap ishaanko/tap https://github.com/ishaanko/arxiv-cli
brew install arxiv
```

Or build from source:

```sh
cargo install --path .
```

## Usage

```sh
# Search (multi-word queries are phrase-matched; alias: `arxiv query`)
arxiv search "attention is all you need" --max 5
arxiv search "diffusion models" --category cs.LG --sort date
arxiv search --author "Yoshua Bengio" --title "generative" --max 10

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

## Agent-friendly design

- `--json` on `search`, `get`, `summary`, and `latest` emits clean structured JSON.
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
| `--max`, `-m` / `--start` | Page size / offset for pagination |

## Releasing

This repo doubles as its own Homebrew tap via `Formula/arxiv.rb`. To release: tag `vX.Y.Z`, create a GitHub release with the `arxiv-X.Y.Z-arm64-darwin.tar.gz` binary asset, then bump `url`/`sha256`/`version` in the formula. Apple Silicon installs the prebuilt binary; other platforms build from the source tarball.
