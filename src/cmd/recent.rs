/// smc recent — show most recent messages across all sessions.
use std::io::Write;

use anyhow::Result;
use serde::Serialize;

use crate::models::Record;
use crate::output::Emitter;
use crate::util::discover::SessionFile;

// ── Opts ───────────────────────────────────────────────────────────────────

pub struct RecentOpts {
    pub limit: usize,
    pub role: Option<String>,
    pub project: Option<String>,
    pub max_tokens: usize,
}

// ── Records ────────────────────────────────────────────────────────────────

#[derive(Serialize, Debug)]
struct RecentRecord {
    #[serde(rename = "type")]
    record_type: &'static str,
    project: String,
    session_id: String,
    role: String,
    timestamp: String,
    text: String,
}

// ── run ────────────────────────────────────────────────────────────────────

pub fn run<W: Write>(opts: &RecentOpts, files: &[SessionFile], em: &mut Emitter<W>) -> Result<()> {
    let filtered: Vec<&SessionFile> = files
        .iter()
        .filter(|f| {
            if let Some(proj) = &opts.project {
                f.project_name.to_lowercase().contains(&proj.to_lowercase())
            } else {
                true
            }
        })
        .collect();

    let mut all: Vec<RecentRecord> = Vec::new();

    for file in &filtered {
        let Ok(f) = std::fs::File::open(&file.path) else { continue };

        use std::io::BufRead;
        let reader = std::io::BufReader::new(f);

        let mut last_lines: Vec<String> = Vec::new();
        for line in reader.lines() {
            let Ok(line) = line else { continue };
            if line.trim().is_empty() {
                continue;
            }
            last_lines.push(line);
            if last_lines.len() > opts.limit * 2 + 50 {
                last_lines.drain(..last_lines.len() - opts.limit - 25);
            }
        }

        for line in last_lines.iter().rev().take(opts.limit + 10) {
            let Ok(record) = serde_json::from_str::<Record>(line) else { continue };
            let Some(msg) = record.as_message() else { continue };

            let role = record.role().to_string();
            if let Some(rf) = &opts.role {
                if role != *rf {
                    continue;
                }
            }

            let ts = msg.timestamp.clone().unwrap_or_default();
            let text = msg.text_content();
            let preview: String = text.chars().take(120).collect::<String>().replace('\n', " ");

            all.push(RecentRecord {
                record_type: "recent",
                project: file.project_name.clone(),
                session_id: file.session_id.clone(),
                role,
                timestamp: ts,
                text: preview,
            });
        }
    }

    all.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let show = std::cmp::min(opts.limit, all.len());
    for rec in all.iter().take(show) {
        if !em.emit(rec)? {
            break;
        }
    }

    let summary = crate::output::SummaryRecord {
        record_type: "summary",
        count: show,
        files_scanned: Some(filtered.len()),
        elapsed_ms: 0,
    };
    em.emit(&summary)?;

    em.flush()?;
    Ok(())
}
