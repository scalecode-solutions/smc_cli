use crate::config::SessionFile;
use crate::display;
use crate::models::{ContentBlock, MessageContent, Record};
use anyhow::Result;
use std::io::BufRead;

pub fn parse_records(file: &SessionFile) -> Result<Vec<Record>> {
    let f = std::fs::File::open(&file.path)?;
    let reader = std::io::BufReader::new(f);
    let mut records = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Record>(&line) {
            Ok(record) => records.push(record),
            Err(_) => continue,
        }
    }

    Ok(records)
}

pub fn list_sessions(
    files: &[SessionFile],
    limit: usize,
    after: Option<&str>,
    before: Option<&str>,
) -> Result<()> {
    let mut entries: Vec<SessionListEntry> = Vec::new();

    for file in files {
        let f = std::fs::File::open(&file.path)?;
        let reader = std::io::BufReader::new(f);

        let mut first_timestamp = None;
        let mut first_user_msg = None;
        let mut msg_count = 0u32;

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let Ok(record) = serde_json::from_str::<Record>(&line) else {
                continue;
            };

            if let Some(msg) = record.as_message_record() {
                msg_count += 1;
                if first_timestamp.is_none() {
                    first_timestamp = msg.timestamp.clone();
                }
                if first_user_msg.is_none() && matches!(record, Record::User(_)) {
                    let text = msg.text_content();
                    first_user_msg = Some(text.chars().take(100).collect::<String>());
                }
            }

            if first_timestamp.is_some() && first_user_msg.is_some() && msg_count > 5 {
                break;
            }
        }

        // Date filters
        if let Some(after_date) = after {
            if let Some(ts) = &first_timestamp {
                if ts.as_str() < after_date {
                    continue;
                }
            }
        }
        if let Some(before_date) = before {
            if let Some(ts) = &first_timestamp {
                if ts.as_str() > before_date {
                    continue;
                }
            }
        }

        entries.push(SessionListEntry {
            file: file.clone(),
            timestamp: first_timestamp,
            preview: first_user_msg,
            msg_count,
        });
    }

    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let show = if limit > 0 {
        std::cmp::min(limit, entries.len())
    } else {
        entries.len()
    };

    println!(
        "{} sessions found (showing {})\n",
        entries.len(),
        show
    );

    for entry in entries.iter().take(show) {
        let ts = entry
            .timestamp
            .as_deref()
            .unwrap_or("unknown")
            .get(..10)
            .unwrap_or("unknown");
        let preview = entry
            .preview
            .as_deref()
            .unwrap_or("[no user message]");

        display::print_session_header(
            &entry.file.project_name,
            &entry.file.session_id,
            &entry.file.size_human(),
        );
        println!("  {} {}", ts.to_string(), preview);
        println!();
    }

    Ok(())
}

pub fn show_session(
    file: &SessionFile,
    show_thinking: bool,
    from: Option<usize>,
    to: Option<usize>,
) -> Result<()> {
    let records = parse_records(file)?;

    println!(
        "Session: {} | Project: {} | Size: {}",
        file.session_id,
        file.project_name,
        file.size_human()
    );
    if from.is_some() || to.is_some() {
        println!(
            "Showing messages {}..{}",
            from.unwrap_or(0),
            to.map_or("end".to_string(), |t| t.to_string())
        );
    }
    println!();

    let mut index = 0;
    for record in &records {
        if !record.is_message() {
            continue;
        }

        let in_range = match (from, to) {
            (Some(f), Some(t)) => index >= f && index <= t,
            (Some(f), None) => index >= f,
            (None, Some(t)) => index <= t,
            (None, None) => true,
        };

        if in_range {
            if !show_thinking {
                // Still show it but we let display handle truncation
            }
            display::print_record(record, index);
        }

        index += 1;

        // Early exit if past range
        if let Some(t) = to {
            if index > t {
                break;
            }
        }
    }

    println!("{}", "─".repeat(80));
    println!("{} messages total, displayed range", index);

    Ok(())
}

pub fn show_tools(file: &SessionFile) -> Result<()> {
    let records = parse_records(file)?;

    println!(
        "Tool calls in session: {} ({})",
        file.session_id, file.project_name
    );
    println!();

    let mut count = 0;
    for record in &records {
        let Some(msg) = record.as_message_record() else {
            continue;
        };

        if let Some(summary) = display::format_tool_summary(msg, record.role_str()) {
            println!("{}", summary);
            count += 1;
        }
    }

    println!("\n{} tool-calling messages", count);
    Ok(())
}

