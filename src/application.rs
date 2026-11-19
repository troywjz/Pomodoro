use std::fmt;
#[cfg(test)]
use std::fs;
use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use pomodoro_storage::{
    BackupStatus, CommitRequest, EventRecord as StoredEvent, SessionRecord as StoredSession, Store,
    StoreError,
};

use crate::domain::{
    ClockSample, DomainEvent, RecoveryChoice, RecoveryReason, SessionRecord, TickResult, TimerCore,
    TimerError, TimerSettings, TimerStatus,
};

const CHECKPOINT_INTERVAL_MS: u64 = 10_000;
const PERSIST_RETRY_INTERVAL_MS: u64 = 5_000;

pub struct Application {
    pub timer: TimerCore,
    store: Store,
    global_revision: u64,
    session_revision: u64,
    settings_revision: u64,
    last_checkpoint_ms: u64,
    last_checkpoint_attempt_ms: u64,
    runtime_dirty: bool,
    last_runtime_message: Option<String>,
    last_backup_status: BackupStatus,
    last_save_error: Option<String>,
    pending: Option<PendingCommit>,
}

struct PendingCommit {
    request: CommitRequest,
    candidate: TimerCore,
    apply_timer: bool,
    checkpoint_written: bool,
    committed_monotonic_ms: u64,
}

#[derive(Debug)]
pub enum ApplicationError {
    Timer(TimerError),
    Storage(String),
    Serialization(String),
    PendingSave,
    RecoveryDecisionRequired,
    ReceiptMismatch,
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timer(error) => write!(formatter, "{error}"),
            Self::Storage(error) => write!(formatter, "本地保存失败：{error}"),
            Self::Serialization(error) => write!(formatter, "本地数据编码失败：{error}"),
            Self::PendingSave => write!(formatter, "上一项保存正在重试，请稍候"),
            Self::RecoveryDecisionRequired => {
                write!(formatter, "系统计时已暂停，请先选择恢复方式")
            }
            Self::ReceiptMismatch => write!(formatter, "保存回执与当前版本不匹配"),
        }
    }
}

impl std::error::Error for ApplicationError {}

impl From<TimerError> for ApplicationError {
    fn from(value: TimerError) -> Self {
        Self::Timer(value)
    }
}

impl From<StoreError> for ApplicationError {
    fn from(value: StoreError) -> Self {
        Self::Storage(value.to_string())
    }
}

impl From<serde_json::Error> for ApplicationError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value.to_string())
    }
}

impl Application {
    pub fn open(data_dir: &Path, now: ClockSample) -> Result<Self, ApplicationError> {
        let store = Store::open(data_dir)?;
        let backup_status = store.open_backup_status().clone();
        let stored = store.load_state()?;
        let settings = stored
            .settings_json
            .as_deref()
            .map(serde_json::from_str::<TimerSettings>)
            .transpose()?
            .unwrap_or_default();
        settings.validate()?;

        let mut timer = if let Some(checkpoint_json) = stored.checkpoint_json.as_deref() {
            serde_json::from_str::<TimerCore>(checkpoint_json)?
        } else {
            TimerCore::new(settings.clone())?
        };
        timer.settings = settings.clone();
        timer.restore_for_recovery(now.clone());

        let mut app = Self {
            timer,
            store,
            global_revision: stored.global_revision,
            session_revision: stored.session_revision,
            settings_revision: stored.settings_revision,
            last_checkpoint_ms: now.monotonic_ms,
            last_checkpoint_attempt_ms: now.monotonic_ms,
            runtime_dirty: false,
            last_runtime_message: None,
            last_backup_status: backup_status,
            last_save_error: None,
            pending: None,
        };

        if stored.settings_json.is_none() {
            let event = DomainEvent::SettingsChanged(settings.clone());
            app.persist_candidate(
                app.timer.clone(),
                Some(event),
                None,
                Some(settings),
                now,
                true,
            )?;
        }
        Ok(app)
    }

    pub fn start(
        &mut self,
        session_id: String,
        now: ClockSample,
    ) -> Result<DomainEvent, ApplicationError> {
        self.ensure_no_pending()?;
        let mut candidate = self.timer.clone();
        let event = candidate.start(session_id, now.clone())?;
        self.persist_candidate(candidate, Some(event.clone()), None, None, now, true)?;
        Ok(event)
    }

