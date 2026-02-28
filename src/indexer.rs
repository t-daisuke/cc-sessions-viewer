use crate::index::{PromptRecord, SessionIndex, SessionRecord};
use crate::parser;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub fn default_db_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|c| c.join("cc-sessions-viewer").join("index.db"))
}

fn default_projects_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("projects"))
}

pub fn build_index(db_path: &Path, projects_dir: &Path) -> Result<()> {
    let index = SessionIndex::open(db_path)?;

    if !projects_dir.exists() {
        return Ok(());
    }

    let project_dirs: Vec<_> = fs::read_dir(projects_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .collect();

    for project_entry in &project_dirs {
        let dir_name = project_entry.file_name().to_string_lossy().to_string();
        let project_dir = project_entry.path();

        let index_metadata = read_index_metadata(&project_dir);

        let jsonl_files: Vec<_> = fs::read_dir(&project_dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "jsonl")
                    .unwrap_or(false)
            })
            .collect();

        for jsonl_entry in &jsonl_files {
            let path = jsonl_entry.path();
            let session_id = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            let file_mtime = fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);

            if let Some(stored_mtime) = index.get_file_mtime(&session_id)? {
                if stored_mtime == file_mtime {
                    continue;
                }
            }

            let meta = index_metadata.get(&session_id);

            let project_path = meta
                .and_then(|m| m.project_path.clone())
                .unwrap_or_else(|| parser::decode_project_path(&dir_name));
            let git_branch = meta.map(|m| m.git_branch.clone()).unwrap_or_default();
            let summary = meta.map(|m| m.summary.clone()).unwrap_or_default();
            let first_prompt_meta = meta.map(|m| m.first_prompt.clone()).unwrap_or_default();
            let message_count = meta.map(|m| m.message_count).unwrap_or(0);
            let created_at = meta.map(|m| m.created_at.clone()).unwrap_or_default();
            let modified_at = meta.map(|m| m.modified_at.clone()).unwrap_or_default();

            let prompts = extract_user_prompts(&path);

            let first_prompt = if first_prompt_meta.is_empty() {
                prompts
                    .first()
                    .map(|p| p.prompt.clone())
                    .unwrap_or_default()
            } else {
                first_prompt_meta
            };

            index.upsert_session(&SessionRecord {
                session_id: session_id.clone(),
                project_path,
                dir_name: dir_name.clone(),
                git_branch,
                summary,
                first_prompt,
                message_count,
                created_at,
                modified_at,
                file_mtime,
            })?;

            index.insert_prompts(&session_id, &prompts)?;
        }
    }

    Ok(())
}

pub fn build_default_index() -> Result<PathBuf> {
    let db_path =
        default_db_path().ok_or_else(|| anyhow::anyhow!("Could not determine cache directory"))?;
    let projects_dir = default_projects_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    build_index(&db_path, &projects_dir)?;
    Ok(db_path)
}

struct IndexEntryMeta {
    project_path: Option<String>,
    git_branch: String,
    summary: String,
    first_prompt: String,
    message_count: i64,
    created_at: String,
    modified_at: String,
}

fn read_index_metadata(project_dir: &Path) -> std::collections::HashMap<String, IndexEntryMeta> {
    let mut map = std::collections::HashMap::new();
    let index_path = project_dir.join("sessions-index.json");
    let content = match fs::read_to_string(&index_path) {
        Ok(c) => c,
        Err(_) => return map,
    };
    let data: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return map,
    };
    if let Some(entries) = data.get("entries").and_then(|v| v.as_array()) {
        for entry in entries {
            let session_id = entry
                .get("sessionId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if session_id.is_empty() {
                continue;
            }
            map.insert(
                session_id,
                IndexEntryMeta {
                    project_path: entry
                        .get("projectPath")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    git_branch: entry
                        .get("gitBranch")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    summary: entry
                        .get("summary")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    first_prompt: entry
                        .get("firstPrompt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    message_count: entry
                        .get("messageCount")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    created_at: entry
                        .get("created")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    modified_at: entry
                        .get("modified")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                },
            );
        }
    }
    map
}

