mod config;
mod display;
mod models;
mod relay;
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
    #[command(visible_alias = "s")]
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

        /// Show match counts per project instead of results
        #[arg(long, short)]
        count: bool,

        /// Output results as JSON (one per line)
        #[arg(long)]
        json: bool,
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
    },

    /// Inter-Claude relay for real-time communication
    Relay {
        #[command(subcommand)]
        action: RelayAction,
    },
}

#[derive(Subcommand)]
enum RelayAction {
    /// Register a Claude instance to a tmux pane
    Register {
        /// Instance name (e.g., claude727)
        name: String,

        /// tmux pane target (e.g., %0, session:window.pane)
        #[arg(long, short)]
        pane: Option<String>,
    },

    /// Unregister a Claude instance
    Unregister {
        /// Instance name
        name: String,
    },

    /// Check for new messages and relay (called by Stop hook)
    Check {
        /// Transcript path (passed by hook via stdin if not specified)
        #[arg(long)]
        transcript: Option<String>,
    },

    /// Show registered instances
    Status,

    /// Send a message to another Claude instance
    Send {
        /// Target instance name
        to: String,

        /// Message text
        message: String,
    },
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
            count,
            json,
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
                count_mode: count,
                json_mode: json,
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
            print_stats(&files)?;
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
            print_projects(&files)?;
        }

        Commands::Freq { mode, limit } => {
            let files = cfg.discover_jsonl_files()?;
            match mode.as_str() {
                "chars" | "c" => print_freq_chars(&files)?,
                "words" | "w" => print_freq_words(&files, limit)?,
                "tools" | "t" => print_freq_tools(&files, limit)?,
                "roles" | "r" => print_freq_roles(&files)?,
                _ => anyhow::bail!("Unknown freq mode '{}'. Use: chars, words, tools, roles", mode),
            }
        }

        Commands::Recent { limit, role } => {
            let files = cfg.discover_jsonl_files()?;
            session::show_recent(&files, limit, role.as_deref())?;
        }

        Commands::Relay { action } => match action {
            RelayAction::Register { name, pane } => {
                relay::register(&name, pane.as_deref())?;
            }
            RelayAction::Unregister { name } => {
                relay::unregister(&name)?;
            }
            RelayAction::Check { transcript } => {
                relay::check(transcript.as_deref())?;
            }
            RelayAction::Status => {
                relay::status()?;
            }
            RelayAction::Send { to, message } => {
                relay::send(&to, &message)?;
            }
        },
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

