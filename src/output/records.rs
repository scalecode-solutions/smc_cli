/// Shared record types emitted by all subcommands.
use serde::Serialize;

// ── Error / Warning ────────────────────────────────────────────────────────

#[derive(Serialize, Debug)]
pub struct ErrorRecord {
    #[serde(rename = "type")]
    pub record_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    pub message: String,
}

impl ErrorRecord {
    pub fn new(file: Option<impl Into<String>>, message: impl Into<String>) -> Self {
        Self { record_type: "error", file: file.map(Into::into), message: message.into() }
    }

    pub fn warn(file: Option<impl Into<String>>, message: impl Into<String>) -> Self {
        Self { record_type: "warning", file: file.map(Into::into), message: message.into() }
    }
}

// ── Summary ────────────────────────────────────────────────────────────────

#[derive(Serialize, Debug)]
pub struct SummaryRecord {
    #[serde(rename = "type")]
    pub record_type: &'static str,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files_scanned: Option<usize>,
    pub elapsed_ms: u128,
}
