use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const JOURNAL: &str = "journal.jsonl";
const LOCK: &str = "journal.lock";
const BACKUP_DIR: &str = "backups";
const AUTO_BACKUP_INTERVAL_MS: i64 = 86_400_000;
static SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Json(serde_json::Error),
    Sqlite(rusqlite::Error),
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
            Self::Io(e) => write!(f, "File operation failed: {e}"),
            Self::Json(e) => write!(f, "JSON operation failed: {e}"),
            Self::Sqlite(e) => write!(f, "Legacy SQLite import failed: {e}"),
            Self::InvalidRequest(x) => write!(f, "Invalid request field: {x}"),
            Self::MissingExpectedRevision(x) => write!(f, "Expected revision is required for {x}"),
            Self::RevisionConflict {
                resource,
                expected,
                actual,
            } => write!(
                f,
                "{resource} revision conflict: expected {expected}, current {actual}"
            ),
            Self::RevisionOverflow(x) => write!(f, "{x} revision overflow"),
            Self::CommandIdConflict => write!(f, "Command ID already has a different payload hash"),
            Self::CorruptDatabase(x) => write!(f, "Journal validation failed: {x}"),
            Self::UnsupportedSchemaVersion(x) => write!(f, "Unsupported legacy schema version {x}"),
            Self::IntegerOutOfRange(x) => write!(f, "Value out of range: {x}"),
        }
    }
}
impl std::error::Error for StoreError {}
impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupStatus {
    NotDue,
    Created,
    Failed { message: String },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredState {
    pub global_revision: u64,
    pub session_revision: u64,
    pub settings_revision: u64,
    pub checkpoint_json: Option<String>,
    pub settings_json: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snapshot {
    state: StoredState,
    sessions: Vec<Value>,
    events: Vec<Value>,
    settings_history: Vec<Value>,
    receipts: HashMap<String, (String, Receipt)>,
    last_backup_utc_ms: Option<i64>,
    last_backup_revision: u64,
    created_utc_ms: i64,
}
#[derive(Serialize, Deserialize)]
struct Frame {
    format_version: u8,
    sequence: u64,
    kind: String,
    request: Option<CommitRequest>,
    receipt: Option<Receipt>,
    seed: Option<Snapshot>,
    backup_utc_ms: Option<i64>,
}
pub struct Store {
    data_dir: PathBuf,
    journal_path: PathBuf,
    lock: File,
    snapshot: Snapshot,
    open_backup_status: BackupStatus,
}
impl Store {
    pub fn open(data_dir: &Path) -> Result<Self, StoreError> {
        Self::open_with_legacy(data_dir, data_dir)
    }

    /// Opens the active JSONL directory and imports a v0.1 SQLite directory once if needed.
    pub fn open_with_legacy(data_dir: &Path, legacy_dir: &Path) -> Result<Self, StoreError> {
        fs::create_dir_all(data_dir)?;
        let lock_path = data_dir.join(LOCK);
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(lock_path)?;
        lock.lock_exclusive()?;
        let journal_path = data_dir.join(JOURNAL);
        if journal_path.exists() && journal_path.metadata()?.len() == 0 {
            fs::remove_file(&journal_path)?;
        }
        if !journal_path.exists() {
            if legacy_dir.join("pomodoro.sqlite3").exists() {
                migrate_legacy(data_dir, legacy_dir, &journal_path)?;
            } else {
                let snap = Snapshot {
                    state: StoredState {
                        global_revision: 0,
                        session_revision: 0,
                        settings_revision: 0,
                        checkpoint_json: None,
                        settings_json: None,
                    },
                    sessions: vec![],
                    events: vec![],
                    settings_history: vec![],
                    receipts: HashMap::new(),
                    last_backup_utc_ms: None,
                    last_backup_revision: 0,
                    created_utc_ms: utc_now_ms(),
                };
                append_frame(
                    &journal_path,
                    &Frame {
                        format_version: 1,
                        sequence: 1,
                        kind: "seed".into(),
                        request: None,
                        receipt: None,
                        seed: Some(snap),
                        backup_utc_ms: None,
                    },
                )?;
            }
        }
        let (snapshot, _) = read_journal(&journal_path, true)?;
        let mut s = Self {
            data_dir: data_dir.to_path_buf(),
            journal_path,
            lock,
            snapshot,
            open_backup_status: BackupStatus::NotDue,
        };
        let now = utc_now_ms();
        if s.snapshot.state.global_revision > 0 {
            s.open_backup_status = s.backup(now);
        }
        FileExt::unlock(&s.lock)?;
        Ok(s)
    }
    pub fn open_backup_status(&self) -> &BackupStatus {
        &self.open_backup_status
    }
    pub fn load_state(&self) -> Result<StoredState, StoreError> {
        self.lock.lock_exclusive()?;
        let state = read_journal(&self.journal_path, true).map(|(snapshot, _)| snapshot.state);
        FileExt::unlock(&self.lock)?;
        state
    }
    pub fn commit(&mut self, r: &CommitRequest) -> Result<CommitOutcome, StoreError> {
        validate(r)?;
        self.lock.lock_exclusive()?;
        let result = (|| {
            let (s, seq) = read_journal(&self.journal_path, true)?;
            if let Some((hash, rec)) = s.receipts.get(&r.command_id) {
                if hash != &r.payload_hash {
                    return Err(StoreError::CommandIdConflict);
                }
                let mut receipt = rec.clone();
                receipt.duplicate = true;
                self.snapshot = s;
                return Ok(CommitOutcome {
                    receipt,
                    backup_status: BackupStatus::NotDue,
                });
            }
            let mut n = s.clone();
            let receipt = apply_commit(&mut n, r)?;
            append_frame(
                &self.journal_path,
                &Frame {
                    format_version: 1,
                    sequence: seq + 1,
                    kind: "commit".into(),
                    request: Some(r.clone()),
                    receipt: Some(receipt.clone()),
                    seed: None,
                    backup_utc_ms: None,
                },
            )?;
            self.snapshot = n;
            let backup_status = if r.session_record.is_some() {
                self.backup(r.committed_utc_ms)
            } else {
                BackupStatus::NotDue
            };
            Ok(CommitOutcome {
                receipt,
                backup_status,
            })
        })();
        FileExt::unlock(&self.lock)?;
        result
    }
    pub fn maybe_auto_backup(&mut self, now: i64) -> BackupStatus {
        if let Err(error) = self.lock.lock_exclusive() {
            return BackupStatus::Failed {
                message: error.to_string(),
            };
        }
        let refreshed = read_journal(&self.journal_path, true);
        if let Ok((snapshot, _)) = refreshed {
            self.snapshot = snapshot;
        } else if let Err(error) = refreshed {
            let _ = FileExt::unlock(&self.lock);
            return BackupStatus::Failed {
                message: error.to_string(),
            };
        }
        if now.saturating_sub(
            self.snapshot
                .last_backup_utc_ms
                .unwrap_or(self.snapshot.created_utc_ms),
        ) >= AUTO_BACKUP_INTERVAL_MS
            && self.snapshot.state.global_revision > self.snapshot.last_backup_revision
        {
            let result = self.backup(now);
            let _ = FileExt::unlock(&self.lock);
            result
        } else {
            let _ = FileExt::unlock(&self.lock);
            BackupStatus::NotDue
        }
    }
    pub fn export_json(&self, destination: &Path) -> Result<(), StoreError> {
        if is_managed_path(destination, &self.data_dir)? {
            return Err(StoreError::InvalidRequest(
                "export destination is application-managed storage",
            ));
        }
        self.lock.lock_exclusive()?;
        let loaded = read_journal(&self.journal_path, true);
        FileExt::unlock(&self.lock)?;
        let (s, _) = loaded?;
        let out = json!({"schema_version":1,"exported_utc_ms":utc_now_ms(),"state":{"global_revision":s.state.global_revision,"session_revision":s.state.session_revision,"settings_revision":s.state.settings_revision,"checkpoint":s.state.checkpoint_json.as_deref().map(serde_json::from_str::<Value>).transpose()?},"settings":s.state.settings_json.as_deref().map(serde_json::from_str::<Value>).transpose()?,"settings_history":s.settings_history,"sessions":s.sessions,"events":s.events});
        write_atomic(destination, &serde_json::to_vec_pretty(&out)?)
    }
    fn backup(&mut self, now: i64) -> BackupStatus {
        let res = (|| -> Result<(), StoreError> {
            let dir = self.data_dir.join(BACKUP_DIR);
            fs::create_dir_all(&dir)?;
            let seq = SEQ.fetch_add(1, Ordering::Relaxed);
            let p = dir.join(format!(
                "backup-{now}-{}-{seq}.jsonl",
                self.snapshot.state.global_revision
            ));
            let (current, sequence) = read_journal(&self.journal_path, true)?;
            let bytes = fs::read(&self.journal_path)?;
            let tmp = p.with_extension("jsonl.partial");
            {
                let mut f = OpenOptions::new().create_new(true).write(true).open(&tmp)?;
                f.write_all(&bytes)?;
                f.sync_all()?
            }
            let _ = read_journal(&tmp, false)?;
            fs::hard_link(&tmp, &p)?;
            fs::remove_file(tmp)?;
            append_frame(
                &self.journal_path,
                &Frame {
                    format_version: 1,
                    sequence: sequence + 1,
                    kind: "backup".into(),
                    request: None,
                    receipt: None,
                    seed: None,
                    backup_utc_ms: Some(now),
                },
            )?;
            self.snapshot = current;
            self.snapshot.last_backup_utc_ms = Some(now);
            Ok(())
        })();
        match res {
            Ok(()) => {
                self.snapshot.last_backup_utc_ms = Some(now);
                self.snapshot.last_backup_revision = self.snapshot.state.global_revision;
                BackupStatus::Created
            }
            Err(e) => BackupStatus::Failed {
                message: e.to_string(),
            },
        }
    }
}

fn validate(r: &CommitRequest) -> Result<(), StoreError> {
    if r.command_id.trim().is_empty() {
        return Err(StoreError::InvalidRequest("command_id"));
    }
    if r.payload_hash.trim().is_empty() {
        return Err(StoreError::InvalidRequest("payload_hash"));
    }
    for x in [&r.checkpoint_json, &r.settings_json] {
        if let Some(v) = x {
            serde_json::from_str::<Value>(v)?;
        }
    }
    Ok(())
}
fn conflict(resource: &'static str, expected: u64, actual: u64) -> StoreError {
    StoreError::RevisionConflict {
        resource,
        expected,
        actual,
    }
}
fn apply_commit(s: &mut Snapshot, r: &CommitRequest) -> Result<Receipt, StoreError> {
    let old = s.state.clone();
    if r.expected_global_revision != old.global_revision {
        return Err(conflict(
            "global",
            r.expected_global_revision,
            old.global_revision,
        ));
    }
    let session_changed = r.checkpoint_json.is_some() || r.session_record.is_some();
    if session_changed
        && r.expected_session_revision
            .ok_or(StoreError::MissingExpectedRevision("session"))?
            != old.session_revision
    {
        return Err(conflict(
            "session",
            r.expected_session_revision.unwrap(),
            old.session_revision,
        ));
    }
    if r.settings_json.is_some()
        && r.expected_settings_revision
            .ok_or(StoreError::MissingExpectedRevision("settings"))?
            != old.settings_revision
    {
        return Err(conflict(
            "settings",
            r.expected_settings_revision.unwrap(),
            old.settings_revision,
        ));
    }
    s.state.global_revision = old
        .global_revision
        .checked_add(1)
        .ok_or(StoreError::RevisionOverflow("global"))?;
    if session_changed {
        s.state.session_revision = old
            .session_revision
            .checked_add(1)
            .ok_or(StoreError::RevisionOverflow("session"))?
    }
    if r.settings_json.is_some() {
        s.state.settings_revision = old
            .settings_revision
            .checked_add(1)
            .ok_or(StoreError::RevisionOverflow("settings"))?
    }
    if let Some(x) = &r.checkpoint_json {
        s.state.checkpoint_json = Some(x.clone())
    }
    if let Some(x) = &r.settings_json {
        s.state.settings_json = Some(x.clone());
        s.settings_history.push(json!({"settings_revision":s.state.settings_revision,"updated_utc_ms":r.committed_utc_ms,"settings":serde_json::from_str::<Value>(x)?}))
    }
    if let Some(x) = &r.session_record {
        s.sessions.push(serde_json::to_value(x)?);
        if r.checkpoint_json.is_none() {
            s.state.checkpoint_json = None
        }
    }
    if let Some(x) = &r.event {
        s.events.push(json!({"event_id":x.event_id,"session_id":x.session_id,"kind":x.kind,"occurred_utc_ms":x.occurred_utc_ms,"local_offset_minutes":x.local_offset_minutes,"payload":serde_json::from_str::<Value>(&x.payload_json)?}))
    }
    let receipt = Receipt {
        command_id: r.command_id.clone(),
        global_revision: s.state.global_revision,
        session_revision: s.state.session_revision,
        settings_revision: s.state.settings_revision,
        committed_utc_ms: r.committed_utc_ms,
        duplicate: false,
    };
    s.receipts.insert(
        r.command_id.clone(),
        (r.payload_hash.clone(), receipt.clone()),
    );
    Ok(receipt)
}
fn utc_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
fn append_frame(path: &Path, frame: &Frame) -> Result<(), StoreError> {
    let mut line = serde_json::to_vec(frame)?;
    line.push(b'\n');
    let mut f = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)?;
    let original_len = f.metadata()?.len();
    f.seek(SeekFrom::End(0))?;
    let result = f
        .write_all(&line)
        .and_then(|()| f.flush())
        .and_then(|()| f.sync_all());
    if let Err(error) = result {
        let rollback = f.set_len(original_len).and_then(|()| f.sync_all());
        return match rollback {
            Ok(()) => Err(StoreError::Io(error)),
            Err(rollback_error) => Err(StoreError::Io(io::Error::new(
                rollback_error.kind(),
                format!("append failed ({error}); rollback failed ({rollback_error})"),
            ))),
        };
    }
    Ok(())
}
fn read_journal(path: &Path, recover_tail: bool) -> Result<(Snapshot, u64), StoreError> {
    let mut bytes = Vec::new();
    File::open(path)?.read_to_end(&mut bytes)?;
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        if !recover_tail {
            return Err(StoreError::CorruptDatabase("incomplete final frame"));
        }
        let end = bytes
            .iter()
            .rposition(|b| *b == b'\n')
            .map(|x| x + 1)
            .unwrap_or(0);
        let f = OpenOptions::new().write(true).open(path)?;
        f.set_len(end as u64)?;
        bytes.truncate(end)
    }
    let mut snap: Option<Snapshot> = None;
    let mut seq = 0;
    for line in bytes.split(|b| *b == b'\n').filter(|x| !x.is_empty()) {
        let fr: Frame = serde_json::from_slice(line)
            .map_err(|_| StoreError::CorruptDatabase("invalid complete frame"))?;
        if fr.format_version != 1 || fr.sequence != seq + 1 {
            return Err(StoreError::CorruptDatabase(
                "unsupported format or sequence gap",
            ));
        }
        match fr.kind.as_str() {
            "seed" => {
                if snap.is_some() || seq != 0 {
                    return Err(StoreError::CorruptDatabase("seed is not first"));
                }
                snap = fr.seed;
            }
            "commit" => {
                let mut state = snap
                    .take()
                    .ok_or(StoreError::CorruptDatabase("commit before seed"))?;
                let req = fr
                    .request
                    .ok_or(StoreError::CorruptDatabase("commit request missing"))?;
                let recorded = fr
                    .receipt
                    .ok_or(StoreError::CorruptDatabase("receipt missing"))?;
                let expected = apply_commit(&mut state, &req)
                    .map_err(|_| StoreError::CorruptDatabase("invalid commit transition"))?;
                if expected != recorded {
                    return Err(StoreError::CorruptDatabase("receipt mismatch"));
                }
                snap = Some(state)
            }
            "migration" => {
                if snap.is_some() || seq != 0 {
                    return Err(StoreError::CorruptDatabase("migration seed is not first"));
                }
                snap = fr.seed;
            }
            "backup" => {
                let mut state = snap
                    .take()
                    .ok_or(StoreError::CorruptDatabase("backup before seed"))?;
                state.last_backup_utc_ms = fr.backup_utc_ms;
                state.last_backup_revision = state.state.global_revision;
                snap = Some(state);
            }
            _ => return Err(StoreError::CorruptDatabase("unknown frame kind")),
        }
        seq += 1
    }
    snap.map(|s| (s, seq))
        .ok_or(StoreError::CorruptDatabase("empty journal"))
}
fn write_atomic(path: &Path, b: &[u8]) -> Result<(), StoreError> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?
    }
    let tmp = path.with_extension("json.partial");
    {
        let mut f = File::create(&tmp)?;
        f.write_all(b)?;
        f.sync_all()?
    }
    fs::rename(tmp, path)?;
    Ok(())
}

