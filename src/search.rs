use crate::config::SessionFile;
use crate::display;
use crate::models::Record;
use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use regex::Regex;
use std::io::BufRead;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct SearchOpts {
    pub queries: Vec<String>,
    pub is_regex: bool,
    pub and_mode: bool,
    pub role: Option<String>,
    pub tool: Option<String>,
    pub project: Option<String>,
    pub after: Option<String>,
    pub before: Option<String>,
    pub branch: Option<String>,
    pub max_results: usize,
    pub stdout_md: bool,
    pub md_file: Option<String>,
    pub count_mode: bool,
    pub summary_mode: bool,
    pub json_mode: bool,
    pub include_smc: bool,
    pub exclude_session: Option<String>,
}

pub const SMC_TAG_OPEN: &str = "<smc-cc-cli>";
pub const SMC_TAG_CLOSE: &str = "</smc-cc-cli>";

impl SearchOpts {
    pub fn query_display(&self) -> String {
        self.queries.join(", ")
    }
}

struct Matcher {
    regexes: Vec<Regex>,
    plains: Vec<String>,
    and_mode: bool,
}

impl Matcher {
    fn new(queries: &[String], is_regex: bool, and_mode: bool) -> Result<Self> {
        if is_regex {
            let regexes = queries
                .iter()
                .map(|q| Regex::new(q))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(Matcher {
                regexes,
                plains: vec![],
                and_mode,
            })
        } else {
            Ok(Matcher {
                regexes: vec![],
                plains: queries.iter().map(|q| q.to_lowercase()).collect(),
                and_mode,
            })
        }
    }

    fn first_matching_query(&self, text: &str) -> Option<String> {
        if self.and_mode {
            return self.all_match(text);
        }
        if !self.regexes.is_empty() {
            for re in &self.regexes {
                if let Some(m) = re.find(text) {
                    return Some(m.as_str().to_string());
                }
            }
        } else {
            let lower = text.to_lowercase();
            for q in &self.plains {
                if lower.contains(q.as_str()) {
                    return Some(q.clone());
                }
            }
        }
        None
    }

    fn all_match(&self, text: &str) -> Option<String> {
        if !self.regexes.is_empty() {
            let mut matches = Vec::new();
            for re in &self.regexes {
                if let Some(m) = re.find(text) {
                    matches.push(m.as_str().to_string());
                } else {
                    return None;
                }
            }
            Some(matches.join(" + "))
        } else {
            let lower = text.to_lowercase();
            for q in &self.plains {
                if !lower.contains(q.as_str()) {
                    return None;
                }
            }
            Some(self.plains.join(" + "))
        }
    }
}

struct SearchHit {
    project: String,
    session_id: String,
    record: Record,
    line_num: usize,
    matched_query: String,
}

