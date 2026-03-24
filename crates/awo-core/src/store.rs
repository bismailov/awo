use crate::error::{AwoError, AwoResult};
use crate::repo::RegisteredRepo;
use crate::runtime::{SessionRecord, SessionStatus};
use crate::slot::{FingerprintStatus, SlotRecord, SlotStatus, SlotStrategy};
use crate::snapshot::CommandLogEntry;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;

const CURRENT_SCHEMA_VERSION: i64 = 4;
const BASE_SCHEMA_VERSION: i64 = 3;
const SCHEMA_VERSION_KEY: &str = "schema_version";

const BASE_SCHEMA_SQL: &str = r#"
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
"#;

const MIGRATION_V4_ADD_SESSION_SUPERVISOR_SQL: &str =
    "ALTER TABLE sessions ADD COLUMN supervisor TEXT";

#[derive(Debug)]
pub struct Store {
    connection: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &Path) -> AwoResult<Self> {
        let connection = Connection::open(path).map_err(|e| {
            AwoError::store(
                format!("failed to open SQLite database at {}", path.display()),
                e,
            )
        })?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn initialize_schema(&self) -> AwoResult<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AwoError::invalid_state("failed to lock store connection"))?;
        connection
            .execute_batch(BASE_SCHEMA_SQL)
            .map_err(|e| AwoError::store("failed to initialize SQLite schema", e))?;
        enable_wal_mode(&connection)?;
        let schema_version = schema_version(&connection)?.unwrap_or(BASE_SCHEMA_VERSION);
        apply_schema_migrations(&connection, schema_version)?;
        connection
            .execute(
                "INSERT INTO app_meta (key, value)
                 VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![SCHEMA_VERSION_KEY, CURRENT_SCHEMA_VERSION.to_string()],
            )
            .map_err(|e| AwoError::store("failed to update SQLite schema version", e))?;
        Ok(())
    }

    pub fn insert_action(&self, command_name: &str, payload: &str) -> AwoResult<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AwoError::invalid_state("failed to lock store connection"))?;
        connection
            .execute(
                "INSERT INTO action_log (command_name, payload) VALUES (?1, ?2)",
                params![command_name, payload],
            )
            .map_err(|e| {
                AwoError::store(
                    format!("failed to insert action log for command `{command_name}`"),
                    e,
                )
            })?;
        Ok(())
    }

    pub fn recent_actions(&self, limit: usize) -> AwoResult<Vec<CommandLogEntry>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AwoError::invalid_state("failed to lock store connection"))?;
        let mut statement = connection
            .prepare(
                "SELECT id, command_name, payload, created_at
                 FROM action_log
                 ORDER BY id DESC
                 LIMIT ?1",
            )
            .map_err(|e| AwoError::store("failed to prepare recent action query", e))?;

        let rows = statement
            .query_map([limit as i64], |row| {
                Ok(CommandLogEntry {
                    id: row.get(0)?,
                    command_name: row.get(1)?,
                    payload: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|e| AwoError::store("failed to query recent actions", e))?;

        let entries = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| AwoError::store("failed to collect recent action rows", e))?;
        Ok(entries)
    }

    pub fn upsert_repository(&self, repo: &RegisteredRepo) -> AwoResult<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AwoError::invalid_state("failed to lock store connection"))?;
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
            .map_err(|e| {
                AwoError::store(format!("failed to upsert repository `{}`", repo.id), e)
            })?;
        Ok(())
    }

    pub fn list_repositories(&self) -> AwoResult<Vec<RegisteredRepo>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AwoError::invalid_state("failed to lock store connection"))?;
        let mut statement = connection
            .prepare(
                "SELECT
                    id, name, repo_root, remote_url, default_base_branch, worktree_root,
                    shared_manifest_path, shared_manifest_present, created_at, updated_at
                 FROM repositories
                 ORDER BY name ASC",
            )
            .map_err(|e| AwoError::store("failed to prepare repository list query", e))?;

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
            .map_err(|e| AwoError::store("failed to query repositories", e))?;

        let repos = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| AwoError::store("failed to collect repository rows", e))?;
        Ok(repos)
    }

    pub fn get_repository(&self, repo_id: &str) -> AwoResult<Option<RegisteredRepo>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AwoError::invalid_state("failed to lock store connection"))?;
        let mut statement = connection
            .prepare(
                "SELECT
                    id, name, repo_root, remote_url, default_base_branch, worktree_root,
                    shared_manifest_path, shared_manifest_present, created_at, updated_at
                 FROM repositories
                 WHERE id = ?1",
            )
            .map_err(|e| AwoError::store("failed to prepare repository lookup query", e))?;

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
            .map_err(|e| AwoError::store("failed to lookup repository", e))?;
        Ok(repo)
    }

    pub fn upsert_slot(&self, slot: &SlotRecord) -> AwoResult<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AwoError::invalid_state("failed to lock store connection"))?;
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
                    slot.strategy.as_str(),
                    slot.status.as_str(),
                    slot.fingerprint_hash,
                    slot.fingerprint_status.as_str(),
                    slot.dirty
                ],
            )
            .map_err(|e| AwoError::store("failed to upsert slot", e))?;
        Ok(())
    }

    pub fn get_slot(&self, slot_id: &str) -> AwoResult<Option<SlotRecord>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AwoError::invalid_state("failed to lock store connection"))?;
        let mut statement = connection
            .prepare(
                "SELECT
                    id, repo_id, task_name, slot_path, branch_name, base_branch,
                    strategy, status, fingerprint_hash, fingerprint_status, dirty,
                    created_at, updated_at
                 FROM slots
                 WHERE id = ?1",
            )
            .map_err(|e| AwoError::store("failed to prepare slot lookup query", e))?;

        let slot = statement
            .query_row([slot_id], row_to_slot)
            .optional()
            .map_err(|e| AwoError::store("failed to lookup slot", e))?;
        Ok(slot)
    }

    pub fn list_slots(&self, repo_id: Option<&str>) -> AwoResult<Vec<SlotRecord>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AwoError::invalid_state("failed to lock store connection"))?;
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
            .map_err(|e| AwoError::store("failed to prepare slot list query", e))?;

        let rows = if let Some(repo_id) = repo_id {
            statement
                .query_map([repo_id], row_to_slot)
                .map_err(|e| AwoError::store("failed to query slots", e))?
        } else {
            statement
                .query_map([], row_to_slot)
                .map_err(|e| AwoError::store("failed to query slots", e))?
        };

        let slots = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| AwoError::store("failed to collect slot rows", e))?;
        Ok(slots)
    }

    pub fn find_reusable_warm_slot(&self, repo_id: &str) -> AwoResult<Option<SlotRecord>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AwoError::invalid_state("failed to lock store connection"))?;
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
            .map_err(|e| AwoError::store("failed to prepare reusable warm slot query", e))?;

        let slot = statement
            .query_row([repo_id], row_to_slot)
            .optional()
            .map_err(|e| AwoError::store("failed to find reusable warm slot", e))?;
        Ok(slot)
    }

    pub fn upsert_session(&self, session: &SessionRecord) -> AwoResult<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AwoError::invalid_state("failed to lock store connection"))?;
        connection
            .execute(
                "INSERT INTO sessions (
                    id, repo_id, slot_id, runtime, supervisor, prompt, status, read_only, dry_run,
                    command_line, stdout_path, stderr_path, exit_code
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT(id) DO UPDATE SET
                    repo_id = excluded.repo_id,
                    slot_id = excluded.slot_id,
                    runtime = excluded.runtime,
                    supervisor = excluded.supervisor,
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
                    session.supervisor,
                    session.prompt,
                    session.status.as_str(),
                    session.read_only as i64,
                    session.dry_run as i64,
                    session.command_line,
                    session.stdout_path,
                    session.stderr_path,
                    session.exit_code
                ],
            )
            .map_err(|e| {
                AwoError::store(format!("failed to upsert session `{}`", session.id), e)
            })?;
        Ok(())
    }

    pub fn delete_session(&self, session_id: &str) -> AwoResult<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AwoError::invalid_state("failed to lock store connection"))?;
        connection
            .execute("DELETE FROM sessions WHERE id = ?1", [session_id])
            .map_err(|e| AwoError::store(format!("failed to delete session `{session_id}`"), e))?;
        Ok(())
    }

    pub fn list_sessions(&self, repo_id: Option<&str>) -> AwoResult<Vec<SessionRecord>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AwoError::invalid_state("failed to lock store connection"))?;
        let query = if repo_id.is_some() {
            "SELECT
                id, repo_id, slot_id, runtime, supervisor, prompt, status, read_only, dry_run,
                command_line, stdout_path, stderr_path, exit_code, created_at, updated_at
             FROM sessions
             WHERE repo_id = ?1
             ORDER BY created_at DESC"
        } else {
            "SELECT
                id, repo_id, slot_id, runtime, supervisor, prompt, status, read_only, dry_run,
                command_line, stdout_path, stderr_path, exit_code, created_at, updated_at
             FROM sessions
             ORDER BY created_at DESC"
        };
        let mut statement = connection
            .prepare(query)
            .map_err(|e| AwoError::store("failed to prepare session list query", e))?;

        let rows = if let Some(repo_id) = repo_id {
            statement
                .query_map([repo_id], row_to_session)
                .map_err(|e| AwoError::store("failed to query sessions", e))?
        } else {
            statement
                .query_map([], row_to_session)
                .map_err(|e| AwoError::store("failed to query sessions", e))?
        };

        let sessions = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| AwoError::store("failed to collect session rows", e))?;
        Ok(sessions)
    }

    pub fn get_session(&self, session_id: &str) -> AwoResult<Option<SessionRecord>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AwoError::invalid_state("failed to lock store connection"))?;
        let mut statement = connection
            .prepare(
                "SELECT
                    id, repo_id, slot_id, runtime, supervisor, prompt, status, read_only, dry_run,
                    command_line, stdout_path, stderr_path, exit_code, created_at, updated_at
                 FROM sessions
                 WHERE id = ?1",
            )
            .map_err(|e| AwoError::store("failed to prepare session lookup query", e))?;

        let session = statement
            .query_row([session_id], row_to_session)
            .optional()
            .map_err(|e| AwoError::store("failed to lookup session", e))?;
        Ok(session)
    }

    pub fn list_sessions_for_slot(&self, slot_id: &str) -> AwoResult<Vec<SessionRecord>> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AwoError::invalid_state("failed to lock store connection"))?;
        let mut statement = connection
            .prepare(
                "SELECT
                    id, repo_id, slot_id, runtime, supervisor, prompt, status, read_only, dry_run,
                    command_line, stdout_path, stderr_path, exit_code, created_at, updated_at
                 FROM sessions
                 WHERE slot_id = ?1
                 ORDER BY created_at DESC",
            )
            .map_err(|e| AwoError::store("failed to prepare slot session list query", e))?;

        let rows = statement
            .query_map([slot_id], row_to_session)
            .map_err(|e| AwoError::store("failed to query slot sessions", e))?;

        let sessions = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| AwoError::store("failed to collect slot session rows", e))?;
        Ok(sessions)
    }
}

