/// smc — Search My Claude.
///
/// Clap CLI harness. All business logic lives in smc::cmd::*.
use clap::{Parser, Subcommand};
use smc::cmd;
use smc::output::Emitter;
use smc::util::discover;

// ── Top-level ──────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "smc",
    version,
    about = "Surgical search through Claude Code conversation logs",
    long_about = "Structured JSONL output for search, sessions, show, tools, export, \
                  context, stats, projects, freq, recent. Every record is machine-parseable \
                  JSON Lines — zero ANSI, zero pagination.",
    after_help = "Exit codes: 0 = success/match, 1 = no results, 2 = error\n\n\
                  NOTE: All output is single-line JSONL. Pipe through `cat` or redirect \
                  to a file to get clean output."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to Claude projects directory (default: ~/.claude/projects)
    #[arg(long, global = true)]
    path: Option<String>,

    /// Hard cap on output tokens (0 = unlimited)
    #[arg(long, global = true, value_name = "N")]
    max_tokens: Option<usize>,
}

// ── Commands ───────────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum Commands {
    /// Search across all conversations
    #[command(visible_alias = "s")]
    Search(SearchArgs),

    /// List sessions with metadata
    #[command(visible_alias = "ls")]
    Sessions(SessionsArgs),

    /// Show a conversation
    Show(ShowArgs),

    /// List tool calls in a session
    #[command(visible_alias = "t")]
    Tools(ToolsArgs),

    /// Aggregate statistics
    Stats,

    /// Export a session as markdown
    #[command(visible_alias = "e")]
    Export(ExportArgs),

    /// Show messages around a specific line
    #[command(visible_alias = "ctx")]
    Context(ContextArgs),

    /// List projects with stats
    #[command(visible_alias = "p")]
    Projects,

    /// Frequency analysis
    #[command(visible_alias = "f")]
    Freq(FreqArgs),

    /// Most recent messages across sessions
    #[command(visible_alias = "r")]
    Recent(RecentArgs),
}

// ── search ─────────────────────────────────────────────────────────────────

#[derive(Parser)]
struct SearchArgs {
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

    /// Include results from previous smc output (excluded by default)
    #[arg(long, short = 'i')]
    include_smc: bool,

    /// Exclude a specific session ID
    #[arg(long)]
    exclude_session: Option<String>,

    /// Filter to messages that touch a file path
    #[arg(long)]
    file: Option<String>,

    /// Search only within tool input content
    #[arg(long)]
    tool_input: bool,

    /// Search only within thinking blocks
    #[arg(long)]
    thinking: bool,

    /// Exclude thinking blocks from search
    #[arg(long)]
    no_thinking: bool,
}

// ── sessions ───────────────────────────────────────────────────────────────

#[derive(Parser)]
struct SessionsArgs {
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
}

// ── show ───────────────────────────────────────────────────────────────────

#[derive(Parser)]
struct ShowArgs {
    /// Session ID (or prefix)
    session: String,

    /// Include thinking blocks
    #[arg(long)]
    thinking: bool,

    /// Start from this message number
    #[arg(long)]
    from: Option<usize>,

    /// End at this message number
    #[arg(long)]
    to: Option<usize>,
}

// ── tools ──────────────────────────────────────────────────────────────────

#[derive(Parser)]
struct ToolsArgs {
    /// Session ID (or prefix)
    session: String,
}

// ── export ─────────────────────────────────────────────────────────────────

#[derive(Parser)]
struct ExportArgs {
    /// Session ID (or prefix)
    session: String,

    /// Print markdown to stdout
    #[arg(long, short)]
    output: bool,

    /// Output file path (default: <session-id>.md)
    #[arg(long, value_name = "FILE")]
    md: Option<String>,
}

// ── context ────────────────────────────────────────────────────────────────

#[derive(Parser)]
struct ContextArgs {
    /// Session ID (or prefix)
    session: String,

    /// Line number to center on
    line: usize,

    /// Number of messages to show before and after
    #[arg(long, short = 'C', default_value = "3")]
    context: usize,
}

// ── freq ───────────────────────────────────────────────────────────────────

#[derive(Parser)]
struct FreqArgs {
    /// What to count: chars, words, tools, roles
    #[arg(default_value = "chars")]
    mode: String,

    /// Max items to show (for words mode)
    #[arg(long, short = 'n', default_value = "30")]
    limit: usize,

    /// Count raw file bytes instead of parsed message content
    #[arg(long)]
    raw: bool,
}

// ── recent ─────────────────────────────────────────────────────────────────

