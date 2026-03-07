/// Session file discovery — finds all JSONL conversation logs under ~/.claude/projects.
use std::path::{Path, PathBuf};

use anyhow::Result;

// ── SessionFile ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SessionFile {
    pub path: PathBuf,
    pub session_id: String,
    pub project_name: String,
    pub size_bytes: u64,
}

impl SessionFile {
    pub fn size_human(&self) -> String {
        let b = self.size_bytes;
        if b < 1024 {
            format!("{}B", b)
        } else if b < 1024 * 1024 {
            format!("{:.1}KB", b as f64 / 1024.0)
        } else if b < 1024 * 1024 * 1024 {
            format!("{:.1}MB", b as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2}GB", b as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

// ── Discovery ──────────────────────────────────────────────────────────────

/// Resolve the Claude projects directory.
pub fn claude_dir(path_override: Option<&str>) -> Result<PathBuf> {
    let dir = if let Some(p) = path_override {
        PathBuf::from(p)
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        Path::new(&home).join(".claude").join("projects")
    };
    anyhow::ensure!(dir.exists(), "Claude projects directory not found at {}", dir.display());
    Ok(dir)
}

/// Discover all JSONL session files, sorted largest-first.
pub fn discover_jsonl_files(base: &Path) -> Result<Vec<SessionFile>> {
    let mut files = Vec::new();

    if !base.is_dir() {
        return Ok(files);
    }

    for entry in std::fs::read_dir(base)? {
        let entry = entry?;
        let project_dir = entry.path();
        if !project_dir.is_dir() {
            continue;
        }

        let project_name = extract_project_name(entry.file_name().to_str().unwrap_or(""));

        for file_entry in std::fs::read_dir(&project_dir)? {
            let file_entry = file_entry?;
            let path = file_entry.path();
            if path.extension().is_some_and(|e| e == "jsonl") && path.is_file() {
                let session_id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();

                let metadata = std::fs::metadata(&path)?;

                files.push(SessionFile {
                    path,
                    session_id,
                    project_name: project_name.clone(),
                    size_bytes: metadata.len(),
                });
            }
        }
    }

    files.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    Ok(files)
}

/// Find a session by exact ID or unique prefix.
pub fn find_session<'a>(
    files: &'a [SessionFile],
    query: &str,
) -> Result<&'a SessionFile> {
    if let Some(f) = files.iter().find(|f| f.session_id == query) {
        return Ok(f);
    }
    let matches: Vec<_> = files
        .iter()
        .filter(|f| f.session_id.starts_with(query))
        .collect();
    match matches.len() {
        0 => anyhow::bail!("no session found matching '{}'", query),
        1 => Ok(matches[0]),
        n => anyhow::bail!(
            "ambiguous session ID '{}' ({} matches) — provide more characters",
            query,
            n
        ),
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn extract_project_name(dir_name: &str) -> String {
    let parts: Vec<&str> = dir_name.split('-').collect();

    if let Some(pos) = parts.iter().position(|&p| p == "GitHub") {
        let project_parts = &parts[pos + 1..];
        if project_parts.is_empty() {
            dir_name.to_string()
        } else {
            project_parts.join("/")
        }
    } else {
        parts
            .iter()
            .rfind(|p| !p.is_empty() && **p != "Users")
            .unwrap_or(&dir_name)
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_github_project() {
        assert_eq!(extract_project_name("-Users-travis-GitHub-myapp"), "myapp");
    }

    #[test]
    fn extracts_nested_project() {
        assert_eq!(
            extract_project_name("-Users-travis-GitHub-misc-smc_cli"),
            "misc/smc_cli"
        );
    }

    #[test]
    fn fallback_last_segment() {
        assert_eq!(extract_project_name("-Users-travis-something"), "something");
    }
}