pub fn search(files: &[SessionFile], opts: &SearchOpts) -> Result<()> {
    anyhow::ensure!(!opts.queries.is_empty(), "Search query cannot be empty");
    let matcher = Matcher::new(&opts.queries, opts.is_regex, opts.and_mode)?;

    // Filter files by project and exclude specific sessions
    let filtered_files: Vec<&SessionFile> = files
        .iter()
        .filter(|f| {
            if let Some(proj) = &opts.project {
                if !f.project_name
                    .to_lowercase()
                    .contains(&proj.to_lowercase())
                {
                    return false;
                }
            }
            if let Some(exc) = &opts.exclude_session {
                if f.session_id.starts_with(exc.as_str()) {
                    return false;
                }
            }
            true
        })
        .collect();

    let pb = ProgressBar::new(filtered_files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} files ({msg})")
            .unwrap()
            .progress_chars("█▓░"),
    );

    let hit_count = AtomicUsize::new(0);
    let max = opts.max_results;

    let results: Vec<Vec<SearchHit>> = filtered_files
        .par_iter()
        .map(|file| {
            if max > 0 && hit_count.load(Ordering::Relaxed) >= max {
                pb.inc(1);
                return vec![];
            }

            let hits = search_file(file, &matcher, opts, &hit_count, max);
            pb.inc(1);
            hits
        })
        .collect();

    pb.finish_and_clear();

    // Count mode: aggregate by project
    if opts.count_mode {
        use std::collections::HashMap;
        let mut counts: HashMap<String, usize> = HashMap::new();
        for hits in &results {
            for hit in hits {
                *counts.entry(hit.project.clone()).or_default() += 1;
            }
        }
        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        let total: usize = sorted.iter().map(|(_, c)| c).sum();

        println!("Match counts for '{}'\n", opts.query_display());
        for (project, count) in &sorted {
            println!("  {:40} {:>5}", project, count);
        }
        println!("\n{} total matches across {} projects", total, sorted.len());
        return Ok(());
    }

    // Summary mode: condensed overview
    if opts.summary_mode {
        use std::collections::{HashMap, HashSet};

        let mut project_counts: HashMap<String, usize> = HashMap::new();
        let mut role_counts: HashMap<String, usize> = HashMap::new();
        let mut sessions: HashSet<String> = HashSet::new();
        let mut earliest: Option<String> = None;
        let mut latest: Option<String> = None;
        let mut word_counts: HashMap<String, usize> = HashMap::new();

        // Stop words to skip in topic extraction
        let stop_words: HashSet<&str> = [
            "the", "and", "for", "that", "this", "with", "from", "are", "was",
            "were", "been", "have", "has", "had", "not", "but", "what", "all",
            "can", "her", "his", "one", "our", "out", "you", "your", "which",
            "their", "them", "then", "than", "into", "could", "would", "there",
            "about", "just", "like", "some", "also", "more", "when", "will",
            "each", "make", "way", "she", "how", "its", "may", "use", "used",
            "using", "let", "get", "got", "did", "does", "done", "any", "very",
            "here", "where", "should", "need", "don", "doesn", "isn", "it's",
            "i'll", "i'm", "we're", "they", "it's", "that's", "file", "line",
            "code", "run", "set", "new", "see", "now", "try", "want",
        ].iter().copied().collect();

        for hits in &results {
            for hit in hits {
                *project_counts.entry(hit.project.clone()).or_default() += 1;
                *role_counts.entry(hit.record.role_str().to_string()).or_default() += 1;
                sessions.insert(format!("{}:{}", hit.project, hit.session_id));

                if let Some(msg) = hit.record.as_message_record() {
                    if let Some(ts) = &msg.timestamp {
                        let ts_date = ts.get(..10).unwrap_or(ts).to_string();
                        if earliest.as_ref().map_or(true, |e| ts_date < *e) {
                            earliest = Some(ts_date.clone());
                        }
                        if latest.as_ref().map_or(true, |l| ts_date > *l) {
                            latest = Some(ts_date);
                        }
                    }

                    // Extract topic words
                    let text = msg.text_content();
                    for word in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
                        let w = word.to_lowercase();
                        if w.len() >= 4 && !stop_words.contains(w.as_str()) {
                            *word_counts.entry(w).or_default() += 1;
                        }
                    }
                }
            }
        }

        // Also skip the query terms themselves from topics
        let query_lower: Vec<String> = opts.queries.iter().map(|q| q.to_lowercase()).collect();

        let mut top_words: Vec<_> = word_counts.into_iter()
            .filter(|(w, _)| !query_lower.iter().any(|q| w.contains(q.as_str())))
            .collect();
        top_words.sort_by(|a, b| b.1.cmp(&a.1));

        let total: usize = project_counts.values().sum();

        println!("Summary for '{}'\n", opts.query_display());

        // Projects
        let mut proj_sorted: Vec<_> = project_counts.into_iter().collect();
        proj_sorted.sort_by(|a, b| b.1.cmp(&a.1));
        println!("  Projects:");
        for (project, count) in &proj_sorted {
            println!("    {:38} {:>5} matches", project, count);
        }

        // Roles
        println!("\n  Roles:");
        let mut role_sorted: Vec<_> = role_counts.into_iter().collect();
        role_sorted.sort_by(|a, b| b.1.cmp(&a.1));
        for (role, count) in &role_sorted {
            println!("    {:38} {:>5}", role, count);
        }

        // Date range
        if let (Some(e), Some(l)) = (&earliest, &latest) {
            if e == l {
                println!("\n  Date:     {}", e);
            } else {
                println!("\n  Dates:    {} → {}", e, l);
            }
        }

        // Sessions
        println!("  Sessions: {}", sessions.len());

        // Topics
        let topics: Vec<&str> = top_words.iter().take(10).map(|(w, _)| w.as_str()).collect();
        if !topics.is_empty() {
            println!("\n  Topics:   {}", topics.join(", "));
        }

        println!("\n{} total matches", total);
        return Ok(());
    }

    let mut total = 0;
    let needs_md = opts.stdout_md || opts.md_file.is_some();
    let mut md_lines: Vec<String> = Vec::new();

    for hits in &results {
        for hit in hits {
            if opts.json_mode {
                // Output as JSON line
                print_hit_json(hit);
            } else if !opts.stdout_md {
                display::print_search_hit(
                    &hit.project,
                    &hit.session_id,
                    &hit.record,
                    hit.line_num,
                    &hit.matched_query,
                );
            }

            if needs_md {
                md_lines.push(format_hit_markdown(hit));
            }

            total += 1;
        }
    }

    if !opts.json_mode && !opts.stdout_md {
        if total == 0 {
            println!("No results found for '{}'", opts.query_display());
        } else {
            println!("\n{} results found", total);
        }
    }

    if opts.stdout_md {
        write_markdown_to(&mut std::io::stdout().lock(), opts, &md_lines, total)?;
    }

    if let Some(path) = &opts.md_file {
        let mut f = std::fs::File::create(path)?;
        write_markdown_to(&mut f, opts, &md_lines, total)?;
        eprintln!("Saved to {}", path);
    }

    Ok(())
}

