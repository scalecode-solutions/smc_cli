use crate::models::{ContentBlock, MessageContent, MessageRecord, Record};
use colored::*;

pub fn print_record(record: &Record, index: usize) {
    let Some(msg) = record.as_message_record() else {
        return;
    };

    let role = record.role_str();
    let role_colored = match role {
        "user" => "USER".green().bold(),
        "assistant" => "ASSISTANT".blue().bold(),
        "system" => "SYSTEM".yellow().bold(),
        _ => "OTHER".dimmed(),
    };

    let timestamp = msg.timestamp.as_deref().unwrap_or("unknown");
    let ts_short = &timestamp[..std::cmp::min(19, timestamp.len())];

    println!("{}", "─".repeat(80).dimmed());
    print!("[{}] {} ", index, role_colored);
    println!("{}", ts_short.dimmed());

    match &msg.message.content {
        MessageContent::Text(s) => {
            println!("{}", truncate(s, 2000));
        }
        MessageContent::Blocks(blocks) => {
            for block in blocks {
                match block {
                    ContentBlock::Text { text } => {
                        println!("{}", truncate(text, 2000));
                    }
                    ContentBlock::Thinking { thinking } => {
                        println!(
                            "{} {}",
                            "💭".dimmed(),
                            truncate(thinking, 500).dimmed()
                        );
                    }
                    ContentBlock::ToolUse { name, input, .. } => {
                        println!("{} {}", "🔧".yellow(), name.yellow().bold());
                        let input_str = input.to_string();
                        if input_str.len() > 200 {
                            println!("   {}", truncate(&input_str, 200).dimmed());
                        } else {
                            println!("   {}", input_str.dimmed());
                        }
                    }
                    ContentBlock::ToolResult { content, .. } => {
                        if let Some(c) = content {
                            let s = c.to_string();
                            println!(
                                "{} {}",
                                "📋".dimmed(),
                                truncate(&s, 300).dimmed()
                            );
                        }
                    }
                    ContentBlock::Other => {}
                }
            }
        }
    }
}

pub fn print_session_header(project: &str, session_id: &str, size: &str) {
    println!(
        "{} {} {}",
        project.cyan().bold(),
        session_id.dimmed(),
        format!("({})", size).dimmed()
    );
}

pub fn print_search_hit(
    project: &str,
    _session_id: &str,
    record: &Record,
    line_num: usize,
    query: &str,
) {
    let Some(msg) = record.as_message_record() else {
        return;
    };

    let role = record.role_str();
    let role_colored = match role {
        "user" => "user".green(),
        "assistant" => "asst".blue(),
        _ => role.dimmed(),
    };

    let timestamp = msg.timestamp.as_deref().unwrap_or("");
    let ts_short = if timestamp.len() >= 10 {
        &timestamp[..10]
    } else {
        timestamp
    };

    let text = msg.text_content();
    let snippet = extract_snippet(&text, query, 150);

    println!(
        "{}:{} [{}] {} {}",
        project.cyan(),
        format!("L{}", line_num).dimmed(),
        role_colored,
        ts_short.dimmed(),
        highlight_match(&snippet, query),
    );
}

fn extract_snippet(text: &str, query: &str, context_chars: usize) -> String {
    // Use char-based indexing to avoid splitting multi-byte characters
    let text_chars: Vec<char> = text.chars().collect();
    let lower_text: String = text_chars.iter().collect::<String>().to_lowercase();
    let lower_query = query.to_lowercase();

    if let Some(byte_pos) = lower_text.find(&lower_query) {
        // Convert byte position to char position
        let char_pos = lower_text[..byte_pos].chars().count();
        let query_char_len = lower_query.chars().count();

        let half_ctx = context_chars / 2;
        let start = char_pos.saturating_sub(half_ctx);
        let end = std::cmp::min(text_chars.len(), char_pos + query_char_len + half_ctx);

        // Try to align start to a whitespace boundary
        let start = if start > 0 {
            text_chars[..start]
                .iter()
                .rposition(|c| c.is_whitespace())
                .map(|p| p + 1)
                .unwrap_or(start)
        } else {
            0
        };

        let slice: String = text_chars[start..end].iter().collect();

        let mut snippet = String::new();
        if start > 0 {
            snippet.push_str("...");
        }
        snippet.push_str(slice.trim());
        if end < text_chars.len() {
            snippet.push_str("...");
        }
        snippet.replace('\n', " ↵ ")
    } else {
        let end = std::cmp::min(text_chars.len(), context_chars);
        let mut s: String = text_chars[..end].iter().collect();
        if end < text_chars.len() {
            s.push_str("...");
        }
        s.replace('\n', " ↵ ")
    }
}

fn highlight_match(text: &str, query: &str) -> String {
    if query.is_empty() {
        return text.to_string();
    }

    let lower = text.to_lowercase();
    let lower_query = query.to_lowercase();
    let mut result = String::new();
    let mut last_end = 0;

    for (byte_start, matched) in lower.match_indices(&lower_query) {
        if byte_start < last_end {
            continue;
        }
        result.push_str(&text[last_end..byte_start]);
        let byte_end = byte_start + matched.len();
        result.push_str(&format!("{}", &text[byte_start..byte_end].red().bold()));
        last_end = byte_end;
    }
    result.push_str(&text[last_end..]);
    result
}

fn truncate(s: &str, max: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{}...", truncated)
    }
}

pub fn format_tool_summary(msg: &MessageRecord, role: &str) -> Option<String> {
    let tools = msg.tool_calls();
    if tools.is_empty() {
        return None;
    }

    let timestamp = msg.timestamp.as_deref().unwrap_or("");
    let ts_short = if timestamp.len() >= 19 {
        &timestamp[..19]
    } else {
        timestamp
    };

    let tool_list: Vec<String> = tools.iter().map(|t| t.yellow().bold().to_string()).collect();
    Some(format!(
        "  {} {} {}",
        ts_short.dimmed(),
        role.blue(),
        tool_list.join(", ")
    ))
}