    pub fn tick(&mut self, now: ClockSample) -> TickResult {
        if self.pending.is_some()
            && now
                .monotonic_ms
                .saturating_sub(self.last_checkpoint_attempt_ms)
                >= PERSIST_RETRY_INTERVAL_MS
        {
            self.last_checkpoint_attempt_ms = now.monotonic_ms;
            let _ = self.retry_pending(now.clone());
        }
        if self.pending.is_some() {
            return TickResult::default();
        }

        let mut candidate = self.timer.clone();
        let result = candidate.tick(now.clone());
        if result.recovery_required {
            let session_id = candidate
                .session
                .as_ref()
                .map(|session| session.session_id.clone())
                .unwrap_or_default();
            let reason = result
                .recovery_reason
                .unwrap_or(RecoveryReason::MonotonicClockReset);
            let event = DomainEvent::RecoveryRequired { session_id, reason };
            self.last_runtime_message = Some(recovery_message(reason).to_owned());
            let _ = self.persist_candidate(candidate, Some(event), None, None, now, true);
            return result;
        }
        if result.elapsed_ms == 0 {
            return result;
        }

        if let Some(record) = result.completed.clone() {
            let event = DomainEvent::Completed(record.clone());
            let _ = self.persist_candidate(candidate, Some(event), Some(record), None, now, true);
            return result;
        }

        self.timer = candidate.clone();
        self.runtime_dirty = true;
        let checkpoint_due =
            now.monotonic_ms.saturating_sub(self.last_checkpoint_ms) >= CHECKPOINT_INTERVAL_MS;
        let retry_ready = now
            .monotonic_ms
            .saturating_sub(self.last_checkpoint_attempt_ms)
            >= PERSIST_RETRY_INTERVAL_MS;
        if self.pending.is_none() && checkpoint_due && retry_ready {
            self.last_checkpoint_attempt_ms = now.monotonic_ms;
            let _ = self.persist_candidate(candidate, None, None, None, now, false);
        }
        result
    }

    pub fn pause(&mut self, now: ClockSample) -> Result<DomainEvent, ApplicationError> {
        self.ensure_no_pending()?;
        let mut candidate = self.timer.clone();
        let tick = candidate.tick(now.clone());
        if tick.recovery_required {
            let reason = tick
                .recovery_reason
                .unwrap_or(RecoveryReason::MonotonicClockReset);
            let event = recovery_event(&candidate, Some(reason));
            self.last_runtime_message = Some(recovery_message(reason).to_owned());
            self.persist_candidate(candidate, Some(event), None, None, now, true)?;
            return Err(ApplicationError::RecoveryDecisionRequired);
        }
        if let Some(record) = tick.completed {
            let event = DomainEvent::Completed(record.clone());
            self.persist_candidate(
                candidate,
                Some(event.clone()),
                Some(record),
                None,
                now,
                true,
            )?;
            return Ok(event);
        }
        let event = candidate.pause(now.clone())?;
        self.persist_candidate(candidate, Some(event.clone()), None, None, now, true)?;
        Ok(event)
    }

    pub fn resume(&mut self, now: ClockSample) -> Result<DomainEvent, ApplicationError> {
        self.ensure_no_pending()?;
        let mut candidate = self.timer.clone();
        let event = candidate.resume(now.clone())?;
        self.persist_candidate(candidate, Some(event.clone()), None, None, now, true)?;
        Ok(event)
    }

    pub fn end_early(&mut self, now: ClockSample) -> Result<DomainEvent, ApplicationError> {
        self.ensure_no_pending()?;
        let mut candidate = self.timer.clone();
        let tick = candidate.tick(now.clone());
        if tick.recovery_required {
            let reason = tick
                .recovery_reason
                .unwrap_or(RecoveryReason::MonotonicClockReset);
            let event = recovery_event(&candidate, Some(reason));
            self.last_runtime_message = Some(recovery_message(reason).to_owned());
            self.persist_candidate(candidate, Some(event), None, None, now, true)?;
            return Err(ApplicationError::RecoveryDecisionRequired);
        }
        let event = candidate.end_early(now.clone())?;
        let terminal = terminal_record(&event);
        self.persist_candidate(candidate, Some(event.clone()), terminal, None, now, true)?;
        Ok(event)
    }