fn print_projects(files: &[config::SessionFile]) -> Result<()> {
    use colored::*;
    use std::collections::HashMap;

    struct ProjectInfo {
        sessions: usize,
        total_size: u64,
        earliest: Option<String>,
        latest: Option<String>,
    }

    let mut projects: HashMap<String, ProjectInfo> = HashMap::new();

    for file in files {
        let entry = projects
            .entry(file.project_name.clone())
            .or_insert(ProjectInfo {
                sessions: 0,
                total_size: 0,
                earliest: None,
                latest: None,
            });
        entry.sessions += 1;
        entry.total_size += file.size_bytes;

        // Quick scan for timestamps
        if let Ok(f) = std::fs::File::open(&file.path) {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(f);
            for line in reader.lines().take(5) {
                let Ok(line) = line else { continue };
                if let Ok(record) = serde_json::from_str::<models::Record>(&line) {
                    if let Some(msg) = record.as_message_record() {
                        if let Some(ts) = &msg.timestamp {
                            let ts_date = ts.get(..10).unwrap_or(ts);
                            if entry.earliest.is_none()
                                || entry.earliest.as_deref().unwrap_or("") > ts_date
                            {
                                entry.earliest = Some(ts_date.to_string());
                            }
                            if entry.latest.is_none()
                                || entry.latest.as_deref().unwrap_or("") < ts_date
                            {
                                entry.latest = Some(ts_date.to_string());
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    let mut sorted: Vec<_> = projects.into_iter().collect();
    sorted.sort_by(|a, b| {
        b.1.latest
            .as_deref()
            .unwrap_or("")
            .cmp(a.1.latest.as_deref().unwrap_or(""))
    });

    println!(
        "{} projects\n",
        sorted.len().to_string().bold()
    );

    for (name, info) in &sorted {
        let date_range = match (&info.earliest, &info.latest) {
            (Some(e), Some(l)) if e == l => e.clone(),
            (Some(e), Some(l)) => format!("{} → {}", e, l),
            (Some(d), None) | (None, Some(d)) => d.clone(),
            (None, None) => "unknown".to_string(),
        };

        println!(
            "  {:30} {:>4} sessions  {:>8}  {}",
            name.cyan(),
            info.sessions,
            format_bytes(info.total_size),
            date_range.dimmed()
        );
    }

    Ok(())
}

fn print_freq_chars(files: &[config::SessionFile]) -> Result<()> {
    use colored::*;
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    let counts: Vec<AtomicU64> = (0..26).map(|_| AtomicU64::new(0)).collect();

    let pb = indicatif::ProgressBar::new(files.len() as u64);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} files")
            .unwrap()
            .progress_chars("█▓░"),
    );

    files.par_iter().for_each(|file| {
        if let Ok(data) = std::fs::read(&file.path) {
            for &b in &data {
                let idx = match b {
                    b'a'..=b'z' => (b - b'a') as usize,
                    b'A'..=b'Z' => (b - b'A') as usize,
                    _ => continue,
                };
                counts[idx].fetch_add(1, Ordering::Relaxed);
            }
        }
        pb.inc(1);
    });

    pb.finish_and_clear();

    let totals: Vec<u64> = counts.iter().map(|c| c.load(Ordering::Relaxed)).collect();
    let max_count = *totals.iter().max().unwrap_or(&1);
    let grand_total: u64 = totals.iter().sum();

    println!("{}", "Character Frequency (a-z, case-insensitive)".bold().cyan());
    println!("{}", "═".repeat(60));

    for (i, count) in totals.iter().enumerate() {
        let letter = (b'a' + i as u8) as char;
        let bar_len = (*count as f64 / max_count as f64 * 40.0) as usize;
        let bar = "█".repeat(bar_len);
        let pct = *count as f64 / grand_total as f64 * 100.0;
        println!(
            "  {}  {:>12}  ({:>5.2}%)  {}",
            letter.to_string().bold(),
            format_count(*count),
            pct,
            bar.cyan()
        );
    }

    println!("{}", "─".repeat(60));
    println!(
        "  Total: {}  across {} files ({})",
        format_count(grand_total).bold(),
        files.len(),
        format_bytes(files.iter().map(|f| f.size_bytes).sum())
    );

    Ok(())
}

fn print_freq_words(files: &[config::SessionFile], limit: usize) -> Result<()> {
    use colored::*;
    use rayon::prelude::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    let word_counts: Mutex<HashMap<String, u64>> = Mutex::new(HashMap::new());

    let pb = indicatif::ProgressBar::new(files.len() as u64);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} files")
            .unwrap()
            .progress_chars("█▓░"),
    );

    files.par_iter().for_each(|file| {
        let mut local: HashMap<String, u64> = HashMap::new();
        if let Ok(f) = std::fs::File::open(&file.path) {
            use std::io::BufRead;
            let reader = std::io::BufReader::with_capacity(256 * 1024, f);
            for line in reader.lines() {
                let Ok(line) = line else { continue };
                let Ok(record) = serde_json::from_str::<models::Record>(&line) else { continue };
                let Some(msg) = record.as_message_record() else { continue };
                let text = msg.text_content();
                for word in text.split(|c: char| !c.is_alphanumeric()) {
                    if word.len() >= 3 {
                        *local.entry(word.to_lowercase()).or_default() += 1;
                    }
                }
            }
        }
        let mut global = word_counts.lock().unwrap();
        for (word, count) in local {
            *global.entry(word).or_default() += count;
        }
        pb.inc(1);
    });

    pb.finish_and_clear();

    let counts = word_counts.into_inner().unwrap();
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let max_count = sorted.first().map(|(_, c)| *c).unwrap_or(1);

    println!("{}", "Word Frequency (top words, 3+ chars)".bold().cyan());
    println!("{}", "═".repeat(60));

    for (word, count) in sorted.iter().take(limit) {
        let bar_len = (*count as f64 / max_count as f64 * 30.0) as usize;
        let bar = "█".repeat(bar_len);
        println!("  {:20} {:>12}  {}", word.bold(), format_count(*count), bar.cyan());
    }

    let grand_total: u64 = sorted.iter().map(|(_, c)| c).sum();
    println!("{}", "─".repeat(60));
    println!("  {} unique words, {} total occurrences", format_count(sorted.len() as u64), format_count(grand_total));

    Ok(())
}

fn print_freq_tools(files: &[config::SessionFile], limit: usize) -> Result<()> {
    use colored::*;
    use rayon::prelude::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    let tool_counts: Mutex<HashMap<String, u64>> = Mutex::new(HashMap::new());

    let pb = indicatif::ProgressBar::new(files.len() as u64);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} files")
            .unwrap()
            .progress_chars("█▓░"),
    );

    files.par_iter().for_each(|file| {
        let mut local: HashMap<String, u64> = HashMap::new();
        if let Ok(f) = std::fs::File::open(&file.path) {
            use std::io::BufRead;
            let reader = std::io::BufReader::with_capacity(256 * 1024, f);
            for line in reader.lines() {
                let Ok(line) = line else { continue };
                let Ok(record) = serde_json::from_str::<models::Record>(&line) else { continue };
                let Some(msg) = record.as_message_record() else { continue };
                for tool in msg.tool_calls() {
                    *local.entry(tool.to_string()).or_default() += 1;
                }
            }
        }
        let mut global = tool_counts.lock().unwrap();
        for (tool, count) in local {
            *global.entry(tool).or_default() += count;
        }
        pb.inc(1);
    });

    pb.finish_and_clear();

    let counts = tool_counts.into_inner().unwrap();
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let max_count = sorted.first().map(|(_, c)| *c).unwrap_or(1);
    let grand_total: u64 = sorted.iter().map(|(_, c)| c).sum();

    println!("{}", "Tool Usage Frequency".bold().cyan());
    println!("{}", "═".repeat(60));

    for (tool, count) in sorted.iter().take(limit) {
        let bar_len = (*count as f64 / max_count as f64 * 30.0) as usize;
        let bar = "█".repeat(bar_len);
        let pct = *count as f64 / grand_total as f64 * 100.0;
        println!("  {:20} {:>10}  ({:>5.1}%)  {}", tool.bold(), format_count(*count), pct, bar.cyan());
    }

    println!("{}", "─".repeat(60));
    println!("  {} total tool calls", format_count(grand_total));

    Ok(())
}

fn print_freq_roles(files: &[config::SessionFile]) -> Result<()> {
    use colored::*;
    use rayon::prelude::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    let role_counts: Mutex<HashMap<String, u64>> = Mutex::new(HashMap::new());

    let pb = indicatif::ProgressBar::new(files.len() as u64);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} files")
            .unwrap()
            .progress_chars("█▓░"),
    );

    files.par_iter().for_each(|file| {
        let mut local: HashMap<String, u64> = HashMap::new();
        if let Ok(f) = std::fs::File::open(&file.path) {
            use std::io::BufRead;
            let reader = std::io::BufReader::with_capacity(256 * 1024, f);
            for line in reader.lines() {
                let Ok(line) = line else { continue };
                let Ok(record) = serde_json::from_str::<models::Record>(&line) else { continue };
                if record.is_message() {
                    *local.entry(record.role_str().to_string()).or_default() += 1;
                }
            }
        }
        let mut global = role_counts.lock().unwrap();
        for (role, count) in local {
            *global.entry(role).or_default() += count;
        }
        pb.inc(1);
    });

    pb.finish_and_clear();

    let counts = role_counts.into_inner().unwrap();
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let max_count = sorted.first().map(|(_, c)| *c).unwrap_or(1);
    let grand_total: u64 = sorted.iter().map(|(_, c)| c).sum();

    println!("{}", "Message Role Frequency".bold().cyan());
    println!("{}", "═".repeat(60));

    for (role, count) in &sorted {
        let bar_len = (*count as f64 / max_count as f64 * 40.0) as usize;
        let bar = "█".repeat(bar_len);
        let pct = *count as f64 / grand_total as f64 * 100.0;
        println!("  {:20} {:>10}  ({:>5.1}%)  {}", role.bold(), format_count(*count), pct, bar.cyan());
    }

    println!("{}", "─".repeat(60));
    println!("  {} total messages", format_count(grand_total));

    Ok(())
}

fn format_count(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
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
