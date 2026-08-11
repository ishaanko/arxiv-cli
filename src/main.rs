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
    after_help = "EXAMPLES:\n  arxiv search \"attention is all you need\" --max 5\n  arxiv search \"diffusion models\" --category cs.LG --sort date --json\n  arxiv get 1706.03762 --json\n  arxiv summary 1706.03762\n  arxiv pdf 1706.03762 -o attention.pdf\n  arxiv pdf https://arxiv.org/abs/1706.03762\n  arxiv bibtex 1706.03762\n  arxiv latest cs.CL --max 10"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
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

#[derive(Subcommand)]
enum Command {
    /// Search arXiv (full-text query over all fields)
    #[command(alias = "query")]
    Search {
        /// Search terms (quoted phrases are matched exactly)
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
        /// Emit machine-readable JSON
        #[arg(short, long)]
        json: bool,
        /// Print only arXiv IDs, one per line (for piping)
        #[arg(long)]
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
        #[arg(long)]
        ids_only: bool,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Search {
            query,
            category,
            author,
            title,
            abstract_,
            max,
            start,
            sort,
            json,
            ids_only,
        } => {
            let q = client::SearchQuery {
                terms: query.join(" "),
                category,
                author,
                title,
                abstract_: abstract_,
                max,
                start,
                sort: match sort {
                    Sort::Relevance => "relevance",
                    Sort::Date => "submittedDate",
                    Sort::Updated => "lastUpdatedDate",
                },
            };
            let papers = client::search(&q)?;
            output::print_list(&papers, json, ids_only)
        }
        Command::Get { ids, json } => {
            anyhow::ensure!(!ids.is_empty(), "provide at least one arXiv ID or URL");
            let ids: Vec<String> = ids.iter().map(|s| model::normalize_id(s)).collect::<Result<_>>()?;
            let papers = client::by_ids(&ids)?;
            output::print_full(&papers, json)
        }
        Command::Summary { id, json } => {
            let id = model::normalize_id(&id)?;
            let papers = client::by_ids(&[id.clone()])?;
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
            let papers = client::search(&q)?;
            output::print_list(&papers, json, ids_only)
        }
    }
}