    pub fn advance(&mut self, now: ClockSample) -> Result<DomainEvent, ApplicationError> {
        self.ensure_no_pending()?;
        let mut candidate = self.timer.clone();
        let event = candidate.advance()?;
        self.persist_candidate(candidate, Some(event.clone()), None, None, now, true)?;
        Ok(event)
    }

    pub fn resolve_recovery(
        &mut self,
        choice: RecoveryChoice,
        now: ClockSample,
    ) -> Result<Option<DomainEvent>, ApplicationError> {
        self.ensure_no_pending()?;
        let mut candidate = self.timer.clone();
        let event = candidate.resolve_recovery(choice, now.clone())?;
        if let Some(event) = event.clone() {
            let terminal = terminal_record(&event);
            self.persist_candidate(candidate, Some(event), terminal, None, now, true)?;
            self.last_runtime_message = None;
        }
        Ok(event)
    }

    pub fn update_settings(
        &mut self,
        settings: TimerSettings,
        now: ClockSample,
    ) -> Result<(), ApplicationError> {
        self.ensure_no_pending()?;
        let mut candidate = self.timer.clone();
        candidate.settings = settings.clone();
        let event = DomainEvent::SettingsChanged(settings.clone());
        self.persist_candidate(candidate, Some(event), None, Some(settings), now, true)
    }

    pub fn maybe_auto_backup(&mut self, now_utc_ms: i64) -> BackupStatus {
        let status = self.store.maybe_auto_backup(now_utc_ms);
        self.observe_backup_status(status.clone());
        status
    }

    pub fn export_analysis_json(&self, destination: &Path) -> Result<(), ApplicationError> {
        self.store.export_json(destination)?;
        Ok(())
    }

