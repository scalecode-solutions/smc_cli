use crate::config::SessionFile;
use crate::display;
use crate::models::Record;
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
            Err(_) => continue, // Skip unparseable lines
        }
    }

    Ok(records)
}

pub fn list_sessions(files: &[SessionFile], limit: usize) -> Result<()> {
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

            // Early exit once we have what we need
            if first_timestamp.is_some() && first_user_msg.is_some() && msg_count > 5 {
                // Count remaining lines approximately
                break;
            }
        }

        entries.push(SessionListEntry {
            file: file.clone(),
            timestamp: first_timestamp,
            preview: first_user_msg,
            msg_count,
        });
    }

    // Sort by timestamp descending
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

pub fn show_session(file: &SessionFile, show_thinking: bool) -> Result<()> {
    let records = parse_records(file)?;

    println!(
        "Session: {} | Project: {} | Size: {}",
        file.session_id,
        file.project_name,
        file.size_human()
    );
    println!();

    let mut index = 0;
    for record in &records {
        if !record.is_message() {
            continue;
        }

        if !show_thinking {
            // Still show it but we let display handle truncation
        }

        display::print_record(record, index);
        index += 1;
    }

    println!("{}", "─".repeat(80));
    println!("{} messages displayed", index);

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

#[allow(dead_code)]
struct SessionListEntry {
    file: SessionFile,
    timestamp: Option<String>,
    preview: Option<String>,
    msg_count: u32,
}
