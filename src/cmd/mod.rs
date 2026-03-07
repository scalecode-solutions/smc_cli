pub mod search;
pub mod sessions;
pub mod show;
pub mod tools;
pub mod export;
pub mod context;
pub mod stats;
pub mod projects;
pub mod freq;
pub mod recent;

use std::io::BufRead;

use anyhow::Result;

use crate::models::Record;
use crate::util::discover::SessionFile;

/// Parse all records from a session JSONL file.
pub fn parse_records(file: &SessionFile) -> Result<Vec<Record>> {
    let f = std::fs::File::open(&file.path)?;
    let reader = std::io::BufReader::new(f);
    let mut records = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(record) = serde_json::from_str::<Record>(&line) {
            records.push(record);
        }
    }

    Ok(records)
}