    pub fn save_label(&self) -> &'static str {
        if self.pending.is_some() {
            "保存重试中"
        } else if self.runtime_dirty {
            "未保存"
        } else {
            "已保存"
        }
    }

    pub fn feedback(&self) -> String {
        let mut messages = Vec::new();
        if let Some(message) = &self.last_runtime_message {
            messages.push(message.clone());
        } else if self.timer.status() == TimerStatus::Recovery {
            messages.push("上次退出时存在未完成计时，请选择恢复方式".to_owned());
        }
        if let Some(error) = &self.last_save_error {
            messages.push(error.clone());
        }
        if let BackupStatus::Failed { message } = &self.last_backup_status {
            messages.push(format!("备份失败，数据已保存，应用会自动重试：{message}"));
        }
        messages.join("；")
    }

    fn ensure_no_pending(&mut self) -> Result<(), ApplicationError> {
        if self.pending.is_some() {
            return Err(ApplicationError::PendingSave);
        }
        Ok(())
    }

    fn persist_candidate(
        &mut self,
        candidate: TimerCore,
        event: Option<DomainEvent>,
        terminal: Option<SessionRecord>,
        settings: Option<TimerSettings>,
        now: ClockSample,
        apply_timer: bool,
    ) -> Result<(), ApplicationError> {
        let checkpoint_json = Some(serde_json::to_string(&candidate)?);
        let settings_json = settings.as_ref().map(serde_json::to_string).transpose()?;
        let session_changed = checkpoint_json.is_some() || terminal.is_some();
        let event_record = event
            .as_ref()
            .map(|event| make_stored_event(event, &candidate, &now))
            .transpose()?;
        let stored_record = terminal.as_ref().map(to_stored_session);
        let command_id = Uuid::new_v4().to_string();
        let payload_hash = payload_hash(
            self.global_revision,
            session_changed.then_some(self.session_revision),
            settings_json.as_deref().map(|_| self.settings_revision),
            checkpoint_json.as_deref(),
            settings_json.as_deref(),
            terminal.as_ref(),
            event_record.as_ref(),
            now.utc_ms,
        )?;
        let request = CommitRequest {
            command_id,
            payload_hash,
            expected_global_revision: self.global_revision,
            expected_session_revision: session_changed.then_some(self.session_revision),
            expected_settings_revision: settings_json.as_ref().map(|_| self.settings_revision),
            checkpoint_json,
            settings_json,
            session_record: stored_record,
            event: event_record,
            committed_utc_ms: now.utc_ms,
        };
        let pending = PendingCommit {
            request,
            candidate: candidate.clone(),
            apply_timer,
            checkpoint_written: true,
            committed_monotonic_ms: now.monotonic_ms,
        };

        match self.store.commit(&pending.request) {
            Ok(outcome) => self.complete_pending(pending, outcome),
            Err(error) => {
                if terminal.is_some() || candidate.status() == TimerStatus::Recovery {
                    self.timer = candidate;
                }
                self.runtime_dirty = true;
                self.last_save_error = Some(format!("本地保存失败，应用会自动重试：{error}"));
                self.last_checkpoint_attempt_ms = now.monotonic_ms;
                self.pending = Some(pending);
                Err(ApplicationError::Storage(error.to_string()))
            }
        }
    }

    fn retry_pending(&mut self, now: ClockSample) -> Result<(), ApplicationError> {
        let Some(pending) = self.pending.take() else {
            return Ok(());
        };
        match self.store.commit(&pending.request) {
            Ok(outcome) => self.complete_pending(pending, outcome),
            Err(error) => {
                self.last_save_error = Some(format!("本地保存失败，应用会自动重试：{error}"));
                self.last_checkpoint_attempt_ms = now.monotonic_ms;
                self.pending = Some(pending);
                Err(ApplicationError::Storage(error.to_string()))
            }
        }
    }

    fn complete_pending(
        &mut self,
        pending: PendingCommit,
        outcome: pomodoro_storage::CommitOutcome,
    ) -> Result<(), ApplicationError> {
        let receipt = &outcome.receipt;
        let expected_global = pending
            .request
            .expected_global_revision
            .checked_add(1)
            .ok_or(ApplicationError::ReceiptMismatch)?;
        let expected_session = pending
            .request
            .expected_session_revision
            .map(|revision| {
                revision
                    .checked_add(1)
                    .ok_or(ApplicationError::ReceiptMismatch)
            })
            .transpose()?
            .unwrap_or(self.session_revision);
        let expected_settings = pending
            .request
            .expected_settings_revision
            .map(|revision| {
                revision
                    .checked_add(1)
                    .ok_or(ApplicationError::ReceiptMismatch)
            })
            .transpose()?
            .unwrap_or(self.settings_revision);
        if receipt.command_id != pending.request.command_id
            || receipt.global_revision != expected_global
            || receipt.session_revision != expected_session
            || receipt.settings_revision != expected_settings
        {
            self.last_save_error = Some(ApplicationError::ReceiptMismatch.to_string());
            self.pending = Some(pending);
            return Err(ApplicationError::ReceiptMismatch);
        }

        self.global_revision = receipt.global_revision;
        self.session_revision = receipt.session_revision;
        self.settings_revision = receipt.settings_revision;
        if pending.apply_timer {
            self.timer = pending.candidate.clone();
        }
        if pending.checkpoint_written {
            self.last_checkpoint_ms = pending.committed_monotonic_ms;
            self.runtime_dirty = self.timer.runtime_seq > pending.candidate.runtime_seq;
        }
        self.last_save_error = None;
        self.observe_backup_status(outcome.backup_status);
        Ok(())
    }

    fn observe_backup_status(&mut self, status: BackupStatus) {
        match status {
            BackupStatus::NotDue => {}
            BackupStatus::Created => self.last_backup_status = BackupStatus::Created,
            BackupStatus::Failed { message } => {
                self.last_backup_status = BackupStatus::Failed { message };
            }
        }
    }
}

fn terminal_record(event: &DomainEvent) -> Option<SessionRecord> {
    match event {
        DomainEvent::Completed(record) | DomainEvent::Ended(record) => Some(record.clone()),
        _ => None,
    }
}

fn recovery_event(timer: &TimerCore, reason: Option<RecoveryReason>) -> DomainEvent {
    DomainEvent::RecoveryRequired {
        session_id: timer
            .session
            .as_ref()
            .map(|session| session.session_id.clone())
            .unwrap_or_default(),
        reason: reason.unwrap_or(RecoveryReason::MonotonicClockReset),
    }
}

