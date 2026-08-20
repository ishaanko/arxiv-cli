mod client;
mod model;
mod output;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "arxiv",
    version,
    about = "A fast, minimal arXiv CLI for humans and agents",
    after_help = "EXAMPLES:\n  arxiv search \"synthetic visual reasoning\" --max 5\n  arxiv search \"diffusion models\" --category cs.LG --sort date --json\n  arxiv search \"mixture of experts\" \"state space models\" --json\n  arxiv get 1706.03762 --json\n  arxiv summary 1706.03762\n  arxiv pdf 1706.03762 -o attention.pdf\n  arxiv pdf https://arxiv.org/abs/1706.03762\n  arxiv bibtex 1706.03762\n  arxiv latest cs.CL --max 10"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    /// Per-request timeout in seconds: a whole-call deadline for each API
    /// attempt, or a per-read stall timeout for PDF downloads (which may be
    /// large). A failed request may still be retried with backoff, so total
    /// time under repeated failures can exceed one timeout.
    #[arg(long, global = true, default_value_t = 10)]
    timeout: u64,
}

#[derive(Clone, Copy, ValueEnum)]
enum Sort {
    /// Sort by relevance to the query
    Relevance,
    /// Sort by submission date
    Date,
    /// Sort by last-updated date
    Updated,
}

impl Sort {
    fn as_api(self) -> &'static str {
        match self {
            Sort::Relevance => "relevance",
            Sort::Date => "submittedDate",
            Sort::Updated => "lastUpdatedDate",
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Search arXiv (full-text query over all fields)
    ///
    /// Each argument is a separate query; quote a multi-word query so it stays
    /// one argument. Within a query the terms are ANDed by default (every word
    /// must appear). If a multi-word query returns nothing, it is retried once
    /// with the terms ORed and sorted by relevance, and a note is printed;
    /// pass --strict to keep pure AND and skip the fallback. Passing several
    /// queries runs them in order under one shared request pace and, with
    /// --json, returns an array of {query, fallback, results} objects.
    ///
    /// Recommended style is short, title-like queries, e.g.
    ///   arxiv search "synthetic visual reasoning" --max 5
    ///   arxiv search "mixture of experts" "state space models" --json
    /// For an exact phrase or raw arXiv syntax, pass it through directly, e.g.
    ///   arxiv search 'all:"chain of thought"'
    ///   arxiv search 'ti:"mamba" AND cat:cs.LG'
    #[command(alias = "query")]
    Search {
        /// One or more queries; quote multi-word queries (each arg is a query)
        query: Vec<String>,
        /// Restrict to a category, e.g. cs.LG, math.CO, hep-th
        #[arg(short, long)]
        category: Option<String>,
        /// Restrict to an author name
        #[arg(short, long)]
        author: Option<String>,
        /// Restrict to words in the title
        #[arg(short, long)]
        title: Option<String>,
        /// Restrict to words in the abstract
        #[arg(long = "abstract")]
        abstract_: Option<String>,
        /// Maximum number of results
        #[arg(short, long, default_value_t = 10)]
        max: usize,
        /// Result offset for pagination
        #[arg(long, default_value_t = 0)]
        start: usize,
        /// Sort order
        #[arg(short, long, value_enum, default_value_t = Sort::Relevance)]
        sort: Sort,
        /// Require every term (pure AND); disable the relaxed OR fallback
        #[arg(long)]
        strict: bool,
        /// Emit machine-readable JSON
        #[arg(short, long)]
        json: bool,
        /// Print only arXiv IDs, one per line (for piping)
        #[arg(long, conflicts_with = "json")]
        ids_only: bool,
    },
    /// Fetch full metadata for one or more papers by ID or URL
    Get {
        /// arXiv IDs or URLs (e.g. 1706.03762, arXiv:2301.10945v2, https://arxiv.org/abs/...)
        ids: Vec<String>,
        /// Emit machine-readable JSON
        #[arg(short, long)]
        json: bool,
    },
    /// Print the abstract of a paper
    Summary {
        /// arXiv ID or URL
        id: String,
        /// Emit machine-readable JSON
        #[arg(short, long)]
        json: bool,
    },
    /// Download the PDF of a paper
    Pdf {
        /// arXiv ID or URL (abs or pdf URL both work)
        id: String,
        /// Output file path (defaults to <id>.pdf in the current directory)
        #[arg(short, long)]
        output: Option<String>,
        /// Directory to save into (filename stays <id>.pdf)
        #[arg(short, long)]
        dir: Option<String>,
    },
    /// Print the official BibTeX citation for a paper
    Bibtex {
        /// arXiv ID or URL
        id: String,
    },
    /// List the most recent submissions in a category
    Latest {
        /// Category, e.g. cs.CL, stat.ML, quant-ph
        category: String,
        /// Maximum number of results
        #[arg(short, long, default_value_t = 10)]
        max: usize,
        /// Emit machine-readable JSON
        #[arg(short, long)]
        json: bool,
        /// Print only arXiv IDs, one per line
        #[arg(long, conflicts_with = "json")]
        ids_only: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    client::set_timeout(cli.timeout);
    let json = cli.command.emits_json();
    match run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Keep the human-facing error to a single line on stderr.
            let msg = format!("{e:#}").replace('\n', "; ");
            eprintln!("error: {msg}");
            // With --json, stdout must stay valid JSON even on failure so
            // callers never see empty output and a broken parse.
            if json {
                let obj = serde_json::json!({ "error": msg });
                println!(
                    "{}",
                    serde_json::to_string(&obj)
                        .unwrap_or_else(|_| "{\"error\":\"internal error\"}".to_string())
                );
            }
            ExitCode::FAILURE
        }
    }
}