fn is_managed_path(destination: &Path, data_dir: &Path) -> Result<bool, StoreError> {
    let root = data_dir.canonicalize()?;
    let absolute = if destination.is_absolute() {
        destination.to_path_buf()
    } else {
        std::env::current_dir()?.join(destination)
    };
    let resolved = if absolute.exists() {
        absolute.canonicalize()?
    } else {
        let parent = absolute
            .parent()
            .ok_or(StoreError::InvalidRequest("export destination"))?;
        let file_name = absolute
            .file_name()
            .ok_or(StoreError::InvalidRequest("export destination"))?;
        parent.canonicalize()?.join(file_name)
    };
    Ok(resolved.starts_with(root))
}

fn migrate_legacy(data_dir: &Path, legacy_dir: &Path, journal: &Path) -> Result<(), StoreError> {
    use rusqlite::{Connection, OptionalExtension};
    let old = legacy_dir.join("pomodoro.sqlite3");
    let c = Connection::open_with_flags(&old, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let integrity: String = c.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if integrity != "ok" {
        return Err(StoreError::CorruptDatabase(
            "legacy SQLite integrity check failed",
        ));
    }
    let ver: i64 = c.query_row(
        "SELECT schema_version FROM schema_meta WHERE singleton=1",
        [],
        |r| r.get(0),
    )?;
    if ver != 1 {
        return Err(StoreError::UnsupportedSchemaVersion(ver));
    }
    let (g,sr,tr):(i64,i64,i64)=c.query_row("SELECT global_revision,session_revision,settings_revision FROM schema_meta WHERE singleton=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    let cp = c
        .query_row(
            "SELECT checkpoint_json FROM active_session WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    let settings = c
        .query_row(
            "SELECT settings_json FROM settings WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    let sessions = read_legacy_rows(
        &c,
        "SELECT session_id, phase, target_ms, actual_active_ms, started_utc_ms, ended_utc_ms, start_local_date, start_offset_minutes, outcome, reason FROM session_history ORDER BY rowid",
        &[
            "session_id",
            "phase",
            "target_ms",
            "actual_active_ms",
            "started_utc_ms",
            "ended_utc_ms",
            "start_local_date",
            "start_offset_minutes",
            "outcome",
            "reason",
        ],
    )?;
    let events = read_legacy_rows(
        &c,
        "SELECT event_id, session_id, kind, occurred_utc_ms, local_offset_minutes, payload_json FROM event_log ORDER BY rowid",
        &[
            "event_id",
            "session_id",
            "kind",
            "occurred_utc_ms",
            "local_offset_minutes",
            "payload",
        ],
    )?;
    let settings_history = read_legacy_rows(
        &c,
        "SELECT settings_revision, settings_json, updated_utc_ms FROM settings_history ORDER BY settings_revision",
        &["settings_revision", "settings_json", "updated_utc_ms"],
    )?;
    let mut receipts = HashMap::new();
    {
        let mut q =
            c.prepare("SELECT command_id,payload_hash,result_payload FROM command_receipts")?;
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (id, hash, payload) = row?;
            let receipt: Receipt = serde_json::from_str(&payload)?;
            receipts.insert(id, (hash, receipt));
        }
    }
    let b = c
        .query_row(
            "SELECT last_backup_utc_ms FROM backup_state WHERE singleton=1",
            [],
            |r| r.get::<_, Option<i64>>(0),
        )
        .optional()?
        .flatten();
    let backup_revision = c
        .query_row(
            "SELECT last_backup_revision FROM backup_state WHERE singleton=1",
            [],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0)
        .max(0) as u64;
    let snap = Snapshot {
        state: StoredState {
            global_revision: g as u64,
            session_revision: sr as u64,
            settings_revision: tr as u64,
            checkpoint_json: cp,
            settings_json: settings,
        },
        sessions,
        events,
        settings_history,
        receipts,
        last_backup_utc_ms: b,
        last_backup_revision: backup_revision,
        created_utc_ms: utc_now_ms(),
    };
    drop(c);
    let partial = data_dir.join("journal.migration.partial");
    let _ = fs::remove_file(&partial);
    append_frame(
        &partial,
        &Frame {
            format_version: 1,
            sequence: 1,
            kind: "migration".into(),
            request: None,
            receipt: None,
            seed: Some(snap),
            backup_utc_ms: None,
        },
    )?;
    let _ = read_journal(&partial, false)?;
    let dir = data_dir.join("legacy-import");
    fs::create_dir_all(&dir)?;
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let src = PathBuf::from(format!("{}{}", old.display(), suffix));
        if src.exists() {
            let dst = dir.join(src.file_name().unwrap());
            fs::copy(&src, &dst)?;
            if fs::read(&src)? != fs::read(&dst)? {
                return Err(StoreError::CorruptDatabase(
                    "legacy archive verification failed",
                ));
            }
        }
    }
    let sqlite_backups = legacy_dir.join(BACKUP_DIR);
    if sqlite_backups.exists() {
        let archived_backups = dir.join("sqlite-backups");
        copy_tree_verified(&sqlite_backups, &archived_backups)?;
    }
    fs::rename(&partial, journal)?;
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let src = PathBuf::from(format!("{}{}", old.display(), suffix));
        if src.exists() {
            if legacy_dir == data_dir {
                fs::remove_file(src)?;
            }
        }
    }
    if legacy_dir != data_dir {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let src = PathBuf::from(format!("{}{}", old.display(), suffix));
            if src.exists() {
                fs::remove_file(src)?;
            }
        }
    }
    if sqlite_backups.exists() {
        fs::remove_dir_all(sqlite_backups)?;
    }
    Ok(())
}

