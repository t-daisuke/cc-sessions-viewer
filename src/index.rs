use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

pub struct SessionRecord {
    pub session_id: String,
    pub project_path: String,
    pub dir_name: String,
    pub git_branch: String,
    pub summary: String,
    pub first_prompt: String,
    pub message_count: i64,
    pub created_at: String,
    pub modified_at: String,
    pub file_mtime: i64,
}

pub struct PromptRecord {
    pub prompt: String,
    pub timestamp: Option<String>,
}

pub struct SearchableSession {
    pub session_id: String,
    pub project_path: String,
    pub dir_name: String,
    pub git_branch: String,
    pub summary: String,
    pub created_at: String,
    pub prompts: Vec<String>,
}

pub struct SessionIndex {
    conn: Connection,
}

impl SessionIndex {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sessions (
                session_id    TEXT PRIMARY KEY,
                project_path  TEXT NOT NULL,
                dir_name      TEXT NOT NULL,
                git_branch    TEXT DEFAULT '',
                summary       TEXT DEFAULT '',
                first_prompt  TEXT DEFAULT '',
                message_count INTEGER DEFAULT 0,
                created_at    TEXT DEFAULT '',
                modified_at   TEXT DEFAULT '',
                file_mtime    INTEGER DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS user_prompts (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL REFERENCES sessions(session_id),
                prompt     TEXT NOT NULL,
                timestamp  TEXT,
                UNIQUE(session_id, prompt, timestamp)
            );
        ",
        )?;
        Ok(SessionIndex { conn })
    }

    pub fn upsert_session(&self, rec: &SessionRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (session_id, project_path, dir_name, git_branch, summary, first_prompt, message_count, created_at, modified_at, file_mtime)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(session_id) DO UPDATE SET
                project_path = excluded.project_path,
                dir_name = excluded.dir_name,
                git_branch = excluded.git_branch,
                summary = excluded.summary,
                first_prompt = excluded.first_prompt,
                message_count = excluded.message_count,
                created_at = excluded.created_at,
                modified_at = excluded.modified_at,
                file_mtime = excluded.file_mtime",
            rusqlite::params![
                rec.session_id,
                rec.project_path,
                rec.dir_name,
                rec.git_branch,
                rec.summary,
                rec.first_prompt,
                rec.message_count,
                rec.created_at,
                rec.modified_at,
                rec.file_mtime,
            ],
        )?;
        Ok(())
    }

    pub fn insert_prompts(&self, session_id: &str, prompts: &[PromptRecord]) -> Result<()> {
        self.conn
            .execute("DELETE FROM user_prompts WHERE session_id = ?1", [session_id])?;
        let mut stmt = self.conn.prepare(
            "INSERT OR IGNORE INTO user_prompts (session_id, prompt, timestamp) VALUES (?1, ?2, ?3)",
        )?;
        for p in prompts {
            stmt.execute(rusqlite::params![session_id, p.prompt, p.timestamp])?;
        }
        Ok(())
    }

    pub fn get_file_mtime(&self, session_id: &str) -> Result<Option<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT file_mtime FROM sessions WHERE session_id = ?1")?;
        let mut rows = stmt.query([session_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn search_all(&self) -> Result<Vec<SearchableSession>> {
        let mut sessions_stmt = self.conn.prepare(
            "SELECT session_id, project_path, dir_name, git_branch, summary, created_at FROM sessions ORDER BY created_at DESC",
        )?;
        let mut prompts_stmt = self
            .conn
            .prepare("SELECT prompt FROM user_prompts WHERE session_id = ?1 ORDER BY id")?;

        let mut results = Vec::new();
        let session_rows = sessions_stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;

        for session_row in session_rows {
            let (session_id, project_path, dir_name, git_branch, summary, created_at) =
                session_row?;
            let prompts: Vec<String> = prompts_stmt
                .query_map([&session_id], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();

            results.push(SearchableSession {
                session_id,
                project_path,
                dir_name,
                git_branch,
                summary,
                created_at,
                prompts,
            });
        }

        Ok(results)
    }

    pub fn all_session_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT session_id FROM sessions")?;
        let ids = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn create_db_creates_tables() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let index = SessionIndex::open(&db_path).unwrap();

        let count: i64 = index
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='sessions'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        let count: i64 = index
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='user_prompts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn upsert_session_and_prompts() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let index = SessionIndex::open(&db_path).unwrap();

        let rec = SessionRecord {
            session_id: "sess-1".to_string(),
            project_path: "/home/user/project".to_string(),
            dir_name: "-home-user-project".to_string(),
            git_branch: "main".to_string(),
            summary: "Test session".to_string(),
            first_prompt: "Hello world".to_string(),
            message_count: 5,
            created_at: "2026-01-15T10:00:00Z".to_string(),
            modified_at: "2026-01-15T11:00:00Z".to_string(),
            file_mtime: 1700000000,
        };
        index.upsert_session(&rec).unwrap();

        let prompts = vec![
            PromptRecord {
                prompt: "Hello world".to_string(),
                timestamp: Some("2026-01-15T10:00:00Z".to_string()),
            },
            PromptRecord {
                prompt: "How are you?".to_string(),
                timestamp: Some("2026-01-15T10:05:00Z".to_string()),
            },
        ];
        index.insert_prompts("sess-1", &prompts).unwrap();

        let results = index.search_all().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].session_id, "sess-1");
        assert_eq!(results[0].project_path, "/home/user/project");
        assert_eq!(results[0].git_branch, "main");
        assert_eq!(results[0].summary, "Test session");
        assert_eq!(results[0].prompts.len(), 2);
        assert_eq!(results[0].prompts[0], "Hello world");
        assert_eq!(results[0].prompts[1], "How are you?");
    }

    #[test]
    fn get_file_mtime_returns_none_for_unknown() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let index = SessionIndex::open(&db_path).unwrap();

        let result = index.get_file_mtime("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn get_file_mtime_returns_stored_value() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let index = SessionIndex::open(&db_path).unwrap();

        let rec = SessionRecord {
            session_id: "sess-1".to_string(),
            project_path: "/project".to_string(),
            dir_name: "-project".to_string(),
            git_branch: "".to_string(),
            summary: "".to_string(),
            first_prompt: "".to_string(),
            message_count: 0,
            created_at: "".to_string(),
            modified_at: "".to_string(),
            file_mtime: 1700000000,
        };
        index.upsert_session(&rec).unwrap();

        let mtime = index.get_file_mtime("sess-1").unwrap();
        assert_eq!(mtime, Some(1700000000));
    }

    #[test]
    fn upsert_session_updates_existing() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let index = SessionIndex::open(&db_path).unwrap();

        let rec1 = SessionRecord {
            session_id: "sess-1".to_string(),
            project_path: "/project".to_string(),
            dir_name: "-project".to_string(),
            git_branch: "main".to_string(),
            summary: "Original".to_string(),
            first_prompt: "Hello".to_string(),
            message_count: 3,
            created_at: "2026-01-15T10:00:00Z".to_string(),
            modified_at: "2026-01-15T10:00:00Z".to_string(),
            file_mtime: 1700000000,
        };
        index.upsert_session(&rec1).unwrap();

        let rec2 = SessionRecord {
            session_id: "sess-1".to_string(),
            project_path: "/project".to_string(),
            dir_name: "-project".to_string(),
            git_branch: "feature".to_string(),
            summary: "Updated".to_string(),
            first_prompt: "Hello".to_string(),
            message_count: 10,
            created_at: "2026-01-15T10:00:00Z".to_string(),
            modified_at: "2026-01-15T12:00:00Z".to_string(),
            file_mtime: 1700001000,
        };
        index.upsert_session(&rec2).unwrap();

        let results = index.search_all().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].summary, "Updated");
        assert_eq!(results[0].git_branch, "feature");

        let mtime = index.get_file_mtime("sess-1").unwrap();
        assert_eq!(mtime, Some(1700001000));
    }
}