fn format_hit_markdown(hit: &SearchHit) -> String {
    let Some(msg) = hit.record.as_message_record() else {
        return String::new();
    };

    let role = hit.record.role_str();
    let timestamp = msg.timestamp.as_deref().unwrap_or("unknown");
    let ts_short = if timestamp.len() >= 19 {
        &timestamp[..19]
    } else {
        timestamp
    };

    let text = msg.text_content();
    let preview: String = text.chars().take(500).collect();
    let truncated = if text.chars().count() > 500 {
        format!("{}...", preview)
    } else {
        preview
    };

    format!(
        "### {project} — {role} ({ts})\n\n> Session: `{session}` Line: {line}\n\n{content}\n",
        project = hit.project,
        role = role,
        ts = ts_short,
        session = hit.session_id,
        line = hit.line_num,
        content = truncated,
    )
}

fn write_markdown_to(w: &mut dyn std::io::Write, opts: &SearchOpts, hits: &[String], total: usize) -> Result<()> {
    writeln!(w, "# smc Search Results\n")?;
    writeln!(w, "**Query:** `{}`", opts.query_display())?;

    let mut filters = Vec::new();
    if let Some(r) = &opts.role {
        filters.push(format!("role={}", r));
    }
    if let Some(t) = &opts.tool {
        filters.push(format!("tool={}", t));
    }
    if let Some(p) = &opts.project {
        filters.push(format!("project={}", p));
    }
    if let Some(a) = &opts.after {
        filters.push(format!("after={}", a));
    }
    if let Some(b) = &opts.before {
        filters.push(format!("before={}", b));
    }
    if let Some(br) = &opts.branch {
        filters.push(format!("branch={}", br));
    }
    if !filters.is_empty() {
        writeln!(w, "**Filters:** {}", filters.join(", "))?;
    }

    writeln!(w, "**Results:** {}\n", total)?;
    writeln!(w, "---\n")?;

    for hit in hits {
        writeln!(w, "{}", hit)?;
        writeln!(w, "---\n")?;
    }

    Ok(())
}

fn print_hit_json(hit: &SearchHit) {
    let msg = hit.record.as_message_record();
    let text = msg.map(|m| m.text_content()).unwrap_or_default();
    let timestamp = msg
        .and_then(|m| m.timestamp.as_deref())
        .unwrap_or("unknown");
    let role = hit.record.role_str();

    let obj = serde_json::json!({
        "project": hit.project,
        "session_id": hit.session_id,
        "line": hit.line_num,
        "role": role,
        "timestamp": timestamp,
        "matched_query": hit.matched_query,
        "text": text,
    });
    println!("{}", serde_json::to_string(&obj).unwrap_or_default());
}

fn search_file(
    file: &SessionFile,
    matcher: &Matcher,
    opts: &SearchOpts,
    hit_count: &AtomicUsize,
    max: usize,
) -> Vec<SearchHit> {
    let mut hits = Vec::new();

    let Ok(f) = std::fs::File::open(&file.path) else {
        return hits;
    };
    let reader = std::io::BufReader::with_capacity(256 * 1024, f);

    for (line_num, line) in reader.lines().enumerate() {
        if max > 0 && hit_count.load(Ordering::Relaxed) >= max {
            break;
        }

        let Ok(line) = line else { continue };
        if line.trim().is_empty() {
            continue;
        }

        let Ok(record) = serde_json::from_str::<Record>(&line) else {
            continue;
        };

        let Some(msg) = record.as_message_record() else {
            continue;
        };

        // Role filter
        if let Some(role) = &opts.role {
            if record.role_str() != role.as_str() {
                continue;
            }
        }

        // Tool filter
        if let Some(tool_name) = &opts.tool {
            let tools = msg.tool_calls();
            if !tools.iter().any(|t| {
                t.to_lowercase()
                    .contains(&tool_name.to_lowercase())
            }) {
                continue;
            }
        }

        // Date filters
        if let Some(after) = &opts.after {
            if let Some(ts) = &msg.timestamp {
                if ts.as_str() < after.as_str() {
                    continue;
                }
            }
        }
        if let Some(before) = &opts.before {
            if let Some(ts) = &msg.timestamp {
                if ts.as_str() > before.as_str() {
                    continue;
                }
            }
        }

        // Branch filter
        if let Some(branch) = &opts.branch {
            if let Some(gb) = &msg.git_branch {
                if !gb.to_lowercase().contains(&branch.to_lowercase()) {
                    continue;
                }
            } else {
                continue;
            }
        }

        // Skip smc output unless --include-smc
        let text = msg.text_content();
        if !opts.include_smc && text.contains(SMC_TAG_OPEN) {
            continue;
        }

        // Text match
        if let Some(matched) = matcher.first_matching_query(&text) {
            hit_count.fetch_add(1, Ordering::Relaxed);
            hits.push(SearchHit {
                project: file.project_name.clone(),
                session_id: file.session_id.clone(),
                record,
                line_num: line_num + 1,
                matched_query: matched,
            });
        }
    }

    hits
}
