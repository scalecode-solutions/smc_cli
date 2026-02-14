use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default)]
struct Registry {
    instances: HashMap<String, InstanceInfo>,
}

#[derive(Serialize, Deserialize, Clone)]
struct InstanceInfo {
    pane: String,
    registered_at: String,
    last_message_id: Option<String>,
    #[serde(default)]
    seen_ids: Vec<String>,
}

fn registry_path() -> PathBuf {
    let dir = dirs_path();
    dir.join("relay.json")
}

fn dirs_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".smc");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn load_registry() -> Result<Registry> {
    let path = registry_path();
    if path.exists() {
        let data = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&data).unwrap_or_default())
    } else {
        Ok(Registry::default())
    }
}

fn save_registry(reg: &Registry) -> Result<()> {
    let path = registry_path();
    let data = serde_json::to_string_pretty(reg)?;
    std::fs::write(&path, data)?;
    Ok(())
}

/// Register a Claude instance to a tmux pane
pub fn register(name: &str, pane: Option<&str>) -> Result<()> {
    let pane_id = match pane {
        Some(p) => p.to_string(),
        None => {
            // Auto-detect current tmux pane
            let output = std::process::Command::new("tmux")
                .args(["display-message", "-p", "#{pane_id}"])
                .output()?;
            let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if id.is_empty() {
                anyhow::bail!("Not in a tmux session. Specify --pane manually.");
            }
            id
        }
    };

    let mut reg = load_registry()?;
    let now = chrono::Utc::now().to_rfc3339();

    reg.instances.insert(
        name.to_string(),
        InstanceInfo {
            pane: pane_id.clone(),
            registered_at: now,
            last_message_id: None,
            seen_ids: Vec::new(),
        },
    );

    save_registry(&reg)?;
    println!("Registered '{}' -> tmux pane '{}'", name, pane_id);
    Ok(())
}

/// Unregister a Claude instance
pub fn unregister(name: &str) -> Result<()> {
    let mut reg = load_registry()?;
    if reg.instances.remove(name).is_some() {
        save_registry(&reg)?;
        println!("Unregistered '{}'", name);
    } else {
        println!("'{}' not found in registry", name);
    }
    Ok(())
}

/// Show registered instances
pub fn status() -> Result<()> {
    let reg = load_registry()?;

    if reg.instances.is_empty() {
        println!("No instances registered.");
        println!("\nRegister with: smc relay register <name> [--pane <tmux-pane>]");
        return Ok(());
    }

    println!("Registered instances:\n");
    for (name, info) in &reg.instances {
        let ts = info.registered_at.get(..19).unwrap_or(&info.registered_at);
        let last = info
            .last_message_id
            .as_deref()
            .unwrap_or("none");
        println!(
            "  {:20} pane: {:10} registered: {}  last_msg: {}",
            name, info.pane, ts, last
        );
    }

    Ok(())
}