fn copy_tree_verified(source: &Path, destination: &Path) -> Result<(), StoreError> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let src = entry.path();
        let dst = destination.join(entry.file_name());
        if src.is_dir() {
            copy_tree_verified(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)?;
            if fs::read(&src)? != fs::read(&dst)? {
                return Err(StoreError::CorruptDatabase(
                    "legacy backup verification failed",
                ));
            }
        }
    }
    Ok(())
}
fn read_legacy_rows(
    c: &rusqlite::Connection,
    sql: &str,
    keys: &[&str],
) -> Result<Vec<Value>, StoreError> {
    let mut q = c.prepare(sql)?;
    let rows = q.query_map([], |row| {
        let mut values = serde_json::Map::new();
        for (i, key) in keys.iter().enumerate() {
            let v: rusqlite::types::Value = row.get(i)?;
            let mut value = match v {
                rusqlite::types::Value::Null => Value::Null,
                rusqlite::types::Value::Integer(x) => json!(x),
                rusqlite::types::Value::Real(x) => json!(x),
                rusqlite::types::Value::Text(x) => json!(x),
                rusqlite::types::Value::Blob(x) => json!(x),
            };
            if matches!(*key, "settings_json" | "payload") {
                if let Value::String(raw) = &value {
                    if let Ok(parsed) = serde_json::from_str(raw) {
                        value = parsed;
                    }
                }
            }
            values.insert((*key).to_owned(), value);
        }
        Ok(Value::Object(values))
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    fn req(id: &str, expected: u64) -> CommitRequest {
        CommitRequest {
            command_id: id.into(),
            payload_hash: "hash".into(),
            expected_global_revision: expected,
            expected_session_revision: Some(0),
            expected_settings_revision: None,
            checkpoint_json: Some(r#"{"phase":"focus"}"#.into()),
            settings_json: None,
            session_record: None,
            event: None,
            committed_utc_ms: 1,
        }
    }
    #[test]
    fn append_idempotent_cas_and_restart() {
        let d = tempdir().unwrap();
        let mut s = Store::open(d.path()).unwrap();
        let r = s.commit(&req("a", 0)).unwrap().receipt;
        assert_eq!(r.global_revision, 1);
        assert!(s.commit(&req("a", 0)).unwrap().receipt.duplicate);
        assert!(matches!(
            s.commit(&req("b", 0)),
            Err(StoreError::RevisionConflict { .. })
        ));
        drop(s);
        let s = Store::open(d.path()).unwrap();
        assert_eq!(
            s.load_state().unwrap().checkpoint_json.as_deref(),
            Some(r#"{"phase":"focus"}"#)
        );
        assert!(
            fs::read_to_string(d.path().join(JOURNAL))
                .unwrap()
                .lines()
                .count()
                >= 2
        );
    }
    #[test]
    fn incomplete_tail_is_discarded() {
        let d = tempdir().unwrap();
        let s = Store::open(d.path()).unwrap();
        drop(s);
        let p = d.path().join(JOURNAL);
        let mut f = OpenOptions::new().append(true).open(&p).unwrap();
        f.write_all(b"{partial").unwrap();
        drop(f);
        let s = Store::open(d.path()).unwrap();
        assert_eq!(s.load_state().unwrap().global_revision, 0);
        assert!(fs::read(&p).unwrap().ends_with(b"\n"));
    }

    #[test]
    fn separate_handles_reload_and_serialize_writes() {
        let dir = tempdir().unwrap();
        let mut first = Store::open(dir.path()).unwrap();
        let mut second = Store::open(dir.path()).unwrap();
        first.commit(&req("first", 0)).unwrap();
        assert_eq!(second.load_state().unwrap().global_revision, 1);
        assert!(matches!(
            second.commit(&req("stale", 0)),
            Err(StoreError::RevisionConflict { actual: 1, .. })
        ));
    }
    #[test]
    fn export_has_history_and_records() {
        let d = tempdir().unwrap();
        let app = d.path().join("app");
        fs::create_dir_all(&app).unwrap();
        let mut s = Store::open(&app).unwrap();
        let mut r = req("a", 0);
        r.checkpoint_json = None;
        r.expected_session_revision = None;
        r.expected_settings_revision = Some(0);
        r.settings_json = Some(r#"{"focus_ms":1}"#.into());
        r.event = Some(EventRecord {
            event_id: "e".into(),
            session_id: None,
            kind: "test".into(),
            occurred_utc_ms: 2,
            local_offset_minutes: 0,
            payload_json: "{}".into(),
        });
        s.commit(&r).unwrap();
        let out = d.path().join("analysis.json");
        s.export_json(&out).unwrap();
        let v: Value = serde_json::from_slice(&fs::read(&out).unwrap()).unwrap();
        assert_eq!(v["settings_history"].as_array().unwrap().len(), 1);
        assert_eq!(v["events"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn migrates_legacy_sqlite_history_receipts_and_archives_after_validation() {
        let dir = tempdir().unwrap();
        let data_dir = dir.path().join("data");
        fs::create_dir(&data_dir).unwrap();
        let legacy_dir = dir.path().join("old-data");
        fs::create_dir(&legacy_dir).unwrap();
        let db = legacy_dir.join("pomodoro.sqlite3");
        let c = rusqlite::Connection::open(&db).unwrap();
        c.execute_batch("CREATE TABLE schema_meta(singleton INTEGER PRIMARY KEY, schema_version INTEGER, global_revision INTEGER, session_revision INTEGER, settings_revision INTEGER, created_utc_ms INTEGER);
            CREATE TABLE active_session(singleton INTEGER PRIMARY KEY, checkpoint_json TEXT, session_revision INTEGER, updated_utc_ms INTEGER);
            CREATE TABLE settings(singleton INTEGER PRIMARY KEY, settings_json TEXT, settings_revision INTEGER, updated_utc_ms INTEGER);
            CREATE TABLE settings_history(settings_revision INTEGER PRIMARY KEY, settings_json TEXT, updated_utc_ms INTEGER);
            CREATE TABLE session_history(session_id TEXT PRIMARY KEY, phase TEXT, target_ms INTEGER, actual_active_ms INTEGER, started_utc_ms INTEGER, ended_utc_ms INTEGER, start_local_date TEXT, start_offset_minutes INTEGER, outcome TEXT, reason TEXT);
            CREATE TABLE event_log(event_id TEXT PRIMARY KEY, session_id TEXT, kind TEXT, occurred_utc_ms INTEGER, local_offset_minutes INTEGER, payload_json TEXT);
            CREATE TABLE command_receipts(command_id TEXT PRIMARY KEY, payload_hash TEXT, result_payload TEXT, global_revision INTEGER, session_revision INTEGER, settings_revision INTEGER, committed_utc_ms INTEGER);
            CREATE TABLE backup_state(singleton INTEGER PRIMARY KEY, last_backup_utc_ms INTEGER, last_backup_revision INTEGER);
            INSERT INTO schema_meta VALUES(1,1,3,1,2,10);
            INSERT INTO active_session VALUES(1,'{\"phase\":\"focus\"}',1,11);
            INSERT INTO settings VALUES(1,'{\"focus_ms\":200}',2,12);
            INSERT INTO settings_history VALUES(1,'{\"focus_ms\":100}',10);
            INSERT INTO settings_history VALUES(2,'{\"focus_ms\":200}',12);
            INSERT INTO session_history VALUES('s1','focus',1000,900,1,2,'2026-01-01',345,'completed',NULL);
            INSERT INTO event_log VALUES('e1','s1','completed',2,345,'{\"ok\":true}');
            INSERT INTO command_receipts VALUES('cmd','hash','{\"command_id\":\"cmd\",\"global_revision\":3,\"session_revision\":1,\"settings_revision\":2,\"committed_utc_ms\":12,\"duplicate\":false}',3,1,2,12);
            INSERT INTO backup_state VALUES(1,10,1);").unwrap();
        drop(c);
        fs::create_dir(legacy_dir.join("backups")).unwrap();
        fs::write(
            legacy_dir.join("backups/backup-1.sqlite3"),
            b"legacy backup",
        )
        .unwrap();

        let store = Store::open_with_legacy(&data_dir, &legacy_dir).unwrap();
        let state = store.load_state().unwrap();
        assert_eq!(state.global_revision, 3);
        assert_eq!(
            state.checkpoint_json.as_deref(),
            Some(r#"{"phase":"focus"}"#)
        );
        let export = dir.path().join("analysis.json");
        store.export_json(&export).unwrap();
        let data: Value = serde_json::from_slice(&fs::read(export).unwrap()).unwrap();
        assert_eq!(data["sessions"][0]["session_id"], "s1");
        assert_eq!(data["events"][0]["event_id"], "e1");
        assert_eq!(data["settings_history"].as_array().unwrap().len(), 2);
        assert!(data_dir.join("legacy-import/pomodoro.sqlite3").exists());
        assert!(
            data_dir
                .join("legacy-import/sqlite-backups/backup-1.sqlite3")
                .exists()
        );
        assert!(!db.exists());
        drop(store);
        let mut reopened = Store::open(&data_dir).unwrap();
        let mut duplicate = req("cmd", 3);
        duplicate.payload_hash = "hash".into();
        let result = reopened.commit(&duplicate).unwrap();
        assert!(result.receipt.duplicate);
        assert_eq!(result.receipt.global_revision, 3);
    }
}
