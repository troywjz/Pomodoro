use std::fmt;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::backup::Backup;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const DATABASE_FILE: &str = "pomodoro.sqlite3";
const BACKUP_DIR: &str = "backups";
const SCHEMA_VERSION: i64 = 1;
const AUTO_BACKUP_INTERVAL_MS: i64 = 24 * 60 * 60 * 1_000;
static FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    Io(io::Error),
    Json(serde_json::Error),
    InvalidRequest(&'static str),
    MissingExpectedRevision(&'static str),
    RevisionConflict {
        resource: &'static str,
        expected: u64,
        actual: u64,
    },
    RevisionOverflow(&'static str),
    CommandIdConflict,
    CorruptDatabase(&'static str),
    UnsupportedSchemaVersion(i64),
    IntegerOutOfRange(&'static str),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(f, "SQLite operation failed: {error}"),
            Self::Io(error) => write!(f, "File operation failed: {error}"),
            Self::Json(error) => write!(f, "JSON operation failed: {error}"),
            Self::InvalidRequest(field) => write!(f, "Invalid request field: {field}"),
            Self::MissingExpectedRevision(resource) => {
                write!(f, "Expected revision is required for {resource}")
            }
            Self::RevisionConflict {
                resource,
                expected,
                actual,
            } => write!(
                f,
                "{resource} revision conflict: expected {expected}, current {actual}"
            ),
            Self::RevisionOverflow(resource) => write!(f, "{resource} revision overflow"),
            Self::CommandIdConflict => write!(f, "Command ID already has a different payload hash"),
            Self::CorruptDatabase(reason) => write!(f, "Database validation failed: {reason}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(f, "Unsupported database schema version {version}")
            }
            Self::IntegerOutOfRange(field) => write!(f, "Value out of SQLite range: {field}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}

impl From<io::Error> for StoreError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupStatus {
    NotDue,
    Created,
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredState {
    pub global_revision: u64,
    pub session_revision: u64,
    pub settings_revision: u64,
    pub checkpoint_json: Option<String>,
    pub settings_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRequest {
    pub command_id: String,
    pub payload_hash: String,
    pub expected_global_revision: u64,
    pub expected_session_revision: Option<u64>,
    pub expected_settings_revision: Option<u64>,
    pub checkpoint_json: Option<String>,
    pub settings_json: Option<String>,
    pub session_record: Option<SessionRecord>,
    pub event: Option<EventRecord>,
    pub committed_utc_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub session_id: String,
    pub phase: String,
    pub target_ms: u64,
    pub actual_active_ms: u64,
    pub started_utc_ms: i64,
    pub ended_utc_ms: i64,
    pub start_local_date: String,
    pub start_offset_minutes: i32,
    pub outcome: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRecord {
    pub event_id: String,
    pub session_id: Option<String>,
    pub kind: String,
    pub occurred_utc_ms: i64,
    pub local_offset_minutes: i32,
    pub payload_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub command_id: String,
    pub global_revision: u64,
    pub session_revision: u64,
    pub settings_revision: u64,
    pub committed_utc_ms: i64,
    pub duplicate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitOutcome {
    pub receipt: Receipt,
    pub backup_status: BackupStatus,
}

pub struct Store {
    conn: Connection,
    data_dir: PathBuf,
    db_path: PathBuf,
    created_utc_ms: i64,
    last_backup_utc_ms: Option<i64>,
    last_backup_revision: u64,
    backup_retry_required: bool,
    open_backup_status: BackupStatus,
}

impl Store {
    pub fn open(data_dir: &Path) -> Result<Self, StoreError> {
        fs::create_dir_all(data_dir)?;
        let db_path = data_dir.join(DATABASE_FILE);
        let conn = Connection::open(&db_path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = FULL;
             PRAGMA foreign_keys = ON;",
        )?;

        let (was_existing_library, created_utc_ms) = initialize_schema(&conn)?;
        let (last_backup_utc_ms, last_backup_revision) = read_backup_state(&conn)?;
        let mut store = Self {
            conn,
            data_dir: data_dir.to_path_buf(),
            db_path,
            created_utc_ms,
            last_backup_utc_ms,
            last_backup_revision,
            backup_retry_required: false,
            open_backup_status: BackupStatus::NotDue,
        };

        if was_existing_library {
            store.backup_retry_required = true;
            let now = utc_now_ms();
            store.open_backup_status = store.create_and_record_backup(now);
        }
        Ok(store)
    }

    /// Returns the automatic backup result produced while opening an existing database.
    pub fn open_backup_status(&self) -> &BackupStatus {
        &self.open_backup_status
    }

    pub fn load_state(&self) -> Result<StoredState, StoreError> {
        let (global_revision, session_revision, settings_revision): (i64, i64, i64) =
            self.conn.query_row(
                "SELECT global_revision, session_revision, settings_revision
                 FROM schema_meta WHERE singleton = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
        let checkpoint_json = self
            .conn
            .query_row(
                "SELECT checkpoint_json FROM active_session WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let settings_json = self
            .conn
            .query_row(
                "SELECT settings_json FROM settings WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        Ok(StoredState {
            global_revision: from_sqlite_u64(global_revision, "global_revision")?,
            session_revision: from_sqlite_u64(session_revision, "session_revision")?,
            settings_revision: from_sqlite_u64(settings_revision, "settings_revision")?,
            checkpoint_json,
            settings_json,
        })
    }

    pub fn commit(&mut self, request: &CommitRequest) -> Result<CommitOutcome, StoreError> {
        validate_request(request)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

        let prior_receipt = tx
            .query_row(
                "SELECT payload_hash, result_payload FROM command_receipts WHERE command_id = ?1",
                [&request.command_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        if let Some((prior_hash, result_payload)) = prior_receipt {
            if prior_hash != request.payload_hash {
                return Err(StoreError::CommandIdConflict);
            }
            let mut receipt: Receipt = serde_json::from_str(&result_payload)?;
            receipt.duplicate = true;
            return Ok(CommitOutcome {
                receipt,
                backup_status: BackupStatus::NotDue,
            });
        }

        let (global_revision, session_revision, settings_revision): (i64, i64, i64) = tx
            .query_row(
                "SELECT global_revision, session_revision, settings_revision
                 FROM schema_meta WHERE singleton = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
        let global_revision = from_sqlite_u64(global_revision, "global_revision")?;
        let session_revision = from_sqlite_u64(session_revision, "session_revision")?;
        let settings_revision = from_sqlite_u64(settings_revision, "settings_revision")?;
        if request.expected_global_revision != global_revision {
            return Err(revision_conflict(
                "global",
                request.expected_global_revision,
                global_revision,
            ));
        }

        let session_changed = request.checkpoint_json.is_some() || request.session_record.is_some();
        if session_changed {
            let expected = request
                .expected_session_revision
                .ok_or(StoreError::MissingExpectedRevision("session"))?;
            if expected != session_revision {
                return Err(revision_conflict("session", expected, session_revision));
            }
        }
        if request.settings_json.is_some() {
            let expected = request
                .expected_settings_revision
                .ok_or(StoreError::MissingExpectedRevision("settings"))?;
            if expected != settings_revision {
                return Err(revision_conflict("settings", expected, settings_revision));
            }
        }

        let next_global_revision = global_revision
            .checked_add(1)
            .ok_or(StoreError::RevisionOverflow("global"))?;
        let next_session_revision = if session_changed {
            session_revision
                .checked_add(1)
                .ok_or(StoreError::RevisionOverflow("session"))?
        } else {
            session_revision
        };
        let next_settings_revision = if request.settings_json.is_some() {
            settings_revision
                .checked_add(1)
                .ok_or(StoreError::RevisionOverflow("settings"))?
        } else {
            settings_revision
        };

        if let Some(checkpoint_json) = &request.checkpoint_json {
            tx.execute(
                "INSERT INTO active_session(singleton, checkpoint_json, session_revision, updated_utc_ms)
                 VALUES (1, ?1, ?2, ?3)
                 ON CONFLICT(singleton) DO UPDATE SET
                    checkpoint_json = excluded.checkpoint_json,
                    session_revision = excluded.session_revision,
                    updated_utc_ms = excluded.updated_utc_ms",
                params![checkpoint_json, to_sqlite_i64(next_session_revision, "session_revision")?, request.committed_utc_ms],
            )?;
        }

        if let Some(settings_json) = &request.settings_json {
            tx.execute(
                "INSERT INTO settings(singleton, settings_json, settings_revision, updated_utc_ms)
                 VALUES (1, ?1, ?2, ?3)
                 ON CONFLICT(singleton) DO UPDATE SET
                    settings_json = excluded.settings_json,
                    settings_revision = excluded.settings_revision,
                    updated_utc_ms = excluded.updated_utc_ms",
                params![
                    settings_json,
                    to_sqlite_i64(next_settings_revision, "settings_revision")?,
                    request.committed_utc_ms
                ],
            )?;
            tx.execute(
                "INSERT INTO settings_history(settings_revision, settings_json, updated_utc_ms)
                 VALUES (?1, ?2, ?3)",
                params![
                    to_sqlite_i64(next_settings_revision, "settings_revision")?,
                    settings_json,
                    request.committed_utc_ms
                ],
            )?;
        }

        if let Some(record) = &request.session_record {
            tx.execute(
                "INSERT INTO session_history(
                    session_id, phase, target_ms, actual_active_ms, started_utc_ms, ended_utc_ms,
                    start_local_date, start_offset_minutes, outcome, reason
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    record.session_id,
                    record.phase,
                    to_sqlite_i64(record.target_ms, "target_ms")?,
                    to_sqlite_i64(record.actual_active_ms, "actual_active_ms")?,
                    record.started_utc_ms,
                    record.ended_utc_ms,
                    record.start_local_date,
                    record.start_offset_minutes,
                    record.outcome,
                    record.reason
                ],
            )?;
            if request.checkpoint_json.is_none() {
                tx.execute("DELETE FROM active_session WHERE singleton = 1", [])?;
            }
        }

        if let Some(event) = &request.event {
            tx.execute(
                "INSERT INTO event_log(
                    event_id, session_id, kind, occurred_utc_ms, local_offset_minutes, payload_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    event.event_id,
                    event.session_id,
                    event.kind,
                    event.occurred_utc_ms,
                    event.local_offset_minutes,
                    event.payload_json
                ],
            )?;
        }

        let changed = tx.execute(
            "UPDATE schema_meta SET global_revision = ?1, session_revision = ?2,
                    settings_revision = ?3 WHERE singleton = 1 AND global_revision = ?4",
            params![
                to_sqlite_i64(next_global_revision, "global_revision")?,
                to_sqlite_i64(next_session_revision, "session_revision")?,
                to_sqlite_i64(next_settings_revision, "settings_revision")?,
                to_sqlite_i64(global_revision, "global_revision")?
            ],
        )?;
        if changed != 1 {
            return Err(revision_conflict(
                "global",
                request.expected_global_revision,
                current_global_revision(&tx)?,
            ));
        }

        let receipt = Receipt {
            command_id: request.command_id.clone(),
            global_revision: next_global_revision,
            session_revision: next_session_revision,
            settings_revision: next_settings_revision,
            committed_utc_ms: request.committed_utc_ms,
            duplicate: false,
        };
        let receipt_json = serde_json::to_string(&receipt)?;
        tx.execute(
            "INSERT INTO command_receipts(
                command_id, payload_hash, result_payload, global_revision,
                session_revision, settings_revision, committed_utc_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                request.command_id,
                request.payload_hash,
                receipt_json,
                to_sqlite_i64(next_global_revision, "global_revision")?,
                to_sqlite_i64(next_session_revision, "session_revision")?,
                to_sqlite_i64(next_settings_revision, "settings_revision")?,
                request.committed_utc_ms
            ],
        )?;
        tx.commit()?;

        let backup_status = if request.session_record.is_some() {
            self.backup_retry_required = true;
            self.create_and_record_backup(request.committed_utc_ms)
        } else {
            BackupStatus::NotDue
        };
        Ok(CommitOutcome {
            receipt,
            backup_status,
        })
    }

    pub fn maybe_auto_backup(&mut self, now_utc_ms: i64) -> BackupStatus {
        let current_revision = match current_global_revision(&self.conn) {
            Ok(value) => value,
            Err(error) => {
                return BackupStatus::Failed {
                    message: error.to_string(),
                };
            }
        };
        let last_success_revision = self.last_backup_revision;
        let has_changes = current_revision > last_success_revision;
        let anchor = self.last_backup_utc_ms.unwrap_or(self.created_utc_ms);
        let due = now_utc_ms
            .checked_sub(anchor)
            .is_some_and(|elapsed| elapsed >= AUTO_BACKUP_INTERVAL_MS);
        if self.backup_retry_required || (due && has_changes) {
            self.create_and_record_backup(now_utc_ms)
        } else {
            BackupStatus::NotDue
        }
    }

    pub fn export_json(&self, destination: &Path) -> Result<(), StoreError> {
        if is_protected_export_path(destination, &self.data_dir, &self.db_path)? {
            return Err(StoreError::InvalidRequest(
                "export destination is application-managed storage",
            ));
        }

        let tx = self.conn.unchecked_transaction()?;
        let (global_revision, session_revision, settings_revision): (i64, i64, i64) = tx
            .query_row(
                "SELECT global_revision, session_revision, settings_revision
                 FROM schema_meta WHERE singleton = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
        let checkpoint_json = tx
            .query_row(
                "SELECT checkpoint_json FROM active_session WHERE singleton = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let settings_json = tx
            .query_row(
                "SELECT settings_json FROM settings WHERE singleton = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let sessions = read_sessions(&tx)?;
        let settings_history = read_settings_history(&tx)?;
        let events = read_events(&tx)?;
        let envelope = json!({
            "schema_version": SCHEMA_VERSION,
            "exported_utc_ms": utc_now_ms(),
            "state": {
                "global_revision": from_sqlite_u64(global_revision, "global_revision")?,
                "session_revision": from_sqlite_u64(session_revision, "session_revision")?,
                "settings_revision": from_sqlite_u64(settings_revision, "settings_revision")?,
                "checkpoint": checkpoint_json.map(|value| serde_json::from_str::<Value>(&value)).transpose()?
            },
            "settings": settings_json.map(|value| serde_json::from_str::<Value>(&value)).transpose()?,
            "settings_history": settings_history,
            "sessions": sessions,
            "events": events
        });
        let json_text = serde_json::to_vec_pretty(&envelope)?;
        tx.commit()?;

        write_atomically(destination, &json_text)?;
        Ok(())
    }

    fn create_and_record_backup(&mut self, backup_utc_ms: i64) -> BackupStatus {
        let revision = match current_global_revision(&self.conn) {
            Ok(value) => value,
            Err(error) => {
                self.backup_retry_required = true;
                return BackupStatus::Failed {
                    message: error.to_string(),
                };
            }
        };
        match create_valid_backup(&self.conn, &self.data_dir, backup_utc_ms, revision) {
            Ok(()) => {
                self.last_backup_utc_ms = Some(backup_utc_ms);
                self.last_backup_revision = revision;
                self.backup_retry_required = false;
                if let Ok(revision) = to_sqlite_i64(revision, "global_revision") {
                    let _ = self.conn.execute(
                        "UPDATE backup_state SET last_backup_utc_ms = ?1, last_backup_revision = ?2
                         WHERE singleton = 1",
                        params![backup_utc_ms, revision],
                    );
                }
                BackupStatus::Created
            }
            Err(error) => {
                self.backup_retry_required = true;
                BackupStatus::Failed {
                    message: error.to_string(),
                }
            }
        }
    }
}

fn initialize_schema(conn: &Connection) -> Result<(bool, i64), StoreError> {
    let table_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )?;
    if table_count == 0 {
        let created_utc_ms = utc_now_ms();
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(
            "CREATE TABLE schema_meta (
                singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
                schema_version INTEGER NOT NULL,
                global_revision INTEGER NOT NULL CHECK(global_revision >= 0),
                session_revision INTEGER NOT NULL CHECK(session_revision >= 0),
                settings_revision INTEGER NOT NULL CHECK(settings_revision >= 0),
                created_utc_ms INTEGER NOT NULL
             );
             CREATE TABLE active_session (
                singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
                checkpoint_json TEXT NOT NULL,
                session_revision INTEGER NOT NULL,
                updated_utc_ms INTEGER NOT NULL
             );
             CREATE TABLE settings (
                singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
                settings_json TEXT NOT NULL,
                settings_revision INTEGER NOT NULL,
                updated_utc_ms INTEGER NOT NULL
             );
             CREATE TABLE settings_history (
                settings_revision INTEGER PRIMARY KEY CHECK(settings_revision > 0),
                settings_json TEXT NOT NULL,
                updated_utc_ms INTEGER NOT NULL
             );
             CREATE TABLE session_history (
                session_id TEXT PRIMARY KEY,
                phase TEXT NOT NULL,
                target_ms INTEGER NOT NULL CHECK(target_ms >= 0),
                actual_active_ms INTEGER NOT NULL CHECK(actual_active_ms >= 0),
                started_utc_ms INTEGER NOT NULL,
                ended_utc_ms INTEGER NOT NULL,
                start_local_date TEXT NOT NULL,
                start_offset_minutes INTEGER NOT NULL,
                outcome TEXT NOT NULL,
                reason TEXT
             );
             CREATE TABLE event_log (
                event_id TEXT PRIMARY KEY,
                session_id TEXT,
                kind TEXT NOT NULL,
                occurred_utc_ms INTEGER NOT NULL,
                local_offset_minutes INTEGER NOT NULL,
                payload_json TEXT NOT NULL
             );
             CREATE TABLE command_receipts (
                command_id TEXT PRIMARY KEY,
                payload_hash TEXT NOT NULL,
                result_payload TEXT NOT NULL,
                global_revision INTEGER NOT NULL,
                session_revision INTEGER NOT NULL,
                settings_revision INTEGER NOT NULL,
                committed_utc_ms INTEGER NOT NULL
             );
             CREATE TABLE backup_state (
                singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
                last_backup_utc_ms INTEGER,
                last_backup_revision INTEGER NOT NULL CHECK(last_backup_revision >= 0)
             );",
        )?;
        tx.execute(
            "INSERT INTO schema_meta VALUES (1, ?1, 0, 0, 0, ?2)",
            params![SCHEMA_VERSION, created_utc_ms],
        )?;
        tx.execute("INSERT INTO backup_state VALUES (1, NULL, 0)", [])?;
        tx.execute_batch("PRAGMA user_version = 1;")?;
        tx.commit()?;
        return Ok((false, created_utc_ms));
    }

    let has_meta: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_meta')",
        [],
        |row| row.get(0),
    )?;
    if !has_meta {
        return Err(StoreError::CorruptDatabase("schema_meta table is missing"));
    }
    let (version, created_utc_ms): (i64, i64) = conn.query_row(
        "SELECT schema_version, created_utc_ms FROM schema_meta WHERE singleton = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if version != SCHEMA_VERSION {
        return Err(StoreError::UnsupportedSchemaVersion(version));
    }
    for table in [
        "active_session",
        "settings",
        "settings_history",
        "session_history",
        "event_log",
        "command_receipts",
        "backup_state",
    ] {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(StoreError::CorruptDatabase("required table is missing"));
        }
    }
    Ok((true, created_utc_ms))
}

fn read_backup_state(conn: &Connection) -> Result<(Option<i64>, u64), StoreError> {
    let (last_backup_utc_ms, last_backup_revision): (Option<i64>, i64) = conn.query_row(
        "SELECT last_backup_utc_ms, last_backup_revision FROM backup_state WHERE singleton = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok((
        last_backup_utc_ms,
        from_sqlite_u64(last_backup_revision, "last_backup_revision")?,
    ))
}

fn validate_request(request: &CommitRequest) -> Result<(), StoreError> {
    if request.command_id.trim().is_empty() {
        return Err(StoreError::InvalidRequest("command_id"));
    }
    if request.payload_hash.len() != 64
        || !request
            .payload_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(StoreError::InvalidRequest("payload_hash"));
    }
    if let Some(value) = &request.checkpoint_json {
        serde_json::from_str::<Value>(value)?;
    }
    if let Some(value) = &request.settings_json {
        serde_json::from_str::<Value>(value)?;
    }
    if let Some(event) = &request.event {
        serde_json::from_str::<Value>(&event.payload_json)?;
    }
    if let Some(record) = &request.session_record {
        to_sqlite_i64(record.target_ms, "target_ms")?;
        to_sqlite_i64(record.actual_active_ms, "actual_active_ms")?;
    }
    Ok(())
}

fn create_valid_backup(
    source: &Connection,
    data_dir: &Path,
    backup_utc_ms: i64,
    global_revision: u64,
) -> Result<(), StoreError> {
    let backup_dir = data_dir.join(BACKUP_DIR);
    fs::create_dir_all(&backup_dir)?;
    let revision = to_sqlite_i64(global_revision, "global_revision")?;
    let (partial_path, mut destination) =
        create_partial_backup(&backup_dir, backup_utc_ms, revision)?;
    let result = (|| -> Result<(), StoreError> {
        {
            let backup = Backup::new(source, &mut destination)?;
            backup.run_to_completion(128, Duration::from_millis(10), None)?;
        }
        destination
            .close()
            .map_err(|(_, error)| StoreError::Sqlite(error))?;
        validate_backup(&partial_path)?;
        publish_backup_no_replace(
            &partial_path,
            &backup_dir,
            backup_utc_ms,
            revision,
            FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        )?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&partial_path);
    }
    result
}

fn create_partial_backup(
    backup_dir: &Path,
    backup_utc_ms: i64,
    revision: i64,
) -> Result<(PathBuf, Connection), StoreError> {
    for _ in 0..100 {
        let sequence = FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = backup_dir.join(format!(
            "backup-{backup_utc_ms}-{revision}-{sequence}.partial"
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => {
                drop(file);
                return Ok((
                    path.clone(),
                    Connection::open(path).map_err(StoreError::Sqlite)?,
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(StoreError::Io(error)),
        }
    }
    Err(StoreError::CorruptDatabase(
        "could not allocate a backup filename",
    ))
}

fn validate_backup(path: &Path) -> Result<(), StoreError> {
    let conn = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(StoreError::CorruptDatabase("backup integrity_check failed"));
    }
    let version: i64 = conn.query_row(
        "SELECT schema_version FROM schema_meta WHERE singleton = 1",
        [],
        |row| row.get(0),
    )?;
    if version != SCHEMA_VERSION {
        return Err(StoreError::UnsupportedSchemaVersion(version));
    }
    for table in [
        "settings",
        "settings_history",
        "active_session",
        "session_history",
        "event_log",
        "command_receipts",
    ] {
        let _: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })?;
    }
    Ok(())
}

fn current_global_revision(conn: &Connection) -> Result<u64, StoreError> {
    let revision: i64 = conn.query_row(
        "SELECT global_revision FROM schema_meta WHERE singleton = 1",
        [],
        |row| row.get(0),
    )?;
    from_sqlite_u64(revision, "global_revision")
}

fn revision_conflict(resource: &'static str, expected: u64, actual: u64) -> StoreError {
    StoreError::RevisionConflict {
        resource,
        expected,
        actual,
    }
}

fn from_sqlite_u64(value: i64, field: &'static str) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::CorruptDatabase(field))
}

fn to_sqlite_i64(value: u64, field: &'static str) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::IntegerOutOfRange(field))
}

fn utc_now_ms() -> i64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    i64::try_from(millis).unwrap_or(i64::MAX)
}

fn is_protected_export_path(
    destination: &Path,
    data_dir: &Path,
    database: &Path,
) -> Result<bool, StoreError> {
    let lexical_destination = lexical_absolute_path(destination)?;
    let lexical_backup_root = lexical_absolute_path(&data_dir.join(BACKUP_DIR))?;
    if is_same_or_descendant_path(&lexical_destination, &lexical_backup_root) {
        return Ok(true);
    }

    let destination = canonicalize_with_missing_suffix(destination)?;
    let database = canonicalize_with_missing_suffix(database)?;
    if same_path(&destination, &database) {
        return Ok(true);
    }

    for suffix in ["-wal", "-shm", "-journal"] {
        let mut sidecar_name = database
            .file_name()
            .ok_or(StoreError::InvalidRequest("database path"))?
            .to_os_string();
        sidecar_name.push(suffix);
        let sidecar = database.with_file_name(sidecar_name);
        if same_path(&destination, &canonicalize_with_missing_suffix(&sidecar)?) {
            return Ok(true);
        }
    }

    let backup_root = canonicalize_with_missing_suffix(&data_dir.join(BACKUP_DIR))?;
    Ok(is_same_or_descendant_path(&destination, &backup_root))
}

fn canonicalize_with_missing_suffix(path: &Path) -> Result<PathBuf, StoreError> {
    let normalized = lexical_absolute_path(path)?;
    let mut existing = normalized.clone();
    let mut suffix = Vec::new();
    while !existing.exists() {
        let name = existing
            .file_name()
            .ok_or(StoreError::InvalidRequest("export destination"))?;
        suffix.push(name.to_os_string());
        if !existing.pop() {
            return Err(StoreError::InvalidRequest("export destination"));
        }
    }

    let mut resolved = existing.canonicalize()?;
    for component in suffix.iter().rev() {
        resolved.push(component);
    }
    Ok(normalize_path(&resolved))
}

fn lexical_absolute_path(path: &Path) -> Result<PathBuf, StoreError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(normalize_path(&absolute))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() && !normalized.has_root() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

fn same_path(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn is_same_or_descendant_path(path: &Path, root: &Path) -> bool {
    if same_path(path, root) {
        return true;
    }
    #[cfg(windows)]
    {
        let path = path.to_string_lossy().replace('/', "\\");
        let root = root
            .to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_owned();
        path.to_lowercase()
            .starts_with(&format!("{}\\", root.to_lowercase()))
    }
    #[cfg(not(windows))]
    {
        path.starts_with(root)
    }
}

fn publish_backup_no_replace(
    partial_path: &Path,
    backup_dir: &Path,
    backup_utc_ms: i64,
    revision: i64,
    first_sequence: u64,
) -> Result<PathBuf, StoreError> {
    for offset in 0..100u64 {
        let sequence = first_sequence.saturating_add(offset);
        let final_path = backup_dir.join(format!(
            "backup-{backup_utc_ms}-{revision}-{sequence}.sqlite3"
        ));
        match fs::hard_link(partial_path, &final_path) {
            Ok(()) => {
                fs::remove_file(partial_path)?;
                return Ok(final_path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(StoreError::Io(error)),
        }
    }
    Err(StoreError::CorruptDatabase(
        "could not allocate a non-overwriting backup filename",
    ))
}

fn write_atomically(destination: &Path, contents: &[u8]) -> Result<(), StoreError> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = destination
        .file_name()
        .ok_or(StoreError::InvalidRequest("export destination"))?;
    for _ in 0..100 {
        let sequence = FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".{}.partial-{sequence}", name.to_string_lossy()));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(mut file) => {
                use std::io::Write;
                let write_result = file.write_all(contents).and_then(|()| file.sync_all());
                drop(file);
                if let Err(error) = write_result {
                    let _ = fs::remove_file(&temporary);
                    return Err(StoreError::Io(error));
                }
                if let Err(error) = fs::rename(&temporary, destination) {
                    let _ = fs::remove_file(&temporary);
                    return Err(StoreError::Io(error));
                }
                return Ok(());
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(StoreError::Io(error)),
        }
    }
    Err(StoreError::CorruptDatabase(
        "could not allocate an export filename",
    ))
}

fn read_sessions(conn: &Connection) -> Result<Vec<Value>, StoreError> {
    let mut statement = conn.prepare(
        "SELECT session_id, phase, target_ms, actual_active_ms, started_utc_ms, ended_utc_ms,
                start_local_date, start_offset_minutes, outcome, reason
         FROM session_history ORDER BY ended_utc_ms, session_id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(json!({
            "session_id": row.get::<_, String>(0)?,
            "phase": row.get::<_, String>(1)?,
            "target_ms": row.get::<_, i64>(2)?,
            "actual_active_ms": row.get::<_, i64>(3)?,
            "started_utc_ms": row.get::<_, i64>(4)?,
            "ended_utc_ms": row.get::<_, i64>(5)?,
            "start_local_date": row.get::<_, String>(6)?,
            "start_offset_minutes": row.get::<_, i32>(7)?,
            "outcome": row.get::<_, String>(8)?,
            "reason": row.get::<_, Option<String>>(9)?
        }))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn read_settings_history(conn: &Connection) -> Result<Vec<Value>, StoreError> {
    let mut statement = conn.prepare(
        "SELECT settings_revision, settings_json, updated_utc_ms
         FROM settings_history ORDER BY settings_revision",
    )?;
    let rows = statement.query_map([], |row| {
        let settings_json: String = row.get(1)?;
        let settings: Value = serde_json::from_str(&settings_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
        Ok(json!({
            "settings_revision": row.get::<_, i64>(0)?,
            "settings": settings,
            "updated_utc_ms": row.get::<_, i64>(2)?
        }))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn read_events(conn: &Connection) -> Result<Vec<Value>, StoreError> {
    let mut statement = conn.prepare(
        "SELECT event_id, session_id, kind, occurred_utc_ms, local_offset_minutes, payload_json
         FROM event_log ORDER BY occurred_utc_ms, event_id",
    )?;
    let rows = statement.query_map([], |row| {
        let payload_json: String = row.get(5)?;
        let payload: Value = serde_json::from_str(&payload_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                5,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
        Ok(json!({
            "event_id": row.get::<_, String>(0)?,
            "session_id": row.get::<_, Option<String>>(1)?,
            "kind": row.get::<_, String>(2)?,
            "occurred_utc_ms": row.get::<_, i64>(3)?,
            "local_offset_minutes": row.get::<_, i32>(4)?,
            "payload": payload
        }))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn hash(value: char) -> String {
        std::iter::repeat_n(value, 64).collect()
    }

    fn event(
        id: &str,
        session: &str,
        kind: &str,
        at: i64,
        offset: i32,
        payload: &str,
    ) -> EventRecord {
        EventRecord {
            event_id: id.to_owned(),
            session_id: Some(session.to_owned()),
            kind: kind.to_owned(),
            occurred_utc_ms: at,
            local_offset_minutes: offset,
            payload_json: payload.to_owned(),
        }
    }

    fn record(id: &str, started: i64, ended: i64, offset: i32, outcome: &str) -> SessionRecord {
        SessionRecord {
            session_id: id.to_owned(),
            phase: "focus".to_owned(),
            target_ms: 1_500_000,
            actual_active_ms: 1_234_567,
            started_utc_ms: started,
            ended_utc_ms: ended,
            start_local_date: "2026-11-18".to_owned(),
            start_offset_minutes: offset,
            outcome: outcome.to_owned(),
            reason: Some("user_action".to_owned()),
        }
    }

    fn request(id: &str, hash_marker: char, global: u64, now: i64) -> CommitRequest {
        CommitRequest {
            command_id: id.to_owned(),
            payload_hash: hash(hash_marker),
            expected_global_revision: global,
            expected_session_revision: None,
            expected_settings_revision: None,
            checkpoint_json: None,
            settings_json: None,
            session_record: None,
            event: None,
            committed_utc_ms: now,
        }
    }

    fn backup_paths(data_dir: &Path) -> Vec<PathBuf> {
        let backup_dir = data_dir.join(BACKUP_DIR);
        if !backup_dir.exists() {
            return Vec::new();
        }
        fs::read_dir(backup_dir)
            .expect("backup directory readable")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "sqlite3")
            })
            .collect()
    }

    fn assert_backup_contents(path: &Path, sessions: i64, events: i64, receipts: i64) {
        let conn = Connection::open(path).expect("backup opens as SQLite");
        let integrity: String = conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .expect("integrity_check runs");
        assert_eq!(integrity, "ok");
        let version: i64 = conn
            .query_row(
                "SELECT schema_version FROM schema_meta WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .expect("schema readable");
        assert_eq!(version, SCHEMA_VERSION);
        for (table, expected) in [
            ("session_history", sessions),
            ("event_log", events),
            ("command_receipts", receipts),
        ] {
            let actual: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("backup table readable");
            assert_eq!(actual, expected, "{table} row count");
        }
    }

    #[test]
    fn opening_existing_database_and_terminal_commit_create_verified_backups() {
        let directory = tempdir().expect("temporary data directory");
        let mut store = Store::open(directory.path()).expect("new store opens");
        assert_eq!(store.open_backup_status(), &BackupStatus::NotDue);
        assert!(backup_paths(directory.path()).is_empty());

        let mut start = request("start", 'a', 0, 1_900_000_000_000);
        start.expected_session_revision = Some(0);
        start.checkpoint_json = Some(r#"{"session_id":"s1","active_ms":0}"#.to_owned());
        start.event = Some(event(
            "e-start",
            "s1",
            "started",
            1_900_000_000_000,
            -360,
            r#"{"target_ms":1500000}"#,
        ));
        assert_eq!(
            store.commit(&start).expect("start persists").backup_status,
            BackupStatus::NotDue
        );
        drop(store);

        let mut store = Store::open(directory.path()).expect("existing store opens");
        assert_eq!(store.open_backup_status(), &BackupStatus::Created);
        let startup_backups = backup_paths(directory.path());
        assert_eq!(startup_backups.len(), 1);
        assert_backup_contents(&startup_backups[0], 0, 1, 1);

        let mut finish = request("finish", 'b', 1, 1_900_000_003_000);
        finish.expected_session_revision = Some(1);
        finish.session_record = Some(record(
            "s1",
            1_900_000_000_000,
            1_900_000_003_000,
            -360,
            "completed",
        ));
        let final_checkpoint = r#"{"phase":"short_break","round_index":1}"#;
        finish.checkpoint_json = Some(final_checkpoint.to_owned());
        finish.event = Some(event(
            "e-finish",
            "s1",
            "completed",
            1_900_000_003_000,
            -360,
            r#"{"actual_active_ms":1234567}"#,
        ));
        let result = store
            .commit(&finish)
            .expect("terminal data persists even with backup");
        assert_eq!(result.receipt.global_revision, 2);
        assert_eq!(result.backup_status, BackupStatus::Created);
        assert_eq!(
            store.load_state().unwrap().checkpoint_json.as_deref(),
            Some(final_checkpoint)
        );
        let all_backups = backup_paths(directory.path());
        assert_eq!(all_backups.len(), 2);
        let terminal_backup = all_backups
            .iter()
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().contains("1900000003000"))
            })
            .expect("terminal commit backup exists");
        assert_backup_contents(terminal_backup, 1, 2, 2);
        let backup = Connection::open(terminal_backup).expect("terminal backup opens");
        let backup_checkpoint: String = backup
            .query_row(
                "SELECT checkpoint_json FROM active_session WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .expect("terminal snapshot is in the same backup");
        assert_eq!(backup_checkpoint, final_checkpoint);
        drop(backup);
        drop(store);

        let store = Store::open(directory.path()).expect("restart restores terminal snapshot");
        assert_eq!(store.open_backup_status(), &BackupStatus::Created);
        let restored = store
            .load_state()
            .expect("completion state reads after restart");
        assert_eq!(restored.session_revision, 2);
        let restored_checkpoint: Value =
            serde_json::from_str(restored.checkpoint_json.as_deref().unwrap()).unwrap();
        assert_eq!(restored_checkpoint["phase"], "short_break");
        assert_eq!(restored_checkpoint["round_index"], 1);
        assert_eq!(backup_paths(directory.path()).len(), 3);
    }

    #[test]
    fn retains_sessions_events_offsets_settings_and_reads_them_after_restart() {
        let directory = tempdir().expect("temporary data directory");
        let mut store = Store::open(directory.path()).expect("store opens");

        let mut start = request("start-one", 'a', 0, 1_800_000_000_000);
        start.expected_session_revision = Some(0);
        start.expected_settings_revision = Some(0);
        start.checkpoint_json = Some(r#"{"session_id":"s1","remaining_ms":1000}"#.to_owned());
        start.settings_json =
            Some(r#"{"schema_version":1,"focus_ms":1500000,"theme_mode":"system"}"#.to_owned());
        start.event = Some(event(
            "s1-start",
            "s1",
            "started",
            1_800_000_000_000,
            345,
            r#"{"phase":"focus"}"#,
        ));
        store.commit(&start).expect("start persists");

        let mut pause = request("pause-one", 'b', 1, 1_800_000_001_000);
        pause.event = Some(event(
            "s1-pause",
            "s1",
            "paused",
            1_800_000_001_000,
            345,
            r#"{"reason":"user"}"#,
        ));
        store.commit(&pause).expect("pause event persists");
        let mut resume = request("resume-one", 'c', 2, 1_800_000_002_000);
        resume.event = Some(event(
            "s1-resume",
            "s1",
            "resumed",
            1_800_000_002_000,
            345,
            r#"{"source":"button"}"#,
        ));
        store.commit(&resume).expect("resume event persists");

        let mut settings_update = request("settings-two", 'f', 3, 1_800_000_002_500);
        settings_update.expected_settings_revision = Some(1);
        settings_update.settings_json =
            Some(r#"{"schema_version":1,"focus_ms":1800000,"theme_mode":"system"}"#.to_owned());
        store
            .commit(&settings_update)
            .expect("new settings revision persists alongside history");

        let mut finish_one = request("finish-one", 'd', 4, 1_800_000_003_000);
        finish_one.expected_session_revision = Some(1);
        finish_one.session_record = Some(record(
            "s1",
            1_800_000_000_000,
            1_800_000_003_000,
            345,
            "completed",
        ));
        finish_one.event = Some(event(
            "s1-complete",
            "s1",
            "completed",
            1_800_000_003_000,
            345,
            r#"{"reason":"elapsed"}"#,
        ));
        store.commit(&finish_one).expect("first session ends");

        let mut finish_two = request("finish-two", 'e', 5, 1_800_000_006_000);
        finish_two.expected_session_revision = Some(2);
        finish_two.session_record = Some(record(
            "s2",
            1_800_000_004_000,
            1_800_000_006_000,
            -300,
            "interrupted",
        ));
        finish_two.event = Some(event(
            "s2-interrupt",
            "s2",
            "interrupted",
            1_800_000_006_000,
            -300,
            r#"{"reason":"exit"}"#,
        ));
        store.commit(&finish_two).expect("second session ends");
        assert_eq!(
            store.load_state().expect("state reads").settings_revision,
            2
        );
        drop(store);

        let store = Store::open(directory.path()).expect("restart reads database");
        let state = store.load_state().expect("state reads after restart");
        assert_eq!(state.global_revision, 6);
        assert_eq!(state.session_revision, 3);
        assert_eq!(state.settings_revision, 2);
        assert_eq!(state.checkpoint_json, None);
        assert_eq!(
            serde_json::from_str::<Value>(state.settings_json.as_deref().unwrap()).unwrap()["focus_ms"],
            1_800_000
        );

        let export = directory.path().join("analysis.json");
        store
            .export_json(&export)
            .expect("analysis export succeeds");
        let exported: Value =
            serde_json::from_slice(&fs::read(&export).expect("export file reads")).unwrap();
        assert_eq!(exported["sessions"].as_array().unwrap().len(), 2);
        assert_eq!(exported["settings_history"].as_array().unwrap().len(), 2);
        assert_eq!(exported["settings_history"][0]["settings_revision"], 1);
        assert_eq!(
            exported["settings_history"][0]["settings"]["focus_ms"],
            1_500_000
        );
        assert_eq!(
            exported["settings_history"][1]["settings"]["focus_ms"],
            1_800_000
        );
        assert_eq!(exported["events"].as_array().unwrap().len(), 5);
        assert_eq!(exported["sessions"][0]["start_offset_minutes"], 345);
        assert_eq!(exported["sessions"][1]["start_offset_minutes"], -300);
        assert_eq!(exported["events"][0]["payload"]["phase"], "focus");
        assert_eq!(exported["settings"]["focus_ms"], 1_800_000);
        assert_eq!(store.load_state().unwrap().global_revision, 6);
    }

    #[test]
    fn receipts_are_idempotent_and_revision_conflicts_do_not_write() {
        let directory = tempdir().expect("temporary data directory");
        let mut store = Store::open(directory.path()).expect("store opens");
        let mut first = request("same-command", 'a', 0, 10);
        first.expected_session_revision = Some(0);
        first.checkpoint_json = Some(r#"{"active_ms":10}"#.to_owned());
        let first_receipt = store.commit(&first).expect("first commit succeeds").receipt;

        let duplicate = store.commit(&first).expect("same command replays receipt");
        assert!(duplicate.receipt.duplicate);
        assert_eq!(
            duplicate.receipt.global_revision,
            first_receipt.global_revision
        );
        assert_eq!(duplicate.backup_status, BackupStatus::NotDue);

        let mut changed = first.clone();
        changed.payload_hash = hash('b');
        assert!(matches!(
            store.commit(&changed),
            Err(StoreError::CommandIdConflict)
        ));

        let mut stale = request("stale-command", 'c', 0, 20);
        stale.event = Some(event("stale-event", "s1", "paused", 20, 0, "{}"));
        assert!(matches!(
            store.commit(&stale),
            Err(StoreError::RevisionConflict {
                resource: "global",
                expected: 0,
                actual: 1
            })
        ));
        let state = store.load_state().expect("state reads");
        assert_eq!(state.global_revision, 1);
        assert_eq!(state.session_revision, 1);
    }

    #[test]
    fn failed_business_transaction_rolls_back_revision_event_and_receipt() {
        let directory = tempdir().expect("temporary data directory");
        let mut store = Store::open(directory.path()).expect("store opens");
        let mut first = request("first-terminal", 'a', 0, 100);
        first.expected_session_revision = Some(0);
        first.session_record = Some(record("same-session", 0, 100, 0, "completed"));
        first.event = Some(event(
            "first-event",
            "same-session",
            "completed",
            100,
            0,
            "{}",
        ));
        store.commit(&first).expect("first record commits");

        let mut failed = request("retry-after-failure", 'b', 1, 200);
        failed.expected_session_revision = Some(1);
        failed.checkpoint_json = Some(r#"{"phase":"next","round_index":2}"#.to_owned());
        failed.session_record = Some(record("same-session", 100, 200, 0, "completed"));
        failed.event = Some(event(
            "failed-event",
            "same-session",
            "completed",
            200,
            0,
            "{}",
        ));
        assert!(matches!(store.commit(&failed), Err(StoreError::Sqlite(_))));
        let state = store.load_state().expect("state still reads");
        assert_eq!(state.global_revision, 1);
        assert_eq!(state.session_revision, 1);
        assert_eq!(
            state.checkpoint_json, None,
            "failed final snapshot rolls back"
        );

        failed.session_record.as_mut().unwrap().session_id = "next-session".to_owned();
        failed.event.as_mut().unwrap().session_id = Some("next-session".to_owned());
        assert!(
            store.commit(&failed).is_ok(),
            "failed transaction left no command receipt"
        );
        assert_eq!(
            store.load_state().unwrap().checkpoint_json.as_deref(),
            Some(r#"{"phase":"next","round_index":2}"#)
        );
    }

    #[test]
    fn backup_directory_failure_keeps_committed_primary_data_and_can_retry() {
        let directory = tempdir().expect("temporary data directory");
        let mut store = Store::open(directory.path()).expect("new store opens");
        fs::write(directory.path().join(BACKUP_DIR), b"blocked").expect("block backup directory");

        let mut finish = request("finish-with-backup-error", 'a', 0, 1_700_000_000_000);
        finish.expected_session_revision = Some(0);
        finish.session_record = Some(record(
            "kept-session",
            1_699_999_000_000,
            1_700_000_000_000,
            60,
            "completed",
        ));
        let result = store.commit(&finish).expect("business commit succeeds");
        assert_eq!(result.receipt.global_revision, 1);
        assert!(
            matches!(result.backup_status, BackupStatus::Failed { ref message } if !message.is_empty())
        );
        assert_eq!(store.load_state().unwrap().global_revision, 1);

        fs::remove_file(directory.path().join(BACKUP_DIR))
            .expect("remove injected directory fault");
        assert_eq!(
            store.maybe_auto_backup(1_700_000_000_001),
            BackupStatus::Created,
            "a failed backup remains retryable"
        );
        let backups = backup_paths(directory.path());
        assert_eq!(backups.len(), 1);
        assert_backup_contents(&backups[0], 1, 0, 1);
        drop(store);

        let store = Store::open(directory.path()).expect("restart opens the persisted database");
        assert_eq!(store.open_backup_status(), &BackupStatus::Created);
        let backups = backup_paths(directory.path());
        assert_eq!(backups.len(), 2, "successful backups remain retained");
        for backup in backups {
            assert_backup_contents(&backup, 1, 0, 1);
        }
    }

    #[test]
    fn periodic_backup_waits_for_interval_and_requires_changes() {
        let directory = tempdir().expect("temporary data directory");
        let mut store = Store::open(directory.path()).expect("store opens");
        let future = utc_now_ms().saturating_add(AUTO_BACKUP_INTERVAL_MS + 10_000);
        assert_eq!(store.maybe_auto_backup(future), BackupStatus::NotDue);

        let mut update = request("checkpoint", 'a', 0, future - AUTO_BACKUP_INTERVAL_MS);
        update.expected_session_revision = Some(0);
        update.checkpoint_json = Some(r#"{"session_id":"periodic","active_ms":50}"#.to_owned());
        store.commit(&update).expect("checkpoint commits");
        assert_eq!(store.maybe_auto_backup(future), BackupStatus::Created);
        assert_eq!(backup_paths(directory.path()).len(), 1);
        assert_eq!(
            store.maybe_auto_backup(future + AUTO_BACKUP_INTERVAL_MS),
            BackupStatus::NotDue
        );
    }

    #[test]
    fn export_rejects_database_sidecars_and_backup_tree_without_writing() {
        let directory = tempdir().expect("temporary data directory");
        let mut store = Store::open(directory.path()).expect("store opens");
        let mut finish = request("protected-export", 'a', 0, 2_000);
        finish.expected_session_revision = Some(0);
        finish.session_record = Some(record("protected-session", 1_000, 2_000, 0, "completed"));
        store
            .commit(&finish)
            .expect("valid application backup created");

        let backup_root = directory.path().join(BACKUP_DIR);
        let backup_path = backup_paths(directory.path()).pop().expect("backup exists");
        let backup_file_name = backup_path.file_name().unwrap();
        let original_backup = fs::read(&backup_path).expect("backup bytes read");
        let mut destinations = vec![
            directory.path().join(DATABASE_FILE),
            directory.path().join(format!("{DATABASE_FILE}-wal")),
            directory.path().join(format!("{DATABASE_FILE}-shm")),
            directory.path().join(format!("{DATABASE_FILE}-journal")),
            backup_path.clone(),
            backup_root.join("nested").join("analysis.json"),
            backup_root
                .join("..")
                .join(BACKUP_DIR)
                .join(backup_file_name),
        ];
        destinations.push(backup_root.clone());

        for destination in destinations {
            assert!(matches!(
                store.export_json(&destination),
                Err(StoreError::InvalidRequest(_))
            ));
        }

        assert!(!backup_root.join("nested").exists());
        assert_eq!(
            fs::read(&backup_path).expect("backup remains readable"),
            original_backup
        );
        assert_backup_contents(&backup_path, 1, 0, 1);
        assert_eq!(store.load_state().unwrap().global_revision, 1);
    }

    #[test]
    fn backup_filename_collision_preserves_existing_valid_backup() {
        let root = tempdir().expect("temporary root");
        let source_dir = root.path().join("source");
        let mut store = Store::open(&source_dir).expect("source store opens");
        let mut finish = request("collision-source", 'b', 0, 3_000);
        finish.expected_session_revision = Some(0);
        finish.session_record = Some(record("collision-session", 2_000, 3_000, 0, "completed"));
        store.commit(&finish).expect("source backup created");
        let source_backup = backup_paths(&source_dir)
            .pop()
            .expect("source backup exists");
        drop(store);

        let collision_dir = root.path().join("collision-backups");
        fs::create_dir_all(&collision_dir).expect("collision directory created");
        let existing = collision_dir.join("backup-1234-1-7.sqlite3");
        fs::copy(&source_backup, &existing).expect("install valid collision fixture");
        let existing_bytes = fs::read(&existing).expect("old backup bytes read");
        let partial = collision_dir.join("pending.sqlite3.partial");
        fs::copy(&source_backup, &partial).expect("install validated backup snapshot");

        let published = publish_backup_no_replace(&partial, &collision_dir, 1234, 1, 7)
            .expect("publisher retries with an unused filename");
        assert_eq!(
            published.file_name().unwrap().to_string_lossy(),
            "backup-1234-1-8.sqlite3"
        );
        assert!(!partial.exists());
        assert_eq!(
            fs::read(&existing).expect("old backup remains readable"),
            existing_bytes
        );
        assert_backup_contents(&existing, 1, 0, 1);
        assert_backup_contents(&published, 1, 0, 1);
    }
}