/// Check for new messages and relay to target
/// Called by the Stop hook after every Claude response
pub fn check(_transcript: Option<&str>) -> Result<()> {
    let reg = load_registry()?;
    if reg.instances.is_empty() {
        return Ok(());
    }

    // Figure out WHO we are so we don't relay messages to ourselves
    // Try TMUX_PANE env var first (per-pane, most reliable)
    // Fall back to tmux display-message (returns active pane — less reliable but better than nothing)
    let my_pane = std::env::var("TMUX_PANE").ok().or_else(|| {
        std::process::Command::new("tmux")
            .args(["display-message", "-p", "#{pane_id}"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    });

    // Find our own name from the registry (match by pane ID)
    let my_name = my_pane.as_ref().and_then(|pane| {
        reg.instances
            .iter()
            .find(|(_, info)| &info.pane == pane)
            .map(|(name, _)| name.clone())
    });

    // Find the most recently modified JSONL files
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let projects_dir = std::path::PathBuf::from(&home).join(".claude/projects");

    let mut jsonl_files: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();
    if let Ok(entries) = walkdir(&projects_dir) {
        for entry in entries {
            if let Ok(meta) = std::fs::metadata(&entry) {
                if let Ok(modified) = meta.modified() {
                    jsonl_files.push((entry, modified));
                }
            }
        }
    }

    // Sort by modification time, check most recent files
    jsonl_files.sort_by(|a, b| b.1.cmp(&a.1));

    for (path, _) in jsonl_files.iter().take(5) {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let lines: Vec<&str> = content.lines().collect();

        // Check last 50 lines for assistant messages with To:/MessageID:
        for line in lines.iter().rev().take(50) {
            let Ok(record) = serde_json::from_str::<crate::models::Record>(line) else {
                continue;
            };

            // Only check assistant messages
            if !matches!(record, crate::models::Record::Assistant(_)) {
                continue;
            }

            let Some(msg) = record.as_message_record() else {
                continue;
            };

            let text = msg.text_content();

            let Some(to_name) = extract_to_field(&text) else {
                continue;
            };

            // SKIP messages addressed to ourselves — we wrote them, don't self-relay
            if let Some(ref me) = my_name {
                if &to_name == me {
                    continue;
                }
            }

            let msg_id = extract_message_id(&text);

            // Check if target is registered
            let Some(target) = reg.instances.get(&to_name) else {
                continue;
            };

            // Check if we already relayed this message
            if let Some(ref new_id) = msg_id {
                if target.seen_ids.contains(new_id) {
                    continue;
                }
            }

            // Relay via tmux send-keys
            let notification = if let Some(ref id) = msg_id {
                format!("new message from the other claude. run: smc search \"{}\"", id)
            } else {
                "new message from the other claude. check smc search".to_string()
            };

            // Type the literal text first (no key interpretation)
            let _ = std::process::Command::new("tmux")
                .args(["send-keys", "-t", &target.pane, "-l", &notification])
                .output();

            // Delay to let the TUI process the text before Enter
            std::thread::sleep(std::time::Duration::from_millis(300));

            // Send Enter separately as a real keypress
            let result = std::process::Command::new("tmux")
                .args(["send-keys", "-t", &target.pane, "Enter"])
                .output();

            if result.is_ok() {
                if let Some(ref id) = msg_id {
                    let mut reg = load_registry()?;
                    if let Some(instance) = reg.instances.get_mut(&to_name) {
                        instance.last_message_id = Some(id.clone());
                        instance.seen_ids.push(id.clone());
                        // Keep only last 100 seen IDs to avoid unbounded growth
                        if instance.seen_ids.len() > 100 {
                            instance.seen_ids = instance.seen_ids.split_off(instance.seen_ids.len() - 100);
                        }
                        save_registry(&reg)?;
                    }
                }
                return Ok(()); // Relayed one message, done
            }
        }
    }

    Ok(())
}

/// Walk directory for .jsonl files
fn walkdir(dir: &std::path::Path) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return Ok(files);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(walkdir(&path)?);
        } else if path.extension().map_or(false, |e| e == "jsonl") {
            files.push(path);
        }
    }
    Ok(files)
}

/// Send a message to a registered Claude instance via tmux
pub fn send(to: &str, message: &str) -> Result<()> {
    let reg = load_registry()?;

    let Some(target) = reg.instances.get(to) else {
        anyhow::bail!(
            "'{}' not registered. Registered instances: {:?}",
            to,
            reg.instances.keys().collect::<Vec<_>>()
        );
    };

    // Type literal text, then send Enter separately
    std::process::Command::new("tmux")
        .args(["send-keys", "-t", &target.pane, "-l", message])
        .output()?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    std::process::Command::new("tmux")
        .args(["send-keys", "-t", &target.pane, "Enter"])
        .output()?;

    println!("Sent to '{}' (pane {})", to, target.pane);
    Ok(())
}

/// Strip all markdown bold markers and trim
fn clean_line(line: &str) -> String {
    line.trim().replace('*', "").trim().to_string()
}

/// Extract "To: <name>" from message text
fn extract_to_field(text: &str) -> Option<String> {
    for line in text.lines() {
        let cleaned = clean_line(line);
        if let Some(rest) = cleaned.strip_prefix("To:") {
            let name = rest.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Extract "MessageID: <id>" from message text
fn extract_message_id(text: &str) -> Option<String> {
    for line in text.lines() {
        let cleaned = clean_line(line);
        if let Some(rest) = cleaned.strip_prefix("MessageID:") {
            let id = rest.trim();
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}
