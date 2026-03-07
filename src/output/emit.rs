/// Emitter<W> — the single output channel for all subcommands.
///
/// # Contract
/// - Every record is emitted as one JSON line (JSONL).
/// - Token budget is tracked; once exhausted `emit()` returns `false`
///   and sets `self.truncated = true`.  Callers should break their loop.
/// - Warnings are emitted inline as `{"type":"warning",...}` records —
///   never to stderr.
/// - `flush()` must be called by the caller before process exit.
use std::io::{BufWriter, Write};

use anyhow::Result;
use serde::Serialize;

use super::records::ErrorRecord;
use crate::util::tokens;

pub struct Emitter<W: Write> {
    out: BufWriter<W>,
    /// Maximum token budget (0 = unlimited).
    budget: usize,
    /// Tokens consumed so far.
    used: usize,
    /// Set when the budget was exhausted and output was cut short.
    pub truncated: bool,
}

impl<W: Write> Emitter<W> {
    pub fn new(writer: W, budget: usize) -> Self {
        Self { out: BufWriter::new(writer), budget, used: 0, truncated: false }
    }

    /// Serialize `rec` as a JSON line and write it.
    /// Returns `Ok(true)` on success, `Ok(false)` when the token budget
    /// has been exhausted (the record was NOT written; caller should stop).
    pub fn emit<T: Serialize>(&mut self, rec: &T) -> Result<bool> {
        let json = serde_json::to_string(rec)?;
        let cost = tokens::approx_line(json.len());
        if self.budget > 0 && self.used + cost > self.budget {
            self.truncated = true;
            return Ok(false);
        }
        self.out.write_all(json.as_bytes())?;
        self.out.write_all(b"\n")?;
        self.used += cost;
        Ok(true)
    }

    /// Emit a `{"type":"warning",...}` record inline.
    /// Never returns an error — file-level warnings must never abort the run.
    pub fn warn(&mut self, file: Option<&str>, msg: &str) {
        let rec = ErrorRecord::warn(file, msg);
        let _ = self.emit(&rec);
    }

    /// Flush the underlying writer.
    pub fn flush(&mut self) -> Result<()> {
        self.out.flush().map_err(Into::into)
    }

    /// Emit a raw text line (not JSON-serialized).
    /// Useful for markdown output modes.
    /// Obeys the token budget the same way `emit()` does.
    pub fn raw(&mut self, line: &str) -> Result<bool> {
        let cost = tokens::approx_line(line.len());
        if self.budget > 0 && self.used + cost > self.budget {
            self.truncated = true;
            return Ok(false);
        }
        self.out.write_all(line.as_bytes())?;
        self.out.write_all(b"\n")?;
        self.used += cost;
        Ok(true)
    }

    /// How many tokens have been emitted so far.
    pub fn tokens_used(&self) -> usize { self.used }
}

// ── Convenience constructors ───────────────────────────────────────────────

impl Emitter<std::io::Stdout> {
    pub fn stdout(budget: usize) -> Self {
        Self::new(std::io::stdout(), budget)
    }
}

impl Emitter<Vec<u8>> {
    pub fn capturing(budget: usize) -> Self {
        Self::new(Vec::new(), budget)
    }

    pub fn into_bytes(mut self) -> Vec<u8> {
        self.out.flush().expect("flush of Vec<u8> cannot fail");
        self.out.into_inner().expect("BufWriter<Vec<u8>> always succeeds")
    }

    pub fn into_records(self) -> Vec<serde_json::Value> {
        let bytes = self.into_bytes();
        bytes
            .split(|&b| b == b'\n')
            .filter(|s| !s.is_empty())
            .filter_map(|s| serde_json::from_slice(s).ok())
            .collect()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn emits_jsonl() {
        let mut em = Emitter::capturing(0);
        em.emit(&json!({"type": "match", "line": 1})).unwrap();
        em.emit(&json!({"type": "match", "line": 2})).unwrap();
        let records = em.into_records();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["line"], 1);
    }

    #[test]
    fn budget_truncates() {
        let mut em = Emitter::capturing(1);
        let big = json!({"type": "x", "data": "aaaa bbbb cccc dddd eeee ffff"});
        let ok = em.emit(&big).unwrap();
        assert!(!ok);
        assert!(em.truncated);
    }

    #[test]
    fn zero_budget_is_unlimited() {
        let mut em = Emitter::capturing(0);
        for i in 0..100 {
            assert!(em.emit(&json!({"n": i})).unwrap());
        }
        assert!(!em.truncated);
    }

    #[test]
    fn warn_emits_warning_record() {
        let mut em = Emitter::capturing(0);
        em.warn(Some("foo.jsonl"), "bad line");
        let records = em.into_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["type"], "warning");
    }
}
