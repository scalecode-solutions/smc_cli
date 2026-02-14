mod config;
mod display;
mod models;
mod search;
mod session;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "smc",
    about = "smc - Surgical search through Claude Code conversation logs",
    version
)]
struct Cli {
    /// Path to Claude projects directory (default: ~/.claude/projects)
    #[arg(long, global = true)]
    path: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Search across all conversations
    Search {
        /// Search queries (multiple terms are OR'd together)
        query: Vec<String>,

        /// Treat query as regex
        #[arg(long, short = 'e')]
        regex: bool,

        /// Filter by role (user, assistant, system)
        #[arg(long)]
        role: Option<String>,

        /// Filter by tool name
        #[arg(long)]
        tool: Option<String>,

        /// Filter by project name (substring match)
        #[arg(long, short)]
        project: Option<String>,

        /// Only results after this date (YYYY-MM-DD)
        #[arg(long)]
        after: Option<String>,

        /// Only results before this date (YYYY-MM-DD)
        #[arg(long)]
        before: Option<String>,

        /// Filter by git branch
        #[arg(long)]
        branch: Option<String>,

        /// Maximum number of results
        #[arg(long, short = 'n', default_value = "50")]
        max: usize,

        /// Print markdown to stdout (pipeable)
        #[arg(long, short)]
        output: bool,

        /// Save results to a markdown file
        #[arg(long, value_name = "FILE")]
        md: Option<String>,
    },

    /// List all sessions
    Sessions {
        /// Maximum sessions to show
        #[arg(long, short = 'n', default_value = "20")]
        limit: usize,

        /// Filter by project name
        #[arg(long, short)]
        project: Option<String>,
    },

    /// Show a conversation
    Show {
        /// Session ID (or prefix)
        session: String,

        /// Show thinking blocks
        #[arg(long)]
        thinking: bool,
    },

    /// Show tool calls in a session
    Tools {
        /// Session ID (or prefix)
        session: String,
    },

    /// Show aggregate statistics
    Stats,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = config::Config::new(cli.path.as_deref())?;

    match cli.command {
        Commands::Search {
            query,
            regex,
            role,
            tool,
            project,
            after,
            before,
            branch,
            max,
            output,
            md,
        } => {
            let files = cfg.discover_jsonl_files()?;
            let opts = search::SearchOpts {
                queries: query,
                is_regex: regex,
                role,
                tool,
                project,
                after,
                before,
                branch,
                max_results: max,
                stdout_md: output,
                md_file: md,
            };
            search::search(&files, &opts)?;
        }

        Commands::Sessions { limit, project } => {
            let mut files = cfg.discover_jsonl_files()?;
            if let Some(proj) = &project {
                files.retain(|f| {
                    f.project_name
                        .to_lowercase()
                        .contains(&proj.to_lowercase())
                });
            }
            session::list_sessions(&files, limit)?;
        }

        Commands::Show { session, thinking } => {
            let files = cfg.discover_jsonl_files()?;
            let file = find_session(&files, &session)?;
            session::show_session(file, thinking)?;
        }

        Commands::Tools { session } => {
            let files = cfg.discover_jsonl_files()?;
            let file = find_session(&files, &session)?;
            session::show_tools(file)?;
        }

        Commands::Stats => {
            let files = cfg.discover_jsonl_files()?;
            print_stats(&files)?;
        }
    }

    Ok(())
}

fn find_session<'a>(
    files: &'a [config::SessionFile],
    query: &str,
) -> Result<&'a config::SessionFile> {
    // Exact match first
    if let Some(f) = files.iter().find(|f| f.session_id == query) {
        return Ok(f);
    }
    // Prefix match
    let matches: Vec<_> = files
        .iter()
        .filter(|f| f.session_id.starts_with(query))
        .collect();
    match matches.len() {
        0 => anyhow::bail!("No session found matching '{}'", query),
        1 => Ok(matches[0]),
        n => {
            eprintln!("Ambiguous session ID '{}', {} matches:", query, n);
            for m in &matches {
                eprintln!("  {} ({})", m.session_id, m.project_name);
            }
            anyhow::bail!("Please provide a more specific session ID");
        }
    }
}

fn print_stats(files: &[config::SessionFile]) -> Result<()> {
    use colored::*;
    use std::collections::HashMap;

    let total_files = files.len();
    let total_size: u64 = files.iter().map(|f| f.size_bytes).sum();

    let mut projects: HashMap<String, (usize, u64)> = HashMap::new();
    for f in files {
        let entry = projects.entry(f.project_name.clone()).or_default();
        entry.0 += 1;
        entry.1 += f.size_bytes;
    }

    println!("{}", "smc Stats".bold().cyan());
    println!("{}", "═".repeat(50));
    println!("  Total sessions:  {}", total_files.to_string().bold());
    println!(
        "  Total size:      {}",
        format_bytes(total_size).bold()
    );
    println!("  Projects:        {}", projects.len().to_string().bold());
    println!();

    println!("{}", "Top Projects by Size".bold());
    println!("{}", "─".repeat(50));

    let mut sorted: Vec<_> = projects.into_iter().collect();
    sorted.sort_by(|a, b| b.1 .1.cmp(&a.1 .1));

    for (name, (count, size)) in sorted.iter().take(15) {
        println!(
            "  {:30} {:>4} sessions  {:>8}",
            name.cyan(),
            count,
            format_bytes(*size)
        );
    }

    if sorted.len() > 15 {
        println!("  ... and {} more projects", sorted.len() - 15);
    }

    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2}GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
