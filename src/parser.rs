use crate::models::{Message, MessageRole, ProjectInfo, SessionInfo};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}

fn claude_projects_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("projects"))
}

/// Decode a project directory name back to the original filesystem path.
///
/// Encoding replaces all `/` and `.` with `-`, so exact recovery is impossible.
/// We use known domain patterns as heuristics. sessions-index.json is preferred
/// when available.
pub(crate) fn decode_project_path(dir_name: &str) -> String {
    if dir_name.is_empty() {
        return dir_name.to_string();
    }

    let encoded = dir_name.trim_start_matches('-');

    let domain_replacements: &[(&str, &str)] = &[
        ("-tech-pepabo-com-", "/tech.pepabo.com/"),
        ("-git-pepabo-com-", "/git.pepabo.com/"),
        ("-github-com-", "/github.com/"),
        ("-gitlab-com-", "/gitlab.com/"),
        ("-bitbucket-org-", "/bitbucket.org/"),
    ];

    let mut encoded = encoded.to_string();
    for (old, new) in domain_replacements {
        encoded = encoded.replace(old, new);
    }

    let domain_endings: &[(&str, &str)] = &[
        ("-tech-pepabo-com", "/tech.pepabo.com"),
        ("-git-pepabo-com", "/git.pepabo.com"),
        ("-github-com", "/github.com"),
        ("-gitlab-com", "/gitlab.com"),
        ("-bitbucket-org", "/bitbucket.org"),
    ];
    for (old, new) in domain_endings {
        if encoded.ends_with(old) {
            let prefix = &encoded[..encoded.len() - old.len()];
            encoded = format!("{}{}", prefix, new);
            break;
        }
    }

    format!("/{}", encoded.replace('-', "/"))
}

/// Try to read originalPath (or projectPath from entries) from sessions-index.json.
fn try_get_original_path(project_dir: &Path) -> Option<String> {
    let index_path = project_dir.join("sessions-index.json");
    let content = fs::read_to_string(&index_path).ok()?;
    let data: Value = serde_json::from_str(&content).ok()?;

    if let Some(orig) = data.get("originalPath").and_then(Value::as_str) {
        return Some(orig.to_string());
    }

    if let Some(entries) = data.get("entries").and_then(Value::as_array) {
        if let Some(first) = entries.first() {
            if let Some(pp) = first.get("projectPath").and_then(Value::as_str) {
                return Some(pp.to_string());
            }
        }
    }

    None
}

/// Parse an ISO 8601 timestamp string (e.g. "2026-01-30T03:17:44.781Z") into DateTime<Utc>.
pub(crate) fn parse_timestamp(ts: Option<&str>) -> Option<DateTime<Utc>> {
    let ts = ts?;
    if ts.is_empty() {
        return None;
    }
    // chrono can parse RFC 3339 / ISO 8601 directly
    ts.parse::<DateTime<Utc>>().ok()
}

/// Extract text from content which can be a string or an array of content blocks.
/// Thinking blocks are excluded; only "text" blocks are concatenated.
pub(crate) fn extract_text_from_content(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(arr) => {
            let texts: Vec<&str> = arr
                .iter()
                .filter_map(|block| {
                    if block.get("type")?.as_str()? == "text" {
                        block.get("text")?.as_str()
                    } else {
                        None
                    }
                })
                .collect();
            texts.join("\n")
        }
        _ => String::new(),
    }
}