fn extract_user_prompts(jsonl_path: &Path) -> Vec<PromptRecord> {
    let content = match fs::read_to_string(jsonl_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let mut prompts = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let obj: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if obj.get("type").and_then(|v| v.as_str()) != Some("user") {
            continue;
        }
        let msg_content = obj
            .get("message")
            .and_then(|m| m.get("content"))
            .cloned()
            .unwrap_or(serde_json::Value::String(String::new()));
        let text = parser::extract_text_from_content(&msg_content);
        if text.is_empty() {
            continue;
        }
        let timestamp = obj
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(String::from);
        prompts.push(PromptRecord {
            prompt: text,
            timestamp,
        });
    }
    prompts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::SessionIndex;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn build_index_from_single_session() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let projects_dir = tmp.path().join("projects");
        let project_dir = projects_dir.join("-Users-foo-src-github-com-org-repo");
        fs::create_dir_all(&project_dir).unwrap();

        let jsonl = r#"{"type":"user","timestamp":"2026-01-15T10:00:00Z","message":{"content":"Hello world"}}
{"type":"assistant","timestamp":"2026-01-15T10:01:00Z","message":{"content":"Hi there"}}
{"type":"user","timestamp":"2026-01-15T10:02:00Z","message":{"content":"How are you?"}}"#;
        fs::write(project_dir.join("sess-abc.jsonl"), jsonl).unwrap();

        build_index(&db_path, &projects_dir).unwrap();

        let index = SessionIndex::open(&db_path).unwrap();
        let results = index.search_all().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].session_id, "sess-abc");
        assert_eq!(results[0].dir_name, "-Users-foo-src-github-com-org-repo");
        assert_eq!(
            results[0].project_path,
            "/Users/foo/src/github.com/org/repo"
        );
        assert_eq!(results[0].prompts.len(), 2);
        assert_eq!(results[0].prompts[0], "Hello world");
        assert_eq!(results[0].prompts[1], "How are you?");
    }

    #[test]
    fn incremental_update_skips_unchanged() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let projects_dir = tmp.path().join("projects");
        let project_dir = projects_dir.join("-project");
        fs::create_dir_all(&project_dir).unwrap();

        let jsonl = r#"{"type":"user","timestamp":"2026-01-15T10:00:00Z","message":{"content":"First"}}"#;
        let jsonl_path = project_dir.join("sess-1.jsonl");
        fs::write(&jsonl_path, jsonl).unwrap();

        // First build
        build_index(&db_path, &projects_dir).unwrap();

        let index = SessionIndex::open(&db_path).unwrap();
        let results = index.search_all().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].prompts[0], "First");

        // Second build without changing the file - should skip
        build_index(&db_path, &projects_dir).unwrap();

        let index = SessionIndex::open(&db_path).unwrap();
        let results = index.search_all().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].prompts[0], "First");
    }

    #[test]
    fn uses_sessions_index_json_metadata() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let projects_dir = tmp.path().join("projects");
        let project_dir = projects_dir.join("-project");
        fs::create_dir_all(&project_dir).unwrap();

        let jsonl = r#"{"type":"user","timestamp":"2026-01-15T10:00:00Z","message":{"content":"Hello"}}"#;
        fs::write(project_dir.join("sess-meta.jsonl"), jsonl).unwrap();

        let index_json = serde_json::json!({
            "entries": [
                {
                    "sessionId": "sess-meta",
                    "projectPath": "/custom/path",
                    "gitBranch": "feature-branch",
                    "summary": "My summary",
                    "firstPrompt": "Custom first prompt",
                    "messageCount": 42,
                    "created": "2026-01-15T09:00:00Z",
                    "modified": "2026-01-15T11:00:00Z"
                }
            ]
        });
        fs::write(
            project_dir.join("sessions-index.json"),
            serde_json::to_string(&index_json).unwrap(),
        )
        .unwrap();

        build_index(&db_path, &projects_dir).unwrap();

        let index = SessionIndex::open(&db_path).unwrap();
        let results = index.search_all().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].session_id, "sess-meta");
        assert_eq!(results[0].project_path, "/custom/path");
        assert_eq!(results[0].git_branch, "feature-branch");
        assert_eq!(results[0].summary, "My summary");
        assert_eq!(results[0].prompts.len(), 1);
        assert_eq!(results[0].prompts[0], "Hello");
    }
}
