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
    pub json_mode: bool,
}

impl SearchOpts {
    pub fn query_display(&self) -> String {
        self.queries.join(", ")
    }
}

struct Matcher {
    regexes: Vec<Regex>,
    plains: Vec<String>,
}

impl Matcher {
    fn new(queries: &[String], is_regex: bool) -> Result<Self> {
        if is_regex {
            let regexes = queries
                .iter()
                .map(|q| Regex::new(q))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(Matcher {
                regexes,
                plains: vec![],
            })
        } else {
            Ok(Matcher {
                regexes: vec![],
                plains: queries.iter().map(|q| q.to_lowercase()).collect(),
            })
        }
    }

    fn first_matching_query(&self, text: &str) -> Option<String> {
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
    let matcher = Matcher::new(&opts.queries, opts.is_regex)?;

    // Filter files by project if specified
    let filtered_files: Vec<&SessionFile> = files
        .iter()
        .filter(|f| {
            if let Some(proj) = &opts.project {
                f.project_name
                    .to_lowercase()
                    .contains(&proj.to_lowercase())
            } else {
                true
            }
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

        // Text match
        let text = msg.text_content();
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