fn recovery_message(reason: RecoveryReason) -> &'static str {
    match reason {
        RecoveryReason::SystemSleep => "检测到系统睡眠，计时已暂停，请选择恢复方式",
        RecoveryReason::MonotonicClockReset => "检测到计时源回退，计时已暂停，请选择恢复方式",
    }
}

fn to_stored_session(record: &SessionRecord) -> StoredSession {
    StoredSession {
        session_id: record.session_id.clone(),
        phase: record.phase.clone(),
        target_ms: record.target_ms,
        actual_active_ms: record.actual_active_ms,
        started_utc_ms: record.started_utc_ms,
        ended_utc_ms: record.ended_utc_ms,
        start_local_date: record.start_local_date.clone(),
        start_offset_minutes: record.start_offset_minutes,
        outcome: record.outcome.clone(),
        reason: record.reason.clone(),
    }
}

fn make_stored_event(
    event: &DomainEvent,
    candidate: &TimerCore,
    now: &ClockSample,
) -> Result<StoredEvent, ApplicationError> {
    let kind = match event {
        DomainEvent::Started { .. } => "started",
        DomainEvent::Paused { .. } => "paused",
        DomainEvent::Resumed { .. } => "resumed",
        DomainEvent::Completed(_) => "completed",
        DomainEvent::Ended(_) => "ended",
        DomainEvent::Advanced { .. } => "advanced",
        DomainEvent::RecoveryContinued { .. } => "recovery_continued",
        DomainEvent::RecoveryKeptPaused { .. } => "recovery_kept_paused",
        DomainEvent::RecoveryRequired { .. } => "recovery_required",
        DomainEvent::SettingsChanged(_) => "settings_changed",
    };
    let session_id = match event {
        DomainEvent::Started { session_id, .. }
        | DomainEvent::Paused { session_id, .. }
        | DomainEvent::Resumed { session_id }
        | DomainEvent::RecoveryContinued { session_id }
        | DomainEvent::RecoveryKeptPaused { session_id }
        | DomainEvent::RecoveryRequired { session_id, .. } => Some(session_id.clone()),
        DomainEvent::Completed(record) | DomainEvent::Ended(record) => {
            Some(record.session_id.clone())
        }
        DomainEvent::Advanced { .. } | DomainEvent::SettingsChanged(_) => None,
    };
    let payload = serde_json::json!({
        "event": event,
        "phase": candidate.phase,
        "round": candidate.round,
        "status": candidate.status(),
        "remaining_ms": candidate.session.as_ref().map(|session| session.remaining_ms),
        "target_ms": candidate.session.as_ref().map(|session| session.target_ms),
        "settings": candidate.settings,
    });
    Ok(StoredEvent {
        event_id: Uuid::new_v4().to_string(),
        session_id,
        kind: kind.to_owned(),
        occurred_utc_ms: now.utc_ms,
        local_offset_minutes: now.local_offset_minutes,
        payload_json: serde_json::to_string(&payload)?,
    })
}

#[derive(Serialize)]
struct HashMaterial<'a> {
    expected_global_revision: u64,
    expected_session_revision: Option<u64>,
    expected_settings_revision: Option<u64>,
    checkpoint_json: Option<&'a str>,
    settings_json: Option<&'a str>,
    session_record: Option<&'a SessionRecord>,
    event: Option<HashedEvent<'a>>,
    committed_utc_ms: i64,
}

#[derive(Serialize)]
struct HashedEvent<'a> {
    event_id: &'a str,
    session_id: Option<&'a str>,
    kind: &'a str,
    occurred_utc_ms: i64,
    local_offset_minutes: i32,
    payload_json: &'a str,
}

