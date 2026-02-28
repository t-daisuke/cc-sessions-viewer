use std::fs;
use tempfile::TempDir;

#[test]
fn full_index_and_search_flow() {
    let tmp = TempDir::new().unwrap();
    let projects_dir = tmp.path().join("projects");
    let db_path = tmp.path().join("index.db");

    // Create two projects with sessions
    let project1 = projects_dir.join("project-auth");
    fs::create_dir_all(&project1).unwrap();
    fs::write(
        project1.join("sess-1.jsonl"),
        r#"{"type":"user","timestamp":"2026-01-15T10:00:00Z","message":{"content":"JWT認証を実装して"},"gitBranch":"feat/auth"}
{"type":"assistant","timestamp":"2026-01-15T10:01:00Z","message":{"content":"承知しました"}}
{"type":"user","timestamp":"2026-01-15T10:02:00Z","message":{"content":"テストも書いて"}}"#,
    )
    .unwrap();

    let project2 = projects_dir.join("project-deploy");
    fs::create_dir_all(&project2).unwrap();
    fs::write(
        project2.join("sess-2.jsonl"),
        r#"{"type":"user","timestamp":"2026-01-14T10:00:00Z","message":{"content":"Kamalでデプロイ設定して"},"gitBranch":"main"}
{"type":"assistant","timestamp":"2026-01-14T10:01:00Z","message":{"content":"OK"}}"#,
    )
    .unwrap();

    // Build index
    cc_sessions_viewer::indexer::build_index(&db_path, &projects_dir).unwrap();

    // Verify index contents
    let index = cc_sessions_viewer::index::SessionIndex::open(&db_path).unwrap();
    let results = index.search_all().unwrap();
    assert_eq!(results.len(), 2);

    // Verify user prompts were extracted
    let auth_session = results.iter().find(|r| r.session_id == "sess-1").unwrap();
    assert_eq!(auth_session.prompts.len(), 2);
    assert!(auth_session.prompts[0].contains("JWT認証"));
    assert!(auth_session.prompts[1].contains("テスト"));

    let deploy_session = results.iter().find(|r| r.session_id == "sess-2").unwrap();
    assert_eq!(deploy_session.prompts.len(), 1);
    assert!(deploy_session.prompts[0].contains("Kamal"));
}

#[test]
fn incremental_index_update() {
    let tmp = TempDir::new().unwrap();
    let projects_dir = tmp.path().join("projects");
    let db_path = tmp.path().join("index.db");

    let project_dir = projects_dir.join("my-project");
    fs::create_dir_all(&project_dir).unwrap();

    // First session
    fs::write(
        project_dir.join("sess-a.jsonl"),
        r#"{"type":"user","timestamp":"2026-01-15T10:00:00Z","message":{"content":"Hello"}}"#,
    )
    .unwrap();

    cc_sessions_viewer::indexer::build_index(&db_path, &projects_dir).unwrap();

    let index = cc_sessions_viewer::index::SessionIndex::open(&db_path).unwrap();
    assert_eq!(index.search_all().unwrap().len(), 1);

    // Add second session
    fs::write(
        project_dir.join("sess-b.jsonl"),
        r#"{"type":"user","timestamp":"2026-01-16T10:00:00Z","message":{"content":"World"}}"#,
    )
    .unwrap();

    cc_sessions_viewer::indexer::build_index(&db_path, &projects_dir).unwrap();

    let index = cc_sessions_viewer::index::SessionIndex::open(&db_path).unwrap();
    assert_eq!(index.search_all().unwrap().len(), 2);
}