pub fn export_session(file: &SessionFile, to_stdout: bool, md_path: Option<&str>) -> Result<()> {
    use std::io::Write;

    let records = parse_records(file)?;

    let mut content = String::new();
    content.push_str(&format!(
        "# Session: {}\n\n**Project:** {}  \n**Size:** {}\n\n---\n\n",
        file.session_id,
        file.project_name,
        file.size_human()
    ));

    for record in &records {
        let Some(msg) = record.as_message_record() else {
            continue;
        };

        let role = record.role_str();
        let timestamp = msg.timestamp.as_deref().unwrap_or("unknown");
        let ts_short = timestamp.get(..19).unwrap_or(timestamp);

        content.push_str(&format!("## {} ({})\n\n", role.to_uppercase(), ts_short));

        match &msg.message.content {
            MessageContent::Text(s) => {
                content.push_str(s);
                content.push_str("\n\n");
            }
            MessageContent::Blocks(blocks) => {
                for block in blocks {
                    match block {
                        ContentBlock::Text { text } => {
                            content.push_str(text);
                            content.push_str("\n\n");
                        }
                        ContentBlock::Thinking { thinking } => {
                            content.push_str(&format!(
                                "<details>\n<summary>Thinking</summary>\n\n{}\n\n</details>\n\n",
                                thinking
                            ));
                        }
                        ContentBlock::ToolUse { name, input, .. } => {
                            content.push_str(&format!(
                                "**Tool: {}**\n```json\n{}\n```\n\n",
                                name,
                                serde_json::to_string_pretty(input).unwrap_or_else(|_| input.to_string())
                            ));
                        }
                        ContentBlock::ToolResult { content: c, .. } => {
                            if let Some(val) = c {
                                let s = val.to_string();
                                let preview: String = s.chars().take(2000).collect();
                                content.push_str(&format!("**Result:**\n```\n{}\n```\n\n", preview));
                            }
                        }
                        ContentBlock::Other => {}
                    }
                }
            }
        }

        content.push_str("---\n\n");
    }

    if to_stdout {
        print!("{}", content);
    }

    let output_path = if let Some(p) = md_path {
        p.to_string()
    } else if !to_stdout {
        format!("{}.md", &file.session_id[..8.min(file.session_id.len())])
    } else {
        return Ok(());
    };

    if !to_stdout || md_path.is_some() {
        let mut f = std::fs::File::create(&output_path)?;
        f.write_all(content.as_bytes())?;
        eprintln!("Exported to {}", output_path);
    }

    Ok(())
}

pub fn show_context(file: &SessionFile, target_line: usize, context: usize) -> Result<()> {
    let f = std::fs::File::open(&file.path)?;
    let reader = std::io::BufReader::new(f);

    let mut messages: Vec<(usize, Record)> = Vec::new();

    for (line_num, line) in reader.lines().enumerate() {
        let Ok(line) = line else { continue };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Record>(&line) else {
            continue;
        };
        if record.is_message() {
            messages.push((line_num + 1, record));
        }
    }

    // Find the message at or nearest to target_line
    let target_idx = messages
        .iter()
        .position(|(ln, _)| *ln >= target_line)
        .unwrap_or(messages.len().saturating_sub(1));

    let start = target_idx.saturating_sub(context);
    let end = std::cmp::min(messages.len(), target_idx + context + 1);

    println!(
        "Context around line {} in {} ({})\n",
        target_line, file.session_id, file.project_name
    );

    for (i, (line_num, record)) in messages[start..end].iter().enumerate() {
        let is_target = start + i == target_idx;
        if is_target {
            println!("{}", ">>> TARGET <<<".to_string());
        }
        display::print_record(record, *line_num);
    }

    println!("{}", "─".repeat(80));
    println!(
        "Showing messages {} through {} (of {} total)",
        start + 1,
        end,
        messages.len()
    );

    Ok(())
}

pub fn show_recent(
    files: &[SessionFile],
    limit: usize,
    role_filter: Option<&str>,
) -> Result<()> {
    use colored::*;

    #[allow(dead_code)]
    struct RecentMsg {
        project: String,
        session_id: String,
        timestamp: String,
        role: String,
        preview: String,
    }

    let mut all_messages: Vec<RecentMsg> = Vec::new();

    for file in files {
        let f = std::fs::File::open(&file.path)?;
        let reader = std::io::BufReader::new(f);

        // Read last N lines efficiently — read all lines, keep last ones
        let mut last_records: Vec<String> = Vec::new();
        for line in reader.lines() {
            let Ok(line) = line else { continue };
            if line.trim().is_empty() {
                continue;
            }
            last_records.push(line);
            // Keep a buffer — we only need the last few per file
            if last_records.len() > limit * 2 + 50 {
                last_records.drain(..last_records.len() - limit - 25);
            }
        }

        for line in last_records.iter().rev().take(limit + 10) {
            let Ok(record) = serde_json::from_str::<Record>(line) else {
                continue;
            };
            let Some(msg) = record.as_message_record() else {
                continue;
            };

            let role = record.role_str().to_string();
            if let Some(rf) = role_filter {
                if role != rf {
                    continue;
                }
            }

            let ts = msg.timestamp.clone().unwrap_or_default();
            let text = msg.text_content();
            let preview: String = text.chars().take(120).collect();

            all_messages.push(RecentMsg {
                project: file.project_name.clone(),
                session_id: file.session_id.clone(),
                timestamp: ts,
                role,
                preview: preview.replace('\n', " ↵ "),
            });
        }
    }

    // Sort by timestamp descending
    all_messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let show = std::cmp::min(limit, all_messages.len());
    println!("Recent messages (showing {})\n", show);

    for msg in all_messages.iter().take(show) {
        let role_colored = match msg.role.as_str() {
            "user" => "user".green(),
            "assistant" => "asst".blue(),
            _ => msg.role.dimmed(),
        };

        let ts_short = msg.timestamp.get(..19).unwrap_or(&msg.timestamp);

        println!(
            "{} [{}] {} {}",
            msg.project.cyan(),
            role_colored,
            ts_short.dimmed(),
            msg.preview
        );
    }

    Ok(())
}

#[allow(dead_code)]
struct SessionListEntry {
    file: SessionFile,
    timestamp: Option<String>,
    preview: Option<String>,
    msg_count: u32,
}
