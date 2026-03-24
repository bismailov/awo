use super::*;
use anyhow::Result;
use rusqlite::Connection as SqliteConnection;
use tempfile::TempDir;

fn open_store() -> Result<(TempDir, Store)> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("state.sqlite3");
    Ok((temp_dir, Store::open(&db_path)?))
}

fn schema_version_at(path: &Path) -> Result<i64> {
    let connection = SqliteConnection::open(path)?;
    let version = connection.query_row(
        "SELECT value FROM app_meta WHERE key = 'schema_version'",
        [],
        |row| row.get::<_, String>(0),
    )?;
    Ok(version.parse()?)
}

fn journal_mode_at(path: &Path) -> Result<String> {
    let connection = SqliteConnection::open(path)?;
    Ok(connection.query_row("PRAGMA journal_mode", [], |row| row.get(0))?)
}

fn session_columns_at(path: &Path) -> Result<Vec<String>> {
    let connection = SqliteConnection::open(path)?;
    let mut statement = connection.prepare("PRAGMA table_info(sessions)")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[test]
fn initialize_schema_enables_wal_and_records_current_version() -> Result<()> {
    let (temp_dir, store) = open_store()?;
    let db_path = temp_dir.path().join("state.sqlite3");

    store.initialize_schema()?;

    assert_eq!(journal_mode_at(&db_path)?, "wal");
    assert_eq!(schema_version_at(&db_path)?, CURRENT_SCHEMA_VERSION);
    assert!(session_columns_at(&db_path)?.contains(&"supervisor".to_string()));
    Ok(())
}

#[test]
fn initialize_schema_migrates_legacy_sessions_table() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("legacy.sqlite3");
    let connection = SqliteConnection::open(&db_path)?;
    connection.execute_batch(
        r#"
            CREATE TABLE app_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE action_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                command_name TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE repositories (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                repo_root TEXT NOT NULL,
                remote_url TEXT,
                default_base_branch TEXT NOT NULL,
                worktree_root TEXT NOT NULL,
                shared_manifest_path TEXT,
                shared_manifest_present INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE slots (
                id TEXT PRIMARY KEY,
                repo_id TEXT NOT NULL,
                task_name TEXT NOT NULL,
                slot_path TEXT NOT NULL,
                branch_name TEXT NOT NULL,
                base_branch TEXT NOT NULL,
                strategy TEXT NOT NULL,
                status TEXT NOT NULL,
                fingerprint_hash TEXT,
                fingerprint_status TEXT NOT NULL,
                dirty INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                repo_id TEXT NOT NULL,
                slot_id TEXT NOT NULL,
                runtime TEXT NOT NULL,
                prompt TEXT NOT NULL,
                status TEXT NOT NULL,
                read_only INTEGER NOT NULL DEFAULT 0,
                dry_run INTEGER NOT NULL DEFAULT 0,
                command_line TEXT NOT NULL,
                stdout_path TEXT,
                stderr_path TEXT,
                exit_code INTEGER,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            INSERT INTO app_meta (key, value) VALUES ('schema_version', '3');
        "#,
    )?;
    drop(connection);

    let store = Store::open(&db_path)?;
    store.initialize_schema()?;

    assert_eq!(journal_mode_at(&db_path)?, "wal");
    assert_eq!(schema_version_at(&db_path)?, CURRENT_SCHEMA_VERSION);
    assert!(session_columns_at(&db_path)?.contains(&"supervisor".to_string()));
    Ok(())
}

#[test]
fn initialize_schema_rejects_unsupported_future_version() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("future.sqlite3");
    let connection = SqliteConnection::open(&db_path)?;
    connection.execute_batch(&format!(
        r#"
            CREATE TABLE app_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO app_meta (key, value) VALUES ('schema_version', '{}');
        "#,
        CURRENT_SCHEMA_VERSION + 1
    ))?;
    drop(connection);

    let result = Store::open(&db_path);

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("unsupported SQLite schema version")
    );
    Ok(())
}

#[test]
fn initialize_schema_rejects_malformed_version_string() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("malformed.sqlite3");
    let connection = SqliteConnection::open(&db_path)?;
    connection.execute_batch(
        r#"
            CREATE TABLE app_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO app_meta (key, value) VALUES ('schema_version', 'not-a-number');
        "#,
    )?;
    drop(connection);

    let result = Store::open(&db_path);

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("invalid SQLite schema version")
    );
    Ok(())
}