#[derive(Parser)]
struct RecentArgs {
    /// Number of recent messages to show
    #[arg(long, short = 'n', default_value = "10")]
    limit: usize,

    /// Filter by role
    #[arg(long)]
    role: Option<String>,

    /// Filter by project name (substring match)
    #[arg(long, short)]
    project: Option<String>,
}

// ── main ───────────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();
    let max_tokens = cli.max_tokens.unwrap_or(0);

    let result = run(cli, max_tokens);

    match result {
        Ok(true) => std::process::exit(0),
        Ok(false) => std::process::exit(1),
        Err(e) => {
            eprintln!("{:#}", e);
            std::process::exit(2);
        }
    }
}

/// Returns Ok(true) for success/matches, Ok(false) for no results.
fn run(cli: Cli, max_tokens: usize) -> anyhow::Result<bool> {
    let claude_dir = discover::claude_dir(cli.path.as_deref())?;
    let files = discover::discover_jsonl_files(&claude_dir)?;

    match cli.command {
        Commands::Search(args) => {
            let opts = cmd::search::SearchOpts {
                queries: args.query,
                is_regex: args.regex,
                and_mode: args.and,
                role: args.role,
                tool: args.tool,
                project: args.project,
                after: args.after,
                before: args.before,
                branch: args.branch,
                file: args.file,
                tool_input: args.tool_input,
                thinking_only: args.thinking,
                no_thinking: args.no_thinking,
                max_results: args.max,
                include_smc: args.include_smc,
                exclude_session: args.exclude_session,
                max_tokens,
            };
            let mut em = Emitter::stdout(max_tokens);
            cmd::search::run(&opts, &files, &mut em)?;
        }

        Commands::Sessions(args) => {
            let opts = cmd::sessions::SessionsOpts {
                limit: args.limit,
                project: args.project,
                after: args.after,
                before: args.before,
            };
            let mut em = Emitter::stdout(max_tokens);
            cmd::sessions::run(&opts, &files, &mut em)?;
        }

        Commands::Show(args) => {
            let file = discover::find_session(&files, &args.session)?;
            let opts = cmd::show::ShowOpts {
                session: args.session,
                thinking: args.thinking,
                from: args.from,
                to: args.to,
                max_tokens,
            };
            let mut em = Emitter::stdout(max_tokens);
            cmd::show::run(&opts, file, &mut em)?;
        }

        Commands::Tools(args) => {
            let file = discover::find_session(&files, &args.session)?;
            let opts = cmd::tools::ToolsOpts {
                session: args.session,
                max_tokens,
            };
            let mut em = Emitter::stdout(max_tokens);
            cmd::tools::run(&opts, file, &mut em)?;
        }

        Commands::Stats => {
            let opts = cmd::stats::StatsOpts { max_tokens };
            let mut em = Emitter::stdout(max_tokens);
            cmd::stats::run(&opts, &files, &mut em)?;
        }

        Commands::Export(args) => {
            let file = discover::find_session(&files, &args.session)?;
            let opts = cmd::export::ExportOpts {
                session: args.session,
                to_stdout: args.output,
                md_path: args.md,
            };
            let mut em = Emitter::stdout(max_tokens);
            cmd::export::run(&opts, file, &mut em)?;
        }

        Commands::Context(args) => {
            let file = discover::find_session(&files, &args.session)?;
            let opts = cmd::context::ContextOpts {
                session: args.session,
                line: args.line,
                context: args.context,
                max_tokens,
            };
            let mut em = Emitter::stdout(max_tokens);
            cmd::context::run(&opts, file, &mut em)?;
        }

        Commands::Projects => {
            let opts = cmd::projects::ProjectsOpts { max_tokens };
            let mut em = Emitter::stdout(max_tokens);
            cmd::projects::run(&opts, &files, &mut em)?;
        }

        Commands::Freq(args) => {
            let mode = cmd::freq::FreqMode::parse(&args.mode)?;
            let opts = cmd::freq::FreqOpts {
                mode,
                limit: args.limit,
                raw: args.raw,
                max_tokens,
            };
            let mut em = Emitter::stdout(max_tokens);
            cmd::freq::run(&opts, &files, &mut em)?;
        }

        Commands::Recent(args) => {
            let opts = cmd::recent::RecentOpts {
                limit: args.limit,
                role: args.role,
                project: args.project,
                max_tokens,
            };
            let mut em = Emitter::stdout(max_tokens);
            cmd::recent::run(&opts, &files, &mut em)?;
        }
    }

    Ok(true)
}