/// Extract tool_use / tool_result blocks from a content array.
pub(crate) fn extract_tool_blocks(content: &Value) -> Vec<&Value> {
    match content {
        Value::Array(arr) => arr
            .iter()
            .filter(|block| {
                if let Some(t) = block.get("type").and_then(Value::as_str) {
                    t == "tool_use" || t == "tool_result"
                } else {
                    false
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Create a human-readable summary of a tool use invocation.
pub(crate) fn summarize_tool_use(tool_name: &str, input: &Value) -> String {
    match tool_name {
        "Bash" => {
            let desc = input
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("");
            if !desc.is_empty() {
                return format!("[Bash] {}", desc);
            }
            let cmd = input.get("command").and_then(Value::as_str).unwrap_or("");
            if cmd.len() > 100 {
                format!("[Bash] {}...", &cmd[..100])
            } else {
                format!("[Bash] {}", cmd)
            }
        }
        "Read" => {
            let fp = input
                .get("file_path")
                .and_then(Value::as_str)
                .unwrap_or("");
            format!("[Read] {}", fp)
        }
        "Write" => {
            let fp = input
                .get("file_path")
                .and_then(Value::as_str)
                .unwrap_or("");
            format!("[Write] {}", fp)
        }
        "Edit" => {
            let fp = input
                .get("file_path")
                .and_then(Value::as_str)
                .unwrap_or("");
            format!("[Edit] {}", fp)
        }
        "Grep" => {
            let pattern = input.get("pattern").and_then(Value::as_str).unwrap_or("");
            let path = input.get("path").and_then(Value::as_str).unwrap_or(".");
            format!("[Grep] {} in {}", pattern, path)
        }
        "Glob" => {
            let pattern = input.get("pattern").and_then(Value::as_str).unwrap_or("");
            format!("[Glob] {}", pattern)
        }
        "WebFetch" => {
            let url = input.get("url").and_then(Value::as_str).unwrap_or("");
            format!("[WebFetch] {}", url)
        }
        _ => format!("[{}]", tool_name),
    }
}

/// List all projects under ~/.claude/projects/.
pub fn list_projects() -> Result<Vec<ProjectInfo>> {
    let projects_dir = match claude_projects_dir() {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };
    list_projects_in(&projects_dir)
}

pub(crate) fn list_projects_in(projects_dir: &Path) -> Result<Vec<ProjectInfo>> {
    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<_> = fs::read_dir(projects_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .collect();

    entries.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    let mut projects = Vec::new();
    for entry in entries {
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let dir_path = entry.path();

        let original_path = try_get_original_path(&dir_path)
            .unwrap_or_else(|| decode_project_path(&dir_name));

        let session_count = fs::read_dir(&dir_path)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "jsonl")
                            .unwrap_or(false)
                    })
                    .count()
            })
            .unwrap_or(0);

        projects.push(ProjectInfo {
            dir_name,
            original_path,
            session_count,
        });
    }

    Ok(projects)
}

/// List sessions for a given project.
///
/// Prefers sessions-index.json when available; falls back to scanning .jsonl files.
pub fn list_sessions(project_name: &str) -> Result<Vec<SessionInfo>> {
    let projects_dir = match claude_projects_dir() {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };
    list_sessions_in(project_name, &projects_dir)
}

pub(crate) fn list_sessions_in(project_name: &str, projects_dir: &Path) -> Result<Vec<SessionInfo>> {
    let project_dir = projects_dir.join(project_name);
    if !project_dir.exists() {
        return Ok(Vec::new());
    }

    let index_path = project_dir.join("sessions-index.json");
    if index_path.exists() {
        let sessions = list_sessions_from_index(project_name, &index_path);
        if !sessions.is_empty() {
            return Ok(sessions);
        }
    }

    Ok(list_sessions_from_files(project_name, &project_dir))
}

/// Parse a single entry from sessions-index.json into a SessionInfo.
pub(crate) fn parse_index_entry(entry: &Value, project_name: &str) -> SessionInfo {
    let session_id = entry
        .get("sessionId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let preview = truncate_str(
        entry
            .get("firstPrompt")
            .and_then(Value::as_str)
            .unwrap_or(""),
        200,
    );
    let timestamp = parse_timestamp(entry.get("created").and_then(Value::as_str));
    let message_count = entry
        .get("messageCount")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let git_branch = entry
        .get("gitBranch")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let summary = entry
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    SessionInfo {
        session_id,
        project_name: project_name.to_string(),
        preview,
        timestamp,
        message_count,
        git_branch,
        summary,
    }
}

fn list_sessions_from_index(project_name: &str, index_path: &Path) -> Vec<SessionInfo> {
    let content = match fs::read_to_string(index_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let data: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let entries = match data.get("entries").and_then(Value::as_array) {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    let mut sessions: Vec<SessionInfo> = entries
        .iter()
        .map(|entry| parse_index_entry(entry, project_name))
        .collect();

    // Sort by timestamp descending (newest first)
    sessions.sort_by(|a, b| {
        let ta = a.timestamp.unwrap_or(DateTime::<Utc>::MIN_UTC);
        let tb = b.timestamp.unwrap_or(DateTime::<Utc>::MIN_UTC);
        tb.cmp(&ta)
    });

    sessions
}

fn list_sessions_from_files(project_name: &str, project_dir: &Path) -> Vec<SessionInfo> {
    let mut sessions = Vec::new();

    let entries = match fs::read_dir(project_dir) {
        Ok(rd) => rd,
        Err(_) => return sessions,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
            let session_id = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            let mut preview = String::new();
            let mut timestamp: Option<DateTime<Utc>> = None;
            let mut git_branch = String::new();
            let mut message_count: usize = 0;

            if let Ok(content) = fs::read_to_string(&path) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    let obj: Value = match serde_json::from_str(line) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let msg_type = obj.get("type").and_then(Value::as_str).unwrap_or("");
                    if msg_type == "user" || msg_type == "assistant" {
                        message_count += 1;
                    }

                    if msg_type == "user" && preview.is_empty() {
                        let msg_content = obj
                            .get("message")
                            .and_then(|m| m.get("content"))
                            .cloned()
                            .unwrap_or(Value::String(String::new()));
                        preview = truncate_str(&extract_text_from_content(&msg_content), 200);
                        timestamp =
                            parse_timestamp(obj.get("timestamp").and_then(Value::as_str));
                        git_branch = obj
                            .get("gitBranch")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                    }
                }
            }

            sessions.push(SessionInfo {
                session_id,
                project_name: project_name.to_string(),
                preview,
                timestamp,
                message_count,
                git_branch,
                summary: String::new(),
            });
        }
    }

    // Sort by timestamp descending
    sessions.sort_by(|a, b| {
        let ta = a.timestamp.unwrap_or(DateTime::<Utc>::MIN_UTC);
        let tb = b.timestamp.unwrap_or(DateTime::<Utc>::MIN_UTC);
        tb.cmp(&ta)
    });

    sessions
}