#[test]
fn roundtrip_repository_crud() -> Result<()> {
    let (_dir, store) = open_store()?;
    store.initialize_schema()?;

    assert!(store.list_repositories()?.is_empty());
    assert!(store.get_repository("test-repo")?.is_none());

    let repo = RegisteredRepo {
        id: "test-repo".to_string(),
        name: "test-repo".to_string(),
        repo_root: "/tmp/test-repo".to_string(),
        remote_url: None,
        default_base_branch: "main".to_string(),
        worktree_root: "/tmp/worktrees".to_string(),
        shared_manifest_path: None,
        shared_manifest_present: false,
        created_at: String::new(),
        updated_at: String::new(),
    };
    store.upsert_repository(&repo)?;

    let loaded = store
        .get_repository("test-repo")?
        .expect("should find repo");
    assert_eq!(loaded.id, "test-repo");
    assert_eq!(loaded.repo_root, "/tmp/test-repo");
    assert_eq!(store.list_repositories()?.len(), 1);

    // Upsert with changes
    let mut updated = repo.clone();
    updated.repo_root = "/tmp/moved-repo".to_string();
    store.upsert_repository(&updated)?;
    let loaded = store
        .get_repository("test-repo")?
        .expect("should find repo");
    assert_eq!(loaded.repo_root, "/tmp/moved-repo");
    assert_eq!(store.list_repositories()?.len(), 1);
    Ok(())
}

#[test]
fn roundtrip_slot_crud() -> Result<()> {
    let (_dir, store) = open_store()?;
    store.initialize_schema()?;

    assert!(store.list_slots(None)?.is_empty());
    assert!(store.get_slot("slot-1")?.is_none());

    let slot = SlotRecord {
        id: "slot-1".to_string(),
        repo_id: "repo-1".to_string(),
        task_name: "test-task".to_string(),
        slot_path: "/tmp/slot-1".to_string(),
        branch_name: "feat/test".to_string(),
        base_branch: "main".to_string(),
        strategy: SlotStrategy::Fresh,
        status: SlotStatus::Active,
        fingerprint_hash: Some("abc123".to_string()),
        fingerprint_status: FingerprintStatus::Ready,
        dirty: false,
        created_at: String::new(),
        updated_at: String::new(),
    };
    store.upsert_slot(&slot)?;

    let loaded = store.get_slot("slot-1")?.expect("should find slot");
    assert_eq!(loaded.task_name, "test-task");
    assert_eq!(loaded.strategy, SlotStrategy::Fresh);
    assert_eq!(loaded.status, SlotStatus::Active);
    assert_eq!(store.list_slots(None)?.len(), 1);
    assert_eq!(store.list_slots(Some("repo-1"))?.len(), 1);
    assert_eq!(store.list_slots(Some("other-repo"))?.len(), 0);
    Ok(())
}

#[test]
fn roundtrip_session_crud() -> Result<()> {
    let (_dir, store) = open_store()?;
    store.initialize_schema()?;

    assert!(store.list_sessions(None)?.is_empty());
    assert!(store.get_session("session-1")?.is_none());

    let session = SessionRecord {
        id: "session-1".to_string(),
        repo_id: "repo-1".to_string(),
        slot_id: "slot-1".to_string(),
        runtime: "shell".to_string(),
        supervisor: Some("tmux".to_string()),
        prompt: "echo hello".to_string(),
        status: SessionStatus::Prepared,
        read_only: false,
        dry_run: false,
        command_line: "bash -c 'echo hello'".to_string(),
        stdout_path: Some("/tmp/stdout.log".to_string()),
        stderr_path: Some("/tmp/stderr.log".to_string()),
        exit_code: None,
        timeout_secs: None,
        started_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    };
    store.upsert_session(&session)?;

    let loaded = store
        .get_session("session-1")?
        .expect("should find session");
    assert_eq!(loaded.runtime, "shell");
    assert_eq!(loaded.supervisor.as_deref(), Some("tmux"));
    assert_eq!(loaded.status, SessionStatus::Prepared);
    assert_eq!(store.list_sessions(None)?.len(), 1);
    assert_eq!(store.list_sessions(Some("repo-1"))?.len(), 1);
    assert_eq!(store.list_sessions(Some("other-repo"))?.len(), 0);
    assert_eq!(store.list_sessions_for_slot("slot-1")?.len(), 1);
    assert_eq!(store.list_sessions_for_slot("other-slot")?.len(), 0);

    // Delete
    store.delete_session("session-1")?;
    assert!(store.get_session("session-1")?.is_none());
    assert!(store.list_sessions(None)?.is_empty());
    Ok(())
}