fn row_to_slot(row: &rusqlite::Row) -> rusqlite::Result<SlotRecord> {
    let strategy_str: String = row.get(6)?;
    let status_str: String = row.get(7)?;
    let fingerprint_status_str: String = row.get(9)?;

    Ok(SlotRecord {
        id: row.get(0)?,
        repo_id: row.get(1)?,
        task_name: row.get(2)?,
        slot_path: row.get(3)?,
        branch_name: row.get(4)?,
        base_branch: row.get(5)?,
        strategy: SlotStrategy::from_str(&strategy_str).map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                6,
                rusqlite::types::Type::Text,
                Box::new(AwoError::unsupported("slot strategy", strategy_str)),
            )
        })?,
        status: SlotStatus::from_str(&status_str).map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                7,
                rusqlite::types::Type::Text,
                Box::new(AwoError::unsupported("slot status", status_str)),
            )
        })?,
        fingerprint_hash: row.get(8)?,
        fingerprint_status: FingerprintStatus::from_str(&fingerprint_status_str).map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                9,
                rusqlite::types::Type::Text,
                Box::new(AwoError::unsupported(
                    "fingerprint status",
                    fingerprint_status_str,
                )),
            )
        })?,
        dirty: row.get::<_, i64>(10)? != 0,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn row_to_session(row: &rusqlite::Row) -> rusqlite::Result<SessionRecord> {
    let status_str: String = row.get(6)?;

    Ok(SessionRecord {
        id: row.get(0)?,
        repo_id: row.get(1)?,
        slot_id: row.get(2)?,
        runtime: row.get(3)?,
        supervisor: row.get(4)?,
        prompt: row.get(5)?,
        status: SessionStatus::from_str(&status_str).map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                6,
                rusqlite::types::Type::Text,
                Box::new(AwoError::unsupported("session status", status_str)),
            )
        })?,
        read_only: row.get::<_, i64>(7)? != 0,
        dry_run: row.get::<_, i64>(8)? != 0,
        command_line: row.get(9)?,
        stdout_path: row.get(10)?,
        stderr_path: row.get(11)?,
        exit_code: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn enable_wal_mode(connection: &Connection) -> AwoResult<()> {
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .map_err(|e| AwoError::store("failed to enable SQLite WAL mode", e))?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(AwoError::invalid_state(format!(
            "failed to enable SQLite WAL mode: journal_mode={journal_mode}"
        )));
    }
    Ok(())
}

fn schema_version(connection: &Connection) -> AwoResult<Option<i64>> {
    connection
        .query_row(
            "SELECT value FROM app_meta WHERE key = ?1",
            [SCHEMA_VERSION_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| AwoError::store("failed to read SQLite schema version", e))?
        .map(|value| {
            value.parse::<i64>().map_err(|error| {
                AwoError::invalid_state(format!("invalid SQLite schema version `{value}`: {error}"))
            })
        })
        .transpose()
}

fn apply_schema_migrations(connection: &Connection, schema_version: i64) -> AwoResult<()> {
    if schema_version > CURRENT_SCHEMA_VERSION {
        return Err(AwoError::invalid_state(format!(
            "unsupported SQLite schema version {schema_version}; expected at most {CURRENT_SCHEMA_VERSION}"
        )));
    }

    if schema_version < 4 {
        connection
            .execute(MIGRATION_V4_ADD_SESSION_SUPERVISOR_SQL, [])
            .map_err(|e| {
                AwoError::store("failed to add `supervisor` column to sessions table", e)
            })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests;