/// Load all messages from a session JSONL file.
pub fn load_session(project_name: &str, session_id: &str) -> Result<Vec<Message>> {
    let projects_dir = match claude_projects_dir() {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };
    load_session_in(project_name, session_id, &projects_dir)
}

pub(crate) fn load_session_in(project_name: &str, session_id: &str, projects_dir: &Path) -> Result<Vec<Message>> {
    let jsonl_path = projects_dir
        .join(project_name)
        .join(format!("{}.jsonl", session_id));

    if !jsonl_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&jsonl_path)?;
    Ok(content.lines().flat_map(parse_jsonl_line).collect())
}

/// Parse a single JSONL line into zero or more Messages.
///
/// Returns an empty Vec for blank lines, parse errors, or unknown message types.
pub(crate) fn parse_jsonl_line(line: &str) -> Vec<Message> {
    let line = line.trim();
    if line.is_empty() {
        return Vec::new();
    }
    let obj: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let msg_type = obj.get("type").and_then(Value::as_str).unwrap_or("");
    let timestamp = parse_timestamp(obj.get("timestamp").and_then(Value::as_str));

    match msg_type {
        "user" => {
            let msg_content = obj
                .get("message")
                .and_then(|m| m.get("content"))
                .cloned()
                .unwrap_or(Value::String(String::new()));

            let mut messages = Vec::new();
            if msg_content.is_array() {
                let tool_blocks = extract_tool_blocks(&msg_content);
                if !tool_blocks.is_empty() {
                    for block in tool_blocks {
                        if block.get("type").and_then(Value::as_str) == Some("tool_result") {
                            let result_content = block
                                .get("content")
                                .cloned()
                                .unwrap_or(Value::String(String::new()));
                            let result_text = if result_content.is_array() {
                                extract_text_from_content(&result_content)
                            } else {
                                match &result_content {
                                    Value::String(s) => s.clone(),
                                    other => other.to_string(),
                                }
                            };
                            messages.push(Message {
                                role: MessageRole::ToolResult,
                                text: result_text,
                                timestamp,
                                tool_name: None,
                            });
                        }
                    }
                } else {
                    // Text-only user message with array content
                    let text = extract_text_from_content(&msg_content);
                    if !text.is_empty() {
                        messages.push(Message {
                            role: MessageRole::User,
                            text,
                            timestamp,
                            tool_name: None,
                        });
                    }
                }
            } else {
                let text = extract_text_from_content(&msg_content);
                if !text.is_empty() {
                    messages.push(Message {
                        role: MessageRole::User,
                        text,
                        timestamp,
                        tool_name: None,
                    });
                }
            }
            messages
        }
        "assistant" => {
            let msg_content = obj
                .get("message")
                .and_then(|m| m.get("content"))
                .cloned()
                .unwrap_or(Value::String(String::new()));

            let mut messages = Vec::new();

            // Extract text portion
            let text = extract_text_from_content(&msg_content);
            if !text.is_empty() {
                messages.push(Message {
                    role: MessageRole::Assistant,
                    text,
                    timestamp,
                    tool_name: None,
                });
            }

            // Extract tool_use blocks as separate messages
            if let Value::Array(arr) = &msg_content {
                for block in arr {
                    if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                        let tool_name = block
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let tool_input = block
                            .get("input")
                            .cloned()
                            .unwrap_or(Value::Object(serde_json::Map::new()));
                        let summary = summarize_tool_use(&tool_name, &tool_input);
                        messages.push(Message {
                            role: MessageRole::ToolUse,
                            text: summary,
                            timestamp,
                            tool_name: Some(tool_name),
                        });
                    }
                }
            }
            messages
        }
        "system" => {
            let subtype = obj
                .get("subtype")
                .and_then(Value::as_str)
                .unwrap_or("");
            let raw_content = obj
                .get("message")
                .and_then(|m| m.get("content"))
                .cloned()
                .unwrap_or(Value::Null);

            let text = match &raw_content {
                Value::String(s) => s.clone(),
                Value::Array(_) | Value::Object(_) => extract_text_from_content(&raw_content),
                _ => String::new(),
            };

            let text = if text.is_empty() {
                if subtype.is_empty() {
                    "[system]".to_string()
                } else {
                    format!("[system: {}]", subtype)
                }
            } else {
                text
            };

            vec![Message {
                role: MessageRole::System,
                text,
                timestamp,
                tool_name: None,
            }]
        }
        _ => {
            // Skip unknown types (e.g. "file-history-snapshot", "progress")
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MessageRole;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    // ================================================================
    // truncate_str
    // ================================================================

    #[test]
    fn truncate_str_short() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn truncate_str_exact() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn truncate_str_long() {
        assert_eq!(truncate_str("hello world", 5), "hello...");
    }

    #[test]
    fn truncate_str_multibyte() {
        // 3 chars, limit 2 -> "ab..."  equivalent with Japanese
        assert_eq!(truncate_str("abcdef", 3), "abc...");
        // Japanese characters
        let jp = "こんにちは世界";
        let result = truncate_str(jp, 3);
        assert_eq!(result, "こんに...");
    }

    // ================================================================
    // decode_project_path
    // ================================================================

    #[test]
    fn decode_project_path_github() {
        let input = "-Users-foo-src-github-com-org-repo";
        let result = decode_project_path(input);
        assert_eq!(result, "/Users/foo/src/github.com/org/repo");
    }

    #[test]
    fn decode_project_path_gitlab() {
        let input = "-Users-foo-src-gitlab-com-org-repo";
        let result = decode_project_path(input);
        assert_eq!(result, "/Users/foo/src/gitlab.com/org/repo");
    }

    #[test]
    fn decode_project_path_empty() {
        assert_eq!(decode_project_path(""), "");
    }

    // ================================================================
    // parse_timestamp
    // ================================================================

    #[test]
    fn parse_timestamp_valid() {
        let result = parse_timestamp(Some("2024-01-15T10:30:00Z"));
        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-01-15");
    }

    #[test]
    fn parse_timestamp_none() {
        assert!(parse_timestamp(None).is_none());
    }

    #[test]
    fn parse_timestamp_empty() {
        assert!(parse_timestamp(Some("")).is_none());
    }

    #[test]
    fn parse_timestamp_invalid() {
        assert!(parse_timestamp(Some("invalid")).is_none());
    }

    // ================================================================
    // extract_text_from_content
    // ================================================================

    #[test]
    fn extract_text_from_content_string() {
        let v = json!("hello");
        assert_eq!(extract_text_from_content(&v), "hello");
    }

    #[test]
    fn extract_text_from_content_array_text_and_thinking() {
        let v = json!([
            {"type": "thinking", "thinking": "hmm"},
            {"type": "text", "text": "answer1"},
            {"type": "text", "text": "answer2"}
        ]);
        assert_eq!(extract_text_from_content(&v), "answer1\nanswer2");
    }

    #[test]
    fn extract_text_from_content_null() {
        let v = json!(null);
        assert_eq!(extract_text_from_content(&v), "");
    }

    // ================================================================
    // extract_tool_blocks
    // ================================================================

    #[test]
    fn extract_tool_blocks_mixed() {
        let v = json!([
            {"type": "text", "text": "hello"},
            {"type": "tool_use", "name": "Bash"},
            {"type": "tool_result", "content": "ok"}
        ]);
        let blocks = extract_tool_blocks(&v);
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn extract_tool_blocks_non_array() {
        let v = json!("hello");
        assert!(extract_tool_blocks(&v).is_empty());
    }

    // ================================================================
    // summarize_tool_use
    // ================================================================

    #[test]
    fn summarize_tool_use_bash_with_description() {
        let input = json!({"description": "List files", "command": "ls -la"});
        assert_eq!(summarize_tool_use("Bash", &input), "[Bash] List files");
    }

    #[test]
    fn summarize_tool_use_bash_no_description() {
        let input = json!({"command": "ls -la"});
        assert_eq!(summarize_tool_use("Bash", &input), "[Bash] ls -la");
    }

    #[test]
    fn summarize_tool_use_read() {
        let input = json!({"file_path": "/path/to/file"});
        assert_eq!(summarize_tool_use("Read", &input), "[Read] /path/to/file");
    }

    #[test]
    fn summarize_tool_use_unknown() {
        let input = json!({});
        assert_eq!(summarize_tool_use("CustomTool", &input), "[CustomTool]");
    }

    // ================================================================
    // parse_jsonl_line
    // ================================================================

    #[test]
    fn parse_jsonl_line_user_message() {
        let line = r#"{"type":"user","timestamp":"2024-01-15T10:30:00Z","message":{"content":"hello"}}"#;
        let msgs = parse_jsonl_line(line);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role, MessageRole::User);
        assert_eq!(msgs[0].text, "hello");
    }

    #[test]
    fn parse_jsonl_line_assistant_text() {
        let line = r#"{"type":"assistant","timestamp":"2024-01-15T10:30:00Z","message":{"content":"response"}}"#;
        let msgs = parse_jsonl_line(line);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role, MessageRole::Assistant);
        assert_eq!(msgs[0].text, "response");
    }

    #[test]
    fn parse_jsonl_line_assistant_with_tool_use() {
        let line = r#"{"type":"assistant","timestamp":"2024-01-15T10:30:00Z","message":{"content":[{"type":"text","text":"Let me check"},{"type":"tool_use","name":"Read","input":{"file_path":"/tmp/test.txt"}}]}}"#;
        let msgs = parse_jsonl_line(line);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, MessageRole::Assistant);
        assert_eq!(msgs[0].text, "Let me check");
        assert_eq!(msgs[1].role, MessageRole::ToolUse);
        assert!(msgs[1].text.contains("[Read]"));
    }

    #[test]
    fn parse_jsonl_line_system() {
        let line = r#"{"type":"system","subtype":"init","message":{"content":"System started"}}"#;
        let msgs = parse_jsonl_line(line);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role, MessageRole::System);
        assert_eq!(msgs[0].text, "System started");
    }

    #[test]
    fn parse_jsonl_line_empty() {
        assert!(parse_jsonl_line("").is_empty());
        assert!(parse_jsonl_line("   ").is_empty());
    }

    #[test]
    fn parse_jsonl_line_invalid_json() {
        assert!(parse_jsonl_line("{invalid json}").is_empty());
    }

    #[test]
    fn parse_jsonl_line_unknown_type() {
        let line = r#"{"type":"progress","data":{}}"#;
        assert!(parse_jsonl_line(line).is_empty());
    }

    // ================================================================
    // parse_index_entry
    // ================================================================

    #[test]
    fn parse_index_entry_full() {
        let entry = json!({
            "sessionId": "abc-123",
            "firstPrompt": "Hello world",
            "created": "2024-01-15T10:30:00Z",
            "messageCount": 5,
            "gitBranch": "main",
            "summary": "Test session"
        });
        let info = parse_index_entry(&entry, "my-project");
        assert_eq!(info.session_id, "abc-123");
        assert_eq!(info.preview, "Hello world");
        assert!(info.timestamp.is_some());
        assert_eq!(info.message_count, 5);
        assert_eq!(info.git_branch, "main");
        assert_eq!(info.summary, "Test session");
        assert_eq!(info.project_name, "my-project");
    }

    #[test]
    fn parse_index_entry_missing_fields() {
        let entry = json!({});
        let info = parse_index_entry(&entry, "proj");
        assert_eq!(info.session_id, "");
        assert_eq!(info.preview, "");
        assert!(info.timestamp.is_none());
        assert_eq!(info.message_count, 0);
        assert_eq!(info.git_branch, "");
        assert_eq!(info.summary, "");
    }

    // ================================================================
    // I/O integration tests (tempfile)
    // ================================================================

    #[test]
    fn list_projects_in_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let result = list_projects_in(tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn list_projects_in_with_one_project() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("-Users-foo-src-github-com-org-repo");
        fs::create_dir(&project_dir).unwrap();
        // Create a .jsonl file so session_count > 0
        fs::write(project_dir.join("session1.jsonl"), "").unwrap();

        let result = list_projects_in(tmp.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].dir_name, "-Users-foo-src-github-com-org-repo");
        assert_eq!(result[0].session_count, 1);
    }

    #[test]
    fn list_projects_in_nonexistent_dir() {
        let tmp = TempDir::new().unwrap();
        let nonexistent = tmp.path().join("does-not-exist");
        let result = list_projects_in(&nonexistent).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn list_sessions_in_from_jsonl_files() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("my-project");
        fs::create_dir(&project_dir).unwrap();

        let jsonl_content = r#"{"type":"user","timestamp":"2024-01-15T10:30:00Z","message":{"content":"hello"}}
{"type":"assistant","timestamp":"2024-01-15T10:31:00Z","message":{"content":"hi there"}}"#;
        fs::write(project_dir.join("session-abc.jsonl"), jsonl_content).unwrap();

        let result = list_sessions_in("my-project", tmp.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].session_id, "session-abc");
        assert_eq!(result[0].message_count, 2);
        assert_eq!(result[0].preview, "hello");
    }

    #[test]
    fn list_sessions_in_from_index() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("my-project");
        fs::create_dir(&project_dir).unwrap();

        let index = json!({
            "entries": [
                {
                    "sessionId": "sess-1",
                    "firstPrompt": "First prompt",
                    "created": "2024-01-15T10:30:00Z",
                    "messageCount": 3,
                    "gitBranch": "main",
                    "summary": "A session"
                }
            ]
        });
        fs::write(
            project_dir.join("sessions-index.json"),
            serde_json::to_string(&index).unwrap(),
        )
        .unwrap();

        let result = list_sessions_in("my-project", tmp.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].session_id, "sess-1");
        assert_eq!(result[0].preview, "First prompt");
        assert_eq!(result[0].message_count, 3);
    }

    #[test]
    fn load_session_in_normal() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("my-project");
        fs::create_dir(&project_dir).unwrap();

        let jsonl_content = r#"{"type":"user","timestamp":"2024-01-15T10:30:00Z","message":{"content":"hello"}}
{"type":"assistant","timestamp":"2024-01-15T10:31:00Z","message":{"content":"hi there"}}"#;
        fs::write(project_dir.join("sess-1.jsonl"), jsonl_content).unwrap();

        let msgs = load_session_in("my-project", "sess-1", tmp.path()).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, MessageRole::User);
        assert_eq!(msgs[0].text, "hello");
        assert_eq!(msgs[1].role, MessageRole::Assistant);
        assert_eq!(msgs[1].text, "hi there");
    }

    #[test]
    fn load_session_in_nonexistent_file() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("my-project");
        fs::create_dir(&project_dir).unwrap();

        let msgs = load_session_in("my-project", "nonexistent", tmp.path()).unwrap();
        assert!(msgs.is_empty());
    }
}
