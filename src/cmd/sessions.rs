/// smc sessions — list conversation sessions with metadata.
use std::io::Write;

use anyhow::Result;
use serde::Serialize;

use crate::models::Record;
use crate::output::Emitter;
use crate::util::discover::SessionFile;

// ── Opts ───────────────────────────────────────────────────────────────────

pub struct SessionsOpts {
    pub limit: usize,
    pub project: Option<String>,
    pub after: Option<String>,
    pub before: Option<String>,
}

// ── Records ────────────────────────────────────────────────────────────────

#[derive(Serialize, Debug)]
struct SessionRecord {
    #[serde(rename = "type")]
    record_type: &'static str,
    session_id: String,
    project: String,
    size_bytes: u64,
    size_human: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preview: Option<String>,
    msg_count: u32,
}

// ── run ────────────────────────────────────────────────────────────────────

pub fn run<W: Write>(opts: &SessionsOpts, files: &[SessionFile], em: &mut Emitter<W>) -> Result<()> {
    let start = std::time::Instant::now();

    let filtered: Vec<&SessionFile> = files
        .iter()
        .filter(|f| {
            if let Some(proj) = &opts.project {
                if !f.project_name.to_lowercase().contains(&proj.to_lowercase()) {
                    return false;
                }
            }
            true
        })
        .collect();

    let mut entries: Vec<SessionRecord> = Vec::new();

    for file in &filtered {
        let Ok(f) = std::fs::File::open(&file.path) else { continue };
        let reader = std::io::BufReader::new(f);

        let mut first_timestamp = None;
        let mut first_user_msg = None;
        let mut msg_count = 0u32;

        use std::io::BufRead;
        for line in reader.lines() {
            let Ok(line) = line else { continue };
            if line.trim().is_empty() {
                continue;
            }
            let Ok(record) = serde_json::from_str::<Record>(&line) else { continue };

            if let Some(msg) = record.as_message() {
                msg_count += 1;
                if first_timestamp.is_none() {
                    first_timestamp = msg.timestamp.clone();
                }
                if first_user_msg.is_none() && matches!(record, Record::User(_)) {
                    let text = msg.text_content();
                    first_user_msg = Some(text.chars().take(120).collect::<String>());
                }
            }

            if first_timestamp.is_some() && first_user_msg.is_some() && msg_count > 5 {
                break;
            }
        }

        // date filters
        if let Some(after) = &opts.after {
            if let Some(ts) = &first_timestamp {
                if ts.as_str() < after.as_str() {
                    continue;
                }
            }
        }
        if let Some(before) = &opts.before {
            if let Some(ts) = &first_timestamp {
                if ts.as_str() > before.as_str() {
                    continue;
                }
            }
        }

        entries.push(SessionRecord {
            record_type: "session",
            session_id: file.session_id.clone(),
            project: file.project_name.clone(),
            size_bytes: file.size_bytes,
            size_human: file.size_human(),
            timestamp: first_timestamp,
            preview: first_user_msg,
            msg_count,
        });
    }

    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let show = if opts.limit > 0 {
        std::cmp::min(opts.limit, entries.len())
    } else {
        entries.len()
    };

    for entry in entries.iter().take(show) {
        if !em.emit(entry)? {
            break;
        }
    }

    let summary = crate::output::SummaryRecord {
        record_type: "summary",
        count: show,
        files_scanned: Some(entries.len()),
        elapsed_ms: start.elapsed().as_millis(),
    };
    em.emit(&summary)?;

    em.flush()?;
    Ok(())
}