impl Command {
    /// Whether this invocation emits JSON on stdout (so the error path must
    /// emit JSON too).
    fn emits_json(&self) -> bool {
        matches!(
            self,
            Command::Search { json, .. }
                | Command::Get { json, .. }
                | Command::Summary { json, .. }
                | Command::Latest { json, .. }
                if *json
        )
    }
}

fn run(command: Command) -> Result<()> {
    match command {
        Command::Search {
            query,
            category,
            author,
            title,
            abstract_,
            max,
            start,
            sort,
            strict,
            json,
            ids_only,
        } => {
            let sort = sort.as_api();
            // Each positional argument is a distinct query. With none, run a
            // single filter-only search (e.g. --author with no free text).
            let queries: Vec<String> = if query.is_empty() {
                vec![String::new()]
            } else {
                query
            };

            let make_query = |terms: String| client::SearchQuery {
                terms,
                category: category.clone(),
                author: author.clone(),
                title: title.clone(),
                abstract_: abstract_.clone(),
                max,
                start,
                sort,
            };
            let note_fallback = |terms: &str| {
                eprintln!(
                    "arxiv: no exact (AND) match for \"{terms}\"; retried with a relaxed OR search sorted by relevance"
                );
            };

            if queries.len() == 1 {
                let q = make_query(queries.into_iter().next().unwrap());
                let outcome = client::search(&q, strict)?;
                if outcome.fallback {
                    note_fallback(&q.terms);
                }
                output::print_list(&outcome.papers, json, ids_only)
            } else {
                let mut sets = Vec::with_capacity(queries.len());
                for terms in queries {
                    let q = make_query(terms.clone());
                    let outcome = client::search(&q, strict)?;
                    if outcome.fallback {
                        note_fallback(&terms);
                    }
                    sets.push((terms, outcome));
                }
                output::print_multi(&sets, json, ids_only)
            }
        }
        Command::Get { ids, json } => {
            anyhow::ensure!(!ids.is_empty(), "provide at least one arXiv ID or URL");
            let ids: Vec<String> = ids.iter().map(|s| model::normalize_id(s)).collect::<Result<_>>()?;
            let papers = client::by_ids(&ids)?;
            output::print_full(&papers, json)
        }
        Command::Summary { id, json } => {
            let id = model::normalize_id(&id)?;
            let papers = client::by_ids(std::slice::from_ref(&id))?;
            let paper = papers
                .first()
                .ok_or_else(|| anyhow::anyhow!("no paper found for id {id}"))?;
            output::print_summary(paper, json)
        }
        Command::Pdf { id, output, dir } => {
            let id = model::normalize_id(&id)?;
            let path = client::download_pdf(&id, output.as_deref(), dir.as_deref())?;
            println!("{}", path.display());
            Ok(())
        }
        Command::Bibtex { id } => {
            let id = model::normalize_id(&id)?;
            let bib = client::bibtex(&id)?;
            println!("{}", bib.trim_end());
            Ok(())
        }
        Command::Latest {
            category,
            max,
            json,
            ids_only,
        } => {
            let q = client::SearchQuery {
                terms: String::new(),
                category: Some(category),
                author: None,
                title: None,
                abstract_: None,
                max,
                start: 0,
                sort: "submittedDate",
            };
            // No free-text terms here, so the OR fallback never applies.
            let outcome = client::search(&q, true)?;
            output::print_list(&outcome.papers, json, ids_only)
        }
    }
}
