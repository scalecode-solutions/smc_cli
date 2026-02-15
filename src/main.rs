use smc_cli_cc::{analytics, config, search, session};

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
    #[command(visible_alias = "s")]
    Search {
        /// Search queries (multiple terms are OR'd together)
        query: Vec<String>,

        /// Treat query as regex
        #[arg(long, short = 'e')]
        regex: bool,

        /// Require ALL terms to match (default is OR)
        #[arg(long, short = 'a')]
        and: bool,

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

        /// Show match counts per project instead of results
        #[arg(long, short)]
        count: bool,

        /// Condensed summary: projects, roles, date range, top topics
        #[arg(long)]
        summary: bool,

        /// Output results as JSON (one per line)
        #[arg(long)]
        json: bool,

        /// Include results from previous smc output (excluded by default)
        #[arg(long, short = 'i')]
        include_smc: bool,

        /// Exclude a specific session ID
        #[arg(long)]
        exclude_session: Option<String>,
    },

    /// List all sessions
    #[command(visible_alias = "ls")]
    Sessions {
        /// Maximum sessions to show
        #[arg(long, short = 'n', default_value = "20")]
        limit: usize,

        /// Filter by project name
        #[arg(long, short)]
        project: Option<String>,

        /// Only sessions after this date (YYYY-MM-DD)
        #[arg(long)]
        after: Option<String>,

        /// Only sessions before this date (YYYY-MM-DD)
        #[arg(long)]
        before: Option<String>,
    },

    /// Show a conversation
    Show {
        /// Session ID (or prefix)
        session: String,

        /// Show thinking blocks
        #[arg(long)]
        thinking: bool,

        /// Start from this message number
        #[arg(long)]
        from: Option<usize>,

        /// End at this message number
        #[arg(long)]
        to: Option<usize>,
    },

    /// Show tool calls in a session
    #[command(visible_alias = "t")]
    Tools {
        /// Session ID (or prefix)
        session: String,
    },

    /// Show aggregate statistics
    Stats,

    /// Export a session as markdown
    #[command(visible_alias = "e")]
    Export {
        /// Session ID (or prefix)
        session: String,

        /// Print to stdout instead of file
        #[arg(long, short)]
        output: bool,

        /// Output file path (default: <session-id>.md)
        #[arg(long, value_name = "FILE")]
        md: Option<String>,
    },

    /// Show messages around a specific line in a session
    #[command(visible_alias = "ctx")]
    Context {
        /// Session ID (or prefix)
        session: String,

        /// Line number to center on
        line: usize,

        /// Number of messages to show before and after
        #[arg(long, short = 'C', default_value = "3")]
        context: usize,
    },

    /// List projects with aggregate stats
    #[command(visible_alias = "p")]
    Projects,

    /// Frequency analysis across all conversations
    #[command(visible_alias = "f")]
    Freq {
        /// What to count: chars, words, tools, roles
        #[arg(default_value = "chars")]
        mode: String,

        /// Max items to show (for words mode)
        #[arg(long, short = 'n', default_value = "30")]
        limit: usize,

        /// Count raw file bytes instead of parsed message content
        #[arg(long)]
        raw: bool,
    },

    /// Show most recent messages across all sessions
    #[command(visible_alias = "r")]
    Recent {
        /// Number of recent messages to show
        #[arg(long, short = 'n', default_value = "10")]
        limit: usize,

        /// Filter by role
        #[arg(long)]
        role: Option<String>,

        /// Filter by project name (substring match)
        #[arg(long, short)]
        project: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = config::Config::new(cli.path.as_deref())?;

    println!("{}", search::SMC_TAG_OPEN);

    let result = run(cli, cfg);

    println!("{}", search::SMC_TAG_CLOSE);

    result
}

fn run(cli: Cli, cfg: config::Config) -> Result<()> {
    match cli.command {
        Commands::Search {
            query,
            regex,
            and,
            role,
            tool,
            project,
            after,
            before,
            branch,
            max,
            output,
            md,
            count,
            summary,
            json,
            include_smc,
            exclude_session,
        } => {
            let files = cfg.discover_jsonl_files()?;
            let opts = search::SearchOpts {
                queries: query,
                is_regex: regex,
                and_mode: and,
                role,
                tool,
                project,
                after,
                before,
                branch,
                max_results: max,
                stdout_md: output,
                md_file: md,
                count_mode: count,
                summary_mode: summary,
                json_mode: json,
                include_smc,
                exclude_session,
            };
            search::search(&files, &opts)?;
        }

        Commands::Sessions {
            limit,
            project,
            after,
            before,
        } => {
            let mut files = cfg.discover_jsonl_files()?;
            if let Some(proj) = &project {
                files.retain(|f| {
                    f.project_name
                        .to_lowercase()
                        .contains(&proj.to_lowercase())
                });
            }
            session::list_sessions(&files, limit, after.as_deref(), before.as_deref())?;
        }

        Commands::Show {
            session,
            thinking,
            from,
            to,
        } => {
            let files = cfg.discover_jsonl_files()?;
            let file = find_session(&files, &session)?;
            session::show_session(file, thinking, from, to)?;
        }

        Commands::Tools { session } => {
            let files = cfg.discover_jsonl_files()?;
            let file = find_session(&files, &session)?;
            session::show_tools(file)?;
        }

        Commands::Stats => {
            let files = cfg.discover_jsonl_files()?;
            analytics::print_stats(&files)?;
        }

        Commands::Export {
            session,
            output,
            md,
        } => {
            let files = cfg.discover_jsonl_files()?;
            let file = find_session(&files, &session)?;
            session::export_session(file, output, md.as_deref())?;
        }

        Commands::Context {
            session,
            line,
            context,
        } => {
            let files = cfg.discover_jsonl_files()?;
            let file = find_session(&files, &session)?;
            session::show_context(file, line, context)?;
        }

        Commands::Projects => {
            let files = cfg.discover_jsonl_files()?;
            analytics::print_projects(&files)?;
        }

        Commands::Freq { mode, limit, raw } => {
            let files = cfg.discover_jsonl_files()?;
            match mode.as_str() {
                "chars" | "c" if raw => analytics::print_freq_chars_raw(&files)?,
                "chars" | "c" => analytics::print_freq_chars(&files)?,
                "words" | "w" => analytics::print_freq_words(&files, limit)?,
                "tools" | "t" => analytics::print_freq_tools(&files, limit)?,
                "roles" | "r" => analytics::print_freq_roles(&files)?,
                _ => anyhow::bail!("Unknown freq mode '{}'. Use: chars, words, tools, roles", mode),
            }
        }

        Commands::Recent { limit, role, project } => {
            let mut files = cfg.discover_jsonl_files()?;
            if let Some(proj) = &project {
                files.retain(|f| {
                    f.project_name
                        .to_lowercase()
                        .contains(&proj.to_lowercase())
                });
            }
            session::show_recent(&files, limit, role.as_deref())?;
        }
    }

    Ok(())
}

fn find_session<'a>(
    files: &'a [config::SessionFile],
    query: &str,
) -> Result<&'a config::SessionFile> {
    if let Some(f) = files.iter().find(|f| f.session_id == query) {
        return Ok(f);
    }
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
