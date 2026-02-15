//! Frequency analysis and aggregate statistics across conversation logs.

use crate::config::SessionFile;
use crate::models;
use anyhow::Result;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::collections::HashMap;
use std::io::BufRead;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Format a number with comma separators (e.g., 1,234,567).
pub fn format_count(n: u64) -> String {
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

/// Format bytes into a human-readable string (e.g., "2.85GB").
pub fn format_bytes(bytes: u64) -> String {
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

fn make_progress_bar(len: u64) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} files")
            .unwrap()
            .progress_chars("█▓░"),
    );
    pb
}

/// Print aggregate statistics: total sessions, size, and top projects.
pub fn print_stats(files: &[SessionFile]) -> Result<()> {
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

/// Print all projects with session counts, sizes, and date ranges.
pub fn print_projects(files: &[SessionFile]) -> Result<()> {
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

        if let Ok(f) = std::fs::File::open(&file.path) {
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

/// Character frequency analysis on parsed message content.
pub fn print_freq_chars(files: &[SessionFile]) -> Result<()> {
    let counts: Vec<AtomicU64> = (0..26).map(|_| AtomicU64::new(0)).collect();
    let pb = make_progress_bar(files.len() as u64);

    files.par_iter().for_each(|file| {
        if let Ok(f) = std::fs::File::open(&file.path) {
            let reader = std::io::BufReader::with_capacity(256 * 1024, f);
            for line in reader.lines() {
                let Ok(line) = line else { continue };
                let Ok(record) = serde_json::from_str::<models::Record>(&line) else { continue };
                let Some(msg) = record.as_message_record() else { continue };
                let text = msg.text_content();
                for b in text.bytes() {
                    let idx = match b {
                        b'a'..=b'z' => (b - b'a') as usize,
                        b'A'..=b'Z' => (b - b'A') as usize,
                        _ => continue,
                    };
                    counts[idx].fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        pb.inc(1);
    });

    pb.finish_and_clear();
    print_char_table(&counts, "parsed content", files);
    Ok(())
}

/// Character frequency analysis on raw JSONL bytes.
pub fn print_freq_chars_raw(files: &[SessionFile]) -> Result<()> {
    let counts: Vec<AtomicU64> = (0..26).map(|_| AtomicU64::new(0)).collect();
    let pb = make_progress_bar(files.len() as u64);

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
    print_char_table(&counts, "raw JSONL bytes", files);
    Ok(())
}

fn print_char_table(counts: &[AtomicU64], label: &str, files: &[SessionFile]) {
    let totals: Vec<u64> = counts.iter().map(|c| c.load(Ordering::Relaxed)).collect();
    let max_count = *totals.iter().max().unwrap_or(&1);
    let grand_total: u64 = totals.iter().sum();

    println!("{}", format!("Character Frequency (a-z, case-insensitive, {})", label).bold().cyan());
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
}

/// Word frequency analysis across parsed message content.
pub fn print_freq_words(files: &[SessionFile], limit: usize) -> Result<()> {
    let word_counts: Mutex<HashMap<String, u64>> = Mutex::new(HashMap::new());
    let pb = make_progress_bar(files.len() as u64);

    files.par_iter().for_each(|file| {
        let mut local: HashMap<String, u64> = HashMap::new();
        if let Ok(f) = std::fs::File::open(&file.path) {
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

/// Tool usage frequency analysis.
pub fn print_freq_tools(files: &[SessionFile], limit: usize) -> Result<()> {
    let tool_counts: Mutex<HashMap<String, u64>> = Mutex::new(HashMap::new());
    let pb = make_progress_bar(files.len() as u64);

    files.par_iter().for_each(|file| {
        let mut local: HashMap<String, u64> = HashMap::new();
        if let Ok(f) = std::fs::File::open(&file.path) {
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

/// Message role frequency analysis.
pub fn print_freq_roles(files: &[SessionFile]) -> Result<()> {
    let role_counts: Mutex<HashMap<String, u64>> = Mutex::new(HashMap::new());
    let pb = make_progress_bar(files.len() as u64);

    files.par_iter().for_each(|file| {
        let mut local: HashMap<String, u64> = HashMap::new();
        if let Ok(f) = std::fs::File::open(&file.path) {
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