#[test]
fn action_log_insert_and_query() -> Result<()> {
    let (_dir, store) = open_store()?;
    store.initialize_schema()?;

    store.insert_action("test_cmd", "payload=1")?;
    store.insert_action("test_cmd", "payload=2")?;

    let actions = store.recent_actions(10)?;
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].command_name, "test_cmd");
    Ok(())
}

#[test]
fn find_reusable_warm_slot_returns_none_when_empty() -> Result<()> {
    let (_dir, store) = open_store()?;
    store.initialize_schema()?;

    assert!(store.find_reusable_warm_slot("repo-1")?.is_none());
    Ok(())
}

#[test]
fn find_reusable_warm_slot_finds_released_warm_slot() -> Result<()> {
    let (_dir, store) = open_store()?;
    store.initialize_schema()?;

    let slot = SlotRecord {
        id: "warm-1".to_string(),
        repo_id: "repo-1".to_string(),
        task_name: "old-task".to_string(),
        slot_path: "/tmp/warm-1".to_string(),
        branch_name: "feat/old".to_string(),
        base_branch: "main".to_string(),
        strategy: SlotStrategy::Warm,
        status: SlotStatus::Released,
        fingerprint_hash: None,
        fingerprint_status: FingerprintStatus::Missing,
        dirty: false,
        created_at: String::new(),
        updated_at: String::new(),
    };
    store.upsert_slot(&slot)?;

    let found = store.find_reusable_warm_slot("repo-1")?;
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, "warm-1");

    // Different repo should not find it
    assert!(store.find_reusable_warm_slot("repo-2")?.is_none());
    Ok(())
}

#[test]
fn open_fails_on_directory_path() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    // Try to open a database where a directory already exists
    let result = Store::open(temp_dir.path());
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("failed to open SQLite database")
    );
    Ok(())
}

#[test]
fn open_fails_on_nonexistent_parent_directory() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("missing_dir").join("state.sqlite3");
    let result = Store::open(&db_path);
    assert!(result.is_err());
    Ok(())
}

#[test]
fn get_nonexistent_records_returns_none() -> Result<()> {
    let (_dir, store) = open_store()?;
    store.initialize_schema()?;

    assert!(store.get_repository("nonexistent")?.is_none());
    assert!(store.get_slot("nonexistent")?.is_none());
    assert!(store.get_session("nonexistent")?.is_none());
    Ok(())
}

#[test]
fn delete_nonexistent_session_is_noop() -> Result<()> {
    let (_dir, store) = open_store()?;
    store.initialize_schema()?;

    let result = store.delete_session("nonexistent");
    assert!(
        result.is_ok(),
        "Deleting a nonexistent session should not error"
    );
    Ok(())
}

#[test]
fn duplicate_repo_registration_updates_record() -> Result<()> {
    let (_dir, store) = open_store()?;
    store.initialize_schema()?;

    let mut repo = RegisteredRepo {
        id: "repo-1".to_string(),
        name: "test".to_string(),
        repo_root: "/test".to_string(),
        remote_url: None,
        default_base_branch: "main".to_string(),
        worktree_root: "/worktrees".to_string(),
        shared_manifest_path: None,
        shared_manifest_present: false,
        created_at: String::new(),
        updated_at: String::new(),
    };

    store.upsert_repository(&repo)?;
    repo.name = "updated".to_string();
    let result = store.upsert_repository(&repo);
    assert!(
        result.is_ok(),
        "Duplicate insert should succeed as an update"
    );

    let loaded = store.get_repository("repo-1")?.unwrap();
    assert_eq!(loaded.name, "updated");
    Ok(())
}

#[test]
fn queries_with_empty_db_return_empty_collections() -> Result<()> {
    let (_dir, store) = open_store()?;
    store.initialize_schema()?;

    assert!(store.list_repositories()?.is_empty());
    assert!(store.list_slots(None)?.is_empty());
    assert!(store.list_sessions(None)?.is_empty());
    assert!(store.list_sessions_for_slot("any")?.is_empty());
    assert!(store.recent_actions(10)?.is_empty());
    Ok(())
}