fn payload_hash(
    expected_global_revision: u64,
    expected_session_revision: Option<u64>,
    expected_settings_revision: Option<u64>,
    checkpoint_json: Option<&str>,
    settings_json: Option<&str>,
    session_record: Option<&SessionRecord>,
    event: Option<&StoredEvent>,
    committed_utc_ms: i64,
) -> Result<String, ApplicationError> {
    let hashed_event = event.map(|event| HashedEvent {
        event_id: &event.event_id,
        session_id: event.session_id.as_deref(),
        kind: &event.kind,
        occurred_utc_ms: event.occurred_utc_ms,
        local_offset_minutes: event.local_offset_minutes,
        payload_json: &event.payload_json,
    });
    let material = HashMaterial {
        expected_global_revision,
        expected_session_revision,
        expected_settings_revision,
        checkpoint_json,
        settings_json,
        session_record,
        event: hashed_event,
        committed_utc_ms,
    };
    let digest = Sha256::digest(serde_json::to_vec(&material)?);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::domain::{Phase, RecoveryReason, SessionStatus, TimerStatus};

    fn sample(monotonic_ms: u64, utc_ms: i64) -> ClockSample {
        ClockSample {
            monotonic_ms,
            boot_elapsed_ms: monotonic_ms,
            utc_ms,
            local_date: "2026-11-19".into(),
            local_offset_minutes: -360,
        }
    }

    fn temporary_app() -> (tempfile::TempDir, Application) {
        let directory = tempfile::tempdir().unwrap();
        let app = Application::open(directory.path(), sample(0, 1_000)).unwrap();
        (directory, app)
    }

    #[test]
    fn defaults_and_settings_are_saved_with_a_receipt() {
        let (directory, app) = temporary_app();
        assert_eq!(app.timer.settings.focus_minutes, 25);
        assert_eq!(app.timer.settings.short_break_minutes, 5);
        assert_eq!(app.timer.settings.long_break_minutes, 15);
        assert_eq!(app.timer.settings.rounds_until_long_break, 4);
        assert_eq!(app.global_revision, 1);
        assert_eq!(app.settings_revision, 1);
        assert_eq!(app.save_label(), "已保存");
        assert!(!directory.path().join("backups").exists());
    }

    #[test]
    fn pause_resume_and_restart_preserve_checkpoint_without_offline_time() {
        let directory = tempfile::tempdir().unwrap();
        let mut app = Application::open(directory.path(), sample(0, 1_000)).unwrap();
        let short_settings = TimerSettings {
            focus_minutes: 1,
            ..TimerSettings::default()
        };
        app.update_settings(short_settings, sample(0, 1_000))
            .unwrap();
        app.start("session-1".into(), sample(0, 1_000)).unwrap();
        app.tick(sample(20_000, 21_000));
        app.pause(sample(30_000, 31_000)).unwrap();
        assert_eq!(app.timer.session.as_ref().unwrap().remaining_ms, 30_000);

        drop(app);
        let mut recovered =
            Application::open(directory.path(), sample(9_000_000, 99_000_000)).unwrap();
        assert_eq!(recovered.timer.status(), TimerStatus::Recovery);
        assert_eq!(
            recovered.timer.session.as_ref().unwrap().remaining_ms,
            30_000
        );
        recovered
            .resolve_recovery(RecoveryChoice::KeepPaused, sample(9_000_000, 99_000_000))
            .unwrap();
        assert_eq!(recovered.timer.status(), TimerStatus::Paused);
        recovered.tick(sample(9_300_000, 99_300_000));
        assert_eq!(
            recovered.timer.session.as_ref().unwrap().remaining_ms,
            30_000
        );
    }

    #[test]
    fn a_sleep_gap_is_persisted_as_recovery_before_timer_completion() {
        let directory = tempfile::tempdir().unwrap();
        let mut app = Application::open(directory.path(), sample(0, 1_000)).unwrap();
        app.start("session-1".into(), sample(0, 1_000)).unwrap();
        app.tick(sample(5_000, 6_000));
        let remaining_before_sleep = app.timer.session.as_ref().unwrap().remaining_ms;

        let result = app.tick(ClockSample {
            monotonic_ms: 6_000,
            boot_elapsed_ms: 60_000,
            utc_ms: 61_000,
            local_date: "2026-11-19".into(),
            local_offset_minutes: -360,
        });
        assert!(result.recovery_required);
        assert_eq!(app.timer.status(), TimerStatus::Recovery);
        assert_eq!(
            app.timer.session.as_ref().unwrap().remaining_ms,
            remaining_before_sleep
        );
        assert!(app.feedback().contains("系统睡眠"));

        app.resolve_recovery(
            RecoveryChoice::Continue,
            ClockSample {
                monotonic_ms: 6_000,
                boot_elapsed_ms: 60_000,
                utc_ms: 61_000,
                local_date: "2026-11-19".into(),
                local_offset_minutes: -360,
            },
        )
        .unwrap();
        assert_eq!(app.timer.status(), TimerStatus::Running);
        app.tick(ClockSample {
            monotonic_ms: 7_000,
            boot_elapsed_ms: 61_000,
            utc_ms: 62_000,
            local_date: "2026-11-19".into(),
            local_offset_minutes: -360,
        });
        assert_eq!(
            app.timer.session.as_ref().unwrap().remaining_ms,
            remaining_before_sleep - 1_000
        );
    }

    #[test]
    fn terminal_commit_creates_backup_and_preserves_analysis_export() {
        let directory = tempfile::tempdir().unwrap();
        let settings = TimerSettings {
            focus_minutes: 1,
            ..TimerSettings::default()
        };
        let mut app = Application::open(directory.path(), sample(0, 1_000)).unwrap();
        app.update_settings(settings, sample(0, 1_000)).unwrap();
        app.start("session-1".into(), sample(0, 1_000)).unwrap();
        app.tick(sample(60_000, 61_000));
        assert_eq!(app.timer.status(), TimerStatus::Completed);
        assert!(matches!(app.last_backup_status, BackupStatus::Created));
        let backups = fs::read_dir(directory.path().join("backups"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "sqlite3")
            })
            .count();
        assert_eq!(backups, 1);

        let export = directory.path().join("analysis.json");
        app.export_analysis_json(&export).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&fs::read(export).unwrap()).unwrap();
        assert_eq!(value["sessions"].as_array().unwrap().len(), 1);
        assert!(value["events"].as_array().unwrap().len() >= 3);

        let reopened = Application::open(directory.path(), sample(70_000, 71_000)).unwrap();
        assert_eq!(reopened.timer.status(), TimerStatus::Completed);
        assert_eq!(
            reopened.timer.session.as_ref().unwrap().status,
            SessionStatus::Completed
        );
        assert_eq!(reopened.timer.phase, Phase::Focus);
    }

    #[test]
    fn settings_history_is_kept_and_exported() {
        let directory = tempfile::tempdir().unwrap();
        let mut app = Application::open(directory.path(), sample(0, 1_000)).unwrap();
        app.update_settings(
            TimerSettings {
                focus_minutes: 30,
                ..TimerSettings::default()
            },
            sample(1, 1_001),
        )
        .unwrap();
        app.update_settings(
            TimerSettings {
                focus_minutes: 35,
                ..TimerSettings::default()
            },
            sample(2, 1_002),
        )
        .unwrap();
        let export = directory.path().join("analysis.json");
        app.export_analysis_json(&export).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&fs::read(export).unwrap()).unwrap();
        assert_eq!(value["settings_history"].as_array().unwrap().len(), 3);
        assert_eq!(value["settings"]["focus_minutes"], 35);
    }

    #[test]
    fn new_settings_preserve_the_current_target_and_apply_to_the_next_session() {
        let directory = tempfile::tempdir().unwrap();
        let mut app = Application::open(directory.path(), sample(0, 1_000)).unwrap();
        app.start("session-1".into(), sample(0, 1_000)).unwrap();
        assert_eq!(app.timer.session.as_ref().unwrap().target_ms, 25 * 60_000);

        app.update_settings(
            TimerSettings {
                focus_minutes: 30,
                ..TimerSettings::default()
            },
            sample(1_000, 2_000),
        )
        .unwrap();
        assert_eq!(app.timer.session.as_ref().unwrap().target_ms, 25 * 60_000);

        app.end_early(sample(2_000, 3_000)).unwrap();
        app.start("session-2".into(), sample(3_000, 4_000)).unwrap();
        assert_eq!(app.timer.session.as_ref().unwrap().target_ms, 30 * 60_000);
    }

    #[test]
    fn periodic_backup_is_polled_on_the_application_store() {
        let directory = tempfile::tempdir().unwrap();
        let mut app = Application::open(directory.path(), sample(0, 1_000)).unwrap();
        assert!(matches!(
            app.maybe_auto_backup(1_000 + Duration::from_secs(60 * 60).as_millis() as i64),
            BackupStatus::NotDue
        ));
    }
}
