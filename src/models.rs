use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Record {
    User(MessageRecord),
    Assistant(MessageRecord),
    System(MessageRecord),
    FileHistorySnapshot(serde_json::Value),
    Progress(serde_json::Value),
    #[serde(other)]
    Unknown,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRecord {
    pub uuid: Option<String>,
    pub parent_uuid: Option<serde_json::Value>,
    pub session_id: Option<String>,
    pub timestamp: Option<String>,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    pub version: Option<String>,
    pub message: Message,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
    },
    ToolUse {
        id: Option<String>,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: Option<String>,
        content: Option<serde_json::Value>,
    },
    #[serde(other)]
    Other,
}

impl Record {
    pub fn as_message_record(&self) -> Option<&MessageRecord> {
        match self {
            Record::User(r) | Record::Assistant(r) | Record::System(r) => Some(r),
            _ => None,
        }
    }

    pub fn role_str(&self) -> &str {
        match self {
            Record::User(_) => "user",
            Record::Assistant(_) => "assistant",
            Record::System(_) => "system",
            _ => "other",
        }
    }

    pub fn is_message(&self) -> bool {
        matches!(self, Record::User(_) | Record::Assistant(_) | Record::System(_))
    }
}

impl MessageRecord {
    pub fn text_content(&self) -> String {
        match &self.message.content {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Blocks(blocks) => {
                let mut parts = Vec::new();
                for block in blocks {
                    match block {
                        ContentBlock::Text { text } => parts.push(text.clone()),
                        ContentBlock::Thinking { thinking } => parts.push(thinking.clone()),
                        ContentBlock::ToolUse { name, input, .. } => {
                            parts.push(format!("[tool: {}] {}", name, input));
                        }
                        ContentBlock::ToolResult { content, .. } => {
                            if let Some(c) = content {
                                parts.push(format!("[result] {}", c));
                            }
                        }
                        ContentBlock::Other => {}
                    }
                }
                parts.join("\n")
            }
        }
    }

    pub fn tool_calls(&self) -> Vec<&str> {
        match &self.message.content {
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolUse { name, .. } => Some(name.as_str()),
                    _ => None,
                })
                .collect(),
            _ => vec![],
        }
    }

    /// Extract only tool input content (the arguments passed to tools).
    pub fn tool_input_content(&self) -> String {
        match &self.message.content {
            MessageContent::Blocks(blocks) => {
                let mut parts = Vec::new();
                for block in blocks {
                    if let ContentBlock::ToolUse { name, input, .. } = block {
                        parts.push(format!("[{}] {}", name, input));
                    }
                }
                parts.join("\n")
            }
            _ => String::new(),
        }
    }

    /// Extract only thinking block content.
    pub fn thinking_content(&self) -> String {
        match &self.message.content {
            MessageContent::Blocks(blocks) => {
                let mut parts = Vec::new();
                for block in blocks {
                    if let ContentBlock::Thinking { thinking } = block {
                        parts.push(thinking.clone());
                    }
                }
                parts.join("\n")
            }
            _ => String::new(),
        }
    }

    /// Extract text content excluding thinking blocks.
    pub fn text_content_no_thinking(&self) -> String {
        match &self.message.content {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Blocks(blocks) => {
                let mut parts = Vec::new();
                for block in blocks {
                    match block {
                        ContentBlock::Text { text } => parts.push(text.clone()),
                        ContentBlock::ToolUse { name, input, .. } => {
                            parts.push(format!("[tool: {}] {}", name, input));
                        }
                        ContentBlock::ToolResult { content, .. } => {
                            if let Some(c) = content {
                                parts.push(format!("[result] {}", c));
                            }
                        }
                        _ => {}
                    }
                }
                parts.join("\n")
            }
        }
    }

    /// Check if any tool input references a file path (substring match).
    pub fn touches_file(&self, path: &str) -> bool {
        let path_lower = path.to_lowercase();
        match &self.message.content {
            MessageContent::Blocks(blocks) => {
                for block in blocks {
                    match block {
                        ContentBlock::ToolUse { input, .. } => {
                            let s = input.to_string().to_lowercase();
                            if s.contains(&path_lower) {
                                return true;
                            }
                        }
                        ContentBlock::ToolResult { content, .. } => {
                            if let Some(c) = content {
                                let s = c.to_string().to_lowercase();
                                if s.contains(&path_lower) {
                                    return true;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                false
            }
            _ => false,
        }
    }
}
