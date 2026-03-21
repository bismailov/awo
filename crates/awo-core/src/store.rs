use crate::repo::RegisteredRepo;
use crate::runtime::SessionRecord;
use crate::slot::SlotRecord;
use crate::snapshot::CommandLogEntry;
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug)]
pub struct Store {
    connection: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)
            .with_context(|| format!("failed to open SQLite database at {}", path.display()))?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn initialize_schema(&self) -> Result<()> {
        let sql = r#"
            CREATE TABLE IF NOT EXISTS app_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS action_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                command_name TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS repositories (
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

            CREATE TABLE IF NOT EXISTS slots (
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

            CREATE TABLE IF NOT EXISTS sessions (
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

            INSERT INTO app_meta (key, value)
            VALUES ('schema_version', '3')
            ON CONFLICT(key) DO UPDATE SET value = excluded.value;
        "#;

        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock store connection"))?;
        connection
            .execute_batch(sql)
            .context("failed to initialize SQLite schema")?;
        Ok(())
    }

    pub fn insert_action(&self, command_name: &str, payload: &str) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock store connection"))?;
        connection
            .execute(
                "INSERT INTO action_log (command_name, payload) VALUES (?1, ?2)",
                params![command_name, payload],
            )
            .with_context(|| format!("failed to insert action log for command `{command_name}`"))?;
        Ok(())
    }

    pub fn recent_actions(&self, limit: usize) -> Result<Vec<CommandLogEntry>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock store connection"))?;
        let mut statement = connection
            .prepare(
                "SELECT id, command_name, payload, created_at
                 FROM action_log
                 ORDER BY id DESC
                 LIMIT ?1",
            )
            .context("failed to prepare recent action query")?;

        let rows = statement
            .query_map([limit as i64], |row| {
                Ok(CommandLogEntry {
                    id: row.get(0)?,
                    command_name: row.get(1)?,
                    payload: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .context("failed to query recent actions")?;

        let entries = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to collect recent action rows")?;
        Ok(entries)
    }

    pub fn upsert_repository(&self, repo: &RegisteredRepo) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock store connection"))?;
        connection
            .execute(
                "INSERT INTO repositories (
                    id, name, repo_root, remote_url, default_base_branch, worktree_root,
                    shared_manifest_path, shared_manifest_present
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    repo_root = excluded.repo_root,
                    remote_url = excluded.remote_url,
                    default_base_branch = excluded.default_base_branch,
                    worktree_root = excluded.worktree_root,
                    shared_manifest_path = excluded.shared_manifest_path,
                    shared_manifest_present = excluded.shared_manifest_present,
                    updated_at = CURRENT_TIMESTAMP",
                params![
                    repo.id,
                    repo.name,
                    repo.repo_root,
                    repo.remote_url,
                    repo.default_base_branch,
                    repo.worktree_root,
                    repo.shared_manifest_path,
                    repo.shared_manifest_present as i64
                ],
            )
            .with_context(|| format!("failed to upsert repository `{}`", repo.id))?;
        Ok(())
    }

    pub fn list_repositories(&self) -> Result<Vec<RegisteredRepo>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock store connection"))?;
        let mut statement = connection
            .prepare(
                "SELECT
                    id, name, repo_root, remote_url, default_base_branch, worktree_root,
                    shared_manifest_path, shared_manifest_present, created_at, updated_at
                 FROM repositories
                 ORDER BY name ASC",
            )
            .context("failed to prepare repository list query")?;

        let rows = statement
            .query_map([], |row| {
                Ok(RegisteredRepo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    repo_root: row.get(2)?,
                    remote_url: row.get(3)?,
                    default_base_branch: row.get(4)?,
                    worktree_root: row.get(5)?,
                    shared_manifest_path: row.get(6)?,
                    shared_manifest_present: row.get::<_, i64>(7)? != 0,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            })
            .context("failed to query repositories")?;

        let repos = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to collect repository rows")?;
        Ok(repos)
    }

    pub fn get_repository(&self, repo_id: &str) -> Result<Option<RegisteredRepo>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock store connection"))?;
        let mut statement = connection
            .prepare(
                "SELECT
                    id, name, repo_root, remote_url, default_base_branch, worktree_root,
                    shared_manifest_path, shared_manifest_present, created_at, updated_at
                 FROM repositories
                 WHERE id = ?1",
            )
            .context("failed to prepare repository lookup query")?;

        let repo = statement
            .query_row([repo_id], |row| {
                Ok(RegisteredRepo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    repo_root: row.get(2)?,
                    remote_url: row.get(3)?,
                    default_base_branch: row.get(4)?,
                    worktree_root: row.get(5)?,
                    shared_manifest_path: row.get(6)?,
                    shared_manifest_present: row.get::<_, i64>(7)? != 0,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            })
            .optional()
            .context("failed to lookup repository")?;
        Ok(repo)
    }

    pub fn upsert_slot(&self, slot: &SlotRecord) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock store connection"))?;
        connection
            .execute(
                "INSERT INTO slots (
                    id, repo_id, task_name, slot_path, branch_name, base_branch, strategy,
                    status, fingerprint_hash, fingerprint_status, dirty
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(id) DO UPDATE SET
                    repo_id = excluded.repo_id,
                    task_name = excluded.task_name,
                    slot_path = excluded.slot_path,
                    branch_name = excluded.branch_name,
                    base_branch = excluded.base_branch,
                    strategy = excluded.strategy,
                    status = excluded.status,
                    fingerprint_hash = excluded.fingerprint_hash,
                    fingerprint_status = excluded.fingerprint_status,
                    dirty = excluded.dirty,
                    updated_at = CURRENT_TIMESTAMP",
                params![
                    slot.id,
                    slot.repo_id,
                    slot.task_name,
                    slot.slot_path,
                    slot.branch_name,
                    slot.base_branch,
                    slot.strategy,
                    slot.status,
                    slot.fingerprint_hash,
                    slot.fingerprint_status,
                    slot.dirty as i64
                ],
            )
            .with_context(|| format!("failed to upsert slot `{}`", slot.id))?;
        Ok(())
    }

    pub fn get_slot(&self, slot_id: &str) -> Result<Option<SlotRecord>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock store connection"))?;
        let mut statement = connection
            .prepare(
                "SELECT
                    id, repo_id, task_name, slot_path, branch_name, base_branch,
                    strategy, status, fingerprint_hash, fingerprint_status, dirty,
                    created_at, updated_at
                 FROM slots
                 WHERE id = ?1",
            )
            .context("failed to prepare slot lookup query")?;

        let slot = statement
            .query_row([slot_id], |row| {
                Ok(SlotRecord {
                    id: row.get(0)?,
                    repo_id: row.get(1)?,
                    task_name: row.get(2)?,
                    slot_path: row.get(3)?,
                    branch_name: row.get(4)?,
                    base_branch: row.get(5)?,
                    strategy: row.get(6)?,
                    status: row.get(7)?,
                    fingerprint_hash: row.get(8)?,
                    fingerprint_status: row.get(9)?,
                    dirty: row.get::<_, i64>(10)? != 0,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            })
            .optional()
            .context("failed to lookup slot")?;
        Ok(slot)
    }

    pub fn list_slots(&self, repo_id: Option<&str>) -> Result<Vec<SlotRecord>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock store connection"))?;
        let query = if repo_id.is_some() {
            "SELECT
                id, repo_id, task_name, slot_path, branch_name, base_branch,
                strategy, status, fingerprint_hash, fingerprint_status, dirty,
                created_at, updated_at
             FROM slots
             WHERE repo_id = ?1
             ORDER BY created_at DESC"
        } else {
            "SELECT
                id, repo_id, task_name, slot_path, branch_name, base_branch,
                strategy, status, fingerprint_hash, fingerprint_status, dirty,
                created_at, updated_at
             FROM slots
             ORDER BY created_at DESC"
        };
        let mut statement = connection
            .prepare(query)
            .context("failed to prepare slot list query")?;

        let map_row = |row: &rusqlite::Row<'_>| {
            Ok(SlotRecord {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                task_name: row.get(2)?,
                slot_path: row.get(3)?,
                branch_name: row.get(4)?,
                base_branch: row.get(5)?,
                strategy: row.get(6)?,
                status: row.get(7)?,
                fingerprint_hash: row.get(8)?,
                fingerprint_status: row.get(9)?,
                dirty: row.get::<_, i64>(10)? != 0,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        };

        let rows = if let Some(repo_id) = repo_id {
            statement.query_map([repo_id], map_row)?
        } else {
            statement.query_map([], map_row)?
        };

        let slots = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to collect slot rows")?;
        Ok(slots)
    }

    pub fn find_reusable_warm_slot(&self, repo_id: &str) -> Result<Option<SlotRecord>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock store connection"))?;
        let mut statement = connection
            .prepare(
                "SELECT
                    id, repo_id, task_name, slot_path, branch_name, base_branch,
                    strategy, status, fingerprint_hash, fingerprint_status, dirty,
                    created_at, updated_at
                 FROM slots
                 WHERE repo_id = ?1
                   AND strategy = 'warm'
                   AND status = 'released'
                   AND dirty = 0
                 ORDER BY updated_at ASC
                 LIMIT 1",
            )
            .context("failed to prepare reusable warm slot query")?;

        let slot = statement
            .query_row([repo_id], |row| {
                Ok(SlotRecord {
                    id: row.get(0)?,
                    repo_id: row.get(1)?,
                    task_name: row.get(2)?,
                    slot_path: row.get(3)?,
                    branch_name: row.get(4)?,
                    base_branch: row.get(5)?,
                    strategy: row.get(6)?,
                    status: row.get(7)?,
                    fingerprint_hash: row.get(8)?,
                    fingerprint_status: row.get(9)?,
                    dirty: row.get::<_, i64>(10)? != 0,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            })
            .optional()
            .context("failed to find reusable warm slot")?;
        Ok(slot)
    }

    pub fn upsert_session(&self, session: &SessionRecord) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock store connection"))?;
        connection
            .execute(
                "INSERT INTO sessions (
                    id, repo_id, slot_id, runtime, prompt, status, read_only, dry_run,
                    command_line, stdout_path, stderr_path, exit_code
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                 ON CONFLICT(id) DO UPDATE SET
                    repo_id = excluded.repo_id,
                    slot_id = excluded.slot_id,
                    runtime = excluded.runtime,
                    prompt = excluded.prompt,
                    status = excluded.status,
                    read_only = excluded.read_only,
                    dry_run = excluded.dry_run,
                    command_line = excluded.command_line,
                    stdout_path = excluded.stdout_path,
                    stderr_path = excluded.stderr_path,
                    exit_code = excluded.exit_code,
                    updated_at = CURRENT_TIMESTAMP",
                params![
                    session.id,
                    session.repo_id,
                    session.slot_id,
                    session.runtime,
                    session.prompt,
                    session.status,
                    session.read_only as i64,
                    session.dry_run as i64,
                    session.command_line,
                    session.stdout_path,
                    session.stderr_path,
                    session.exit_code
                ],
            )
            .with_context(|| format!("failed to upsert session `{}`", session.id))?;
        Ok(())
    }

    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock store connection"))?;
        connection
            .execute("DELETE FROM sessions WHERE id = ?1", [session_id])
            .with_context(|| format!("failed to delete session `{session_id}`"))?;
        Ok(())
    }

    pub fn list_sessions(&self, repo_id: Option<&str>) -> Result<Vec<SessionRecord>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock store connection"))?;
        let query = if repo_id.is_some() {
            "SELECT
                id, repo_id, slot_id, runtime, prompt, status, read_only, dry_run,
                command_line, stdout_path, stderr_path, exit_code, created_at, updated_at
             FROM sessions
             WHERE repo_id = ?1
             ORDER BY created_at DESC"
        } else {
            "SELECT
                id, repo_id, slot_id, runtime, prompt, status, read_only, dry_run,
                command_line, stdout_path, stderr_path, exit_code, created_at, updated_at
             FROM sessions
             ORDER BY created_at DESC"
        };
        let mut statement = connection
            .prepare(query)
            .context("failed to prepare session list query")?;

        let map_row = |row: &rusqlite::Row<'_>| {
            Ok(SessionRecord {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                slot_id: row.get(2)?,
                runtime: row.get(3)?,
                prompt: row.get(4)?,
                status: row.get(5)?,
                read_only: row.get::<_, i64>(6)? != 0,
                dry_run: row.get::<_, i64>(7)? != 0,
                command_line: row.get(8)?,
                stdout_path: row.get(9)?,
                stderr_path: row.get(10)?,
                exit_code: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        };

        let rows = if let Some(repo_id) = repo_id {
            statement.query_map([repo_id], map_row)?
        } else {
            statement.query_map([], map_row)?
        };

        let sessions = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to collect session rows")?;
        Ok(sessions)
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionRecord>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock store connection"))?;
        let mut statement = connection
            .prepare(
                "SELECT
                    id, repo_id, slot_id, runtime, prompt, status, read_only, dry_run,
                    command_line, stdout_path, stderr_path, exit_code, created_at, updated_at
                 FROM sessions
                 WHERE id = ?1",
            )
            .context("failed to prepare session lookup query")?;

        let session = statement
            .query_row([session_id], |row| {
                Ok(SessionRecord {
                    id: row.get(0)?,
                    repo_id: row.get(1)?,
                    slot_id: row.get(2)?,
                    runtime: row.get(3)?,
                    prompt: row.get(4)?,
                    status: row.get(5)?,
                    read_only: row.get::<_, i64>(6)? != 0,
                    dry_run: row.get::<_, i64>(7)? != 0,
                    command_line: row.get(8)?,
                    stdout_path: row.get(9)?,
                    stderr_path: row.get(10)?,
                    exit_code: row.get(11)?,
                    created_at: row.get(12)?,
                    updated_at: row.get(13)?,
                })
            })
            .optional()
            .context("failed to lookup session")?;
        Ok(session)
    }

    pub fn list_sessions_for_slot(&self, slot_id: &str) -> Result<Vec<SessionRecord>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock store connection"))?;
        let mut statement = connection
            .prepare(
                "SELECT
                    id, repo_id, slot_id, runtime, prompt, status, read_only, dry_run,
                    command_line, stdout_path, stderr_path, exit_code, created_at, updated_at
                 FROM sessions
                 WHERE slot_id = ?1
                 ORDER BY created_at DESC",
            )
            .context("failed to prepare slot session list query")?;

        let rows = statement.query_map([slot_id], |row| {
            Ok(SessionRecord {
                id: row.get(0)?,
                repo_id: row.get(1)?,
                slot_id: row.get(2)?,
                runtime: row.get(3)?,
                prompt: row.get(4)?,
                status: row.get(5)?,
                read_only: row.get::<_, i64>(6)? != 0,
                dry_run: row.get::<_, i64>(7)? != 0,
                command_line: row.get(8)?,
                stdout_path: row.get(9)?,
                stderr_path: row.get(10)?,
                exit_code: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })?;

        let sessions = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to collect slot session rows")?;
        Ok(sessions)
    }
}
