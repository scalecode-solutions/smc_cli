use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct Config {
    pub claude_dir: PathBuf,
}

impl Config {
    pub fn new(path_override: Option<&str>) -> Result<Self> {
        let claude_dir = if let Some(p) = path_override {
            PathBuf::from(p)
        } else {
            dirs_fallback()
        };

        anyhow::ensure!(
            claude_dir.exists(),
            "Claude projects directory not found at {}",
            claude_dir.display()
        );

        Ok(Config { claude_dir })
    }

    pub fn discover_jsonl_files(&self) -> Result<Vec<SessionFile>> {
        let mut files = Vec::new();
        let projects_dir = &self.claude_dir;

        if !projects_dir.is_dir() {
            return Ok(files);
        }

        for entry in std::fs::read_dir(projects_dir)? {
            let entry = entry?;
            let project_dir = entry.path();
            if !project_dir.is_dir() {
                continue;
            }

            let project_name = extract_project_name(entry.file_name().to_str().unwrap_or(""));

            for file_entry in std::fs::read_dir(&project_dir)? {
                let file_entry = file_entry?;
                let path = file_entry.path();
                if path.extension().map_or(false, |e| e == "jsonl") && path.is_file() {
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

            // Also check subagents directory
            let subagents_dir = project_dir.join("subagents");
            if subagents_dir.is_dir() {
                // We skip subagent files from top-level discovery but could add them later
            }
        }

        files.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
        Ok(files)
    }
}

fn dirs_fallback() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".claude").join("projects")
}

pub fn extract_project_name(dir_name: &str) -> String {
    // Format: -Users-travis-GitHub-ProjectName or -Users-travis-GitHub-misc-subproject
    let parts: Vec<&str> = dir_name.split('-').collect();

    // Find "GitHub" and take everything after it
    if let Some(pos) = parts.iter().position(|&p| p == "GitHub") {
        let project_parts: Vec<&str> = parts[pos + 1..].iter().copied().collect();
        if project_parts.is_empty() {
            dir_name.to_string()
        } else {
            project_parts.join("/")
        }
    } else {
        // Fallback: take last meaningful segment
        parts
            .iter()
            .filter(|p| !p.is_empty() && *p != &"Users")
            .last()
            .unwrap_or(&dir_name)
            .to_string()
    }
}

#[derive(Debug, Clone)]
pub struct SessionFile {
    pub path: PathBuf,
    pub session_id: String,
    pub project_name: String,
    pub size_bytes: u64,
}

impl SessionFile {
    pub fn size_human(&self) -> String {
        let bytes = self.size_bytes;
        if bytes < 1024 {
            format!("{}B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1}KB", bytes as f64 / 1024.0)
        } else {
            format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
        }
    }
}
