use serde::{Deserialize, Serialize};

const MILLIS_PER_MINUTE: u64 = 60_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Focus,
    ShortBreak,
    LongBreak,
}

impl Phase {
    pub fn title(self) -> &'static str {
        match self {
            Self::Focus => "专注",
            Self::ShortBreak => "短休",
            Self::LongBreak => "长休",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Focus => "focus",
            Self::ShortBreak => "short_break",
            Self::LongBreak => "long_break",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TimerSettings {
    pub focus_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
    pub rounds_until_long_break: u8,
    #[serde(default)]
    pub topmost: bool,
}

impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            rounds_until_long_break: 4,
            topmost: false,
        }
    }
}

impl TimerSettings {
    pub fn target_ms(&self, phase: Phase) -> u64 {
        let minutes = match phase {
            Phase::Focus => self.focus_minutes,
            Phase::ShortBreak => self.short_break_minutes,
            Phase::LongBreak => self.long_break_minutes,
        };
        u64::from(minutes) * MILLIS_PER_MINUTE
    }

    pub fn validate(&self) -> Result<(), TimerError> {
        if !(1..=180).contains(&self.focus_minutes)
            || !(1..=60).contains(&self.short_break_minutes)
            || !(1..=120).contains(&self.long_break_minutes)
            || !(1..=12).contains(&self.rounds_until_long_break)
        {
            return Err(TimerError::InvalidSettings);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Running,
    Paused,
    Completed,
    Recovery,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerStatus {
    Idle,
    Running,
    Paused,
    Completed,
    Recovery,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClockSample {
    pub monotonic_ms: u64,
    pub boot_elapsed_ms: u64,
    pub utc_ms: i64,
    pub local_date: String,
    pub local_offset_minutes: i32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TimerSession {
    pub session_id: String,
    pub phase: Phase,
    pub round: u8,
    pub status: SessionStatus,
    pub target_ms: u64,
    pub remaining_ms: u64,
    pub actual_active_ms: u64,
    pub started_utc_ms: i64,
    pub start_local_date: String,
    pub start_offset_minutes: i32,
    #[serde(skip)]
    last_sample_monotonic_ms: u64,
    #[serde(skip)]
    last_boot_elapsed_ms: u64,
    #[serde(skip)]
    status_before_recovery: Option<SessionStatus>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TimerCore {
    pub settings: TimerSettings,
    pub phase: Phase,
    pub round: u8,
    pub runtime_seq: u64,
    pub session: Option<TimerSession>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DomainEvent {
    Started {
        session_id: String,
        phase: Phase,
        round: u8,
    },
    Paused {
        session_id: String,
        remaining_ms: u64,
    },
    Resumed {
        session_id: String,
    },
    Completed(SessionRecord),
    Ended(SessionRecord),
    Advanced {
        from: Phase,
        to: Phase,
    },
    RecoveryContinued {
        session_id: String,
    },
    RecoveryRequired {
        session_id: String,
        reason: RecoveryReason,
    },
    SettingsChanged(TimerSettings),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryReason {
    SystemSleep,
    MonotonicClockReset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryChoice {
    Continue,
    Discard,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimerError {
    InvalidSettings,
    NoActiveSession,
    AlreadyRunning,
    NotRunning,
    NotPaused,
    NotCompleted,
    NotRecovering,
}

impl std::fmt::Display for TimerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TimerError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TickResult {
    pub elapsed_ms: u64,
    pub completed: Option<SessionRecord>,
    pub recovery_required: bool,
    pub recovery_reason: Option<RecoveryReason>,
}

impl TimerCore {
    pub fn new(settings: TimerSettings) -> Result<Self, TimerError> {
        settings.validate()?;
        Ok(Self {
            settings,
            phase: Phase::Focus,
            round: 1,
            runtime_seq: 0,
            session: None,
        })
    }

    pub fn status(&self) -> TimerStatus {
        match self.session.as_ref().map(|session| session.status) {
            None => TimerStatus::Idle,
            Some(SessionStatus::Running) => TimerStatus::Running,
            Some(SessionStatus::Paused) => TimerStatus::Paused,
            Some(SessionStatus::Completed) => TimerStatus::Completed,
            Some(SessionStatus::Recovery) => TimerStatus::Recovery,
        }
    }

    pub fn start(
        &mut self,
        session_id: String,
        sample: ClockSample,
    ) -> Result<DomainEvent, TimerError> {
        if self.session.is_some() {
            return Err(TimerError::AlreadyRunning);
        }
        let target_ms = self.settings.target_ms(self.phase);
        self.session = Some(TimerSession {
            session_id: session_id.clone(),
            phase: self.phase,
            round: self.round,
            status: SessionStatus::Running,
            target_ms,
            remaining_ms: target_ms,
            actual_active_ms: 0,
            started_utc_ms: sample.utc_ms,
            start_local_date: sample.local_date,
            start_offset_minutes: sample.local_offset_minutes,
            last_sample_monotonic_ms: sample.monotonic_ms,
            last_boot_elapsed_ms: sample.boot_elapsed_ms,
            status_before_recovery: None,
        });
        Ok(DomainEvent::Started {
            session_id,
            phase: self.phase,
            round: self.round,
        })
    }

    pub fn tick(&mut self, sample: ClockSample) -> TickResult {
        let Some(session) = self.session.as_mut() else {
            return TickResult::default();
        };
        if session.status != SessionStatus::Running {
            return TickResult::default();
        }

        if sample.monotonic_ms < session.last_sample_monotonic_ms {
            session.status_before_recovery = Some(SessionStatus::Running);
            session.status = SessionStatus::Recovery;
            session.last_sample_monotonic_ms = sample.monotonic_ms;
            session.last_boot_elapsed_ms = sample.boot_elapsed_ms;
            return TickResult {
                recovery_required: true,
                recovery_reason: Some(RecoveryReason::MonotonicClockReset),
                ..TickResult::default()
            };
        }

        let elapsed_ms = sample.monotonic_ms - session.last_sample_monotonic_ms;
        if sample.boot_elapsed_ms < session.last_boot_elapsed_ms {
            session.status_before_recovery = Some(SessionStatus::Running);
            session.status = SessionStatus::Recovery;
            session.last_sample_monotonic_ms = sample.monotonic_ms;
            session.last_boot_elapsed_ms = sample.boot_elapsed_ms;
            return TickResult {
                recovery_required: true,
                recovery_reason: Some(RecoveryReason::MonotonicClockReset),
                ..TickResult::default()
            };
        }
        let boot_elapsed_ms = sample.boot_elapsed_ms - session.last_boot_elapsed_ms;
        if boot_elapsed_ms > elapsed_ms.saturating_add(500) {
            session.status_before_recovery = Some(SessionStatus::Running);
            session.status = SessionStatus::Recovery;
            session.last_sample_monotonic_ms = sample.monotonic_ms;
            session.last_boot_elapsed_ms = sample.boot_elapsed_ms;
            return TickResult {
                recovery_required: true,
                recovery_reason: Some(RecoveryReason::SystemSleep),
                ..TickResult::default()
            };
        }
        if elapsed_ms == 0 {
            return TickResult::default();
        }
        session.last_sample_monotonic_ms = sample.monotonic_ms;
        session.last_boot_elapsed_ms = sample.boot_elapsed_ms;
        let counted_ms = elapsed_ms.min(session.remaining_ms);
        session.actual_active_ms = session.actual_active_ms.saturating_add(counted_ms);
        session.remaining_ms -= counted_ms;
        self.runtime_seq = self.runtime_seq.saturating_add(1);

        if session.remaining_ms == 0 {
            session.status = SessionStatus::Completed;
            let record = session.to_record(sample.utc_ms, "completed", None);
            TickResult {
                elapsed_ms,
                completed: Some(record),
                ..TickResult::default()
            }
        } else {
            TickResult {
                elapsed_ms,
                completed: None,
                ..TickResult::default()
            }
        }
    }

    pub fn pause(&mut self, sample: ClockSample) -> Result<DomainEvent, TimerError> {
        self.tick(sample);
        let session = self.session.as_mut().ok_or(TimerError::NoActiveSession)?;
        if session.status != SessionStatus::Running {
            return Err(TimerError::NotRunning);
        }
        session.status = SessionStatus::Paused;
        Ok(DomainEvent::Paused {
            session_id: session.session_id.clone(),
            remaining_ms: session.remaining_ms,
        })
    }

    pub fn resume(&mut self, sample: ClockSample) -> Result<DomainEvent, TimerError> {
        let session = self.session.as_mut().ok_or(TimerError::NoActiveSession)?;
        if session.status != SessionStatus::Paused {
            return Err(TimerError::NotPaused);
        }
        session.status = SessionStatus::Running;
        session.last_sample_monotonic_ms = sample.monotonic_ms;
        session.last_boot_elapsed_ms = sample.boot_elapsed_ms;
        Ok(DomainEvent::Resumed {
            session_id: session.session_id.clone(),
        })
    }

    pub fn end_early(&mut self, sample: ClockSample) -> Result<DomainEvent, TimerError> {
        let ended_utc_ms = sample.utc_ms;
        let tick = self.tick(sample);
        if let Some(record) = tick.completed {
            return Ok(DomainEvent::Completed(record));
        }
        let session = self.session.as_ref().ok_or(TimerError::NoActiveSession)?;
        if session.status == SessionStatus::Recovery {
            return Err(TimerError::NotRecovering);
        }
        if session.status == SessionStatus::Completed {
            return Err(TimerError::NotCompleted);
        }
        let outcome = if session.phase == Phase::Focus {
            "interrupted"
        } else {
            "skipped"
        };
        let reason = if outcome == "interrupted" {
            "ended_early"
        } else {
            "skipped_early"
        };
        let record = session.to_record(ended_utc_ms, outcome, Some(reason.to_owned()));
        self.session = None;
        Ok(DomainEvent::Ended(record))
    }

    pub fn advance(&mut self) -> Result<DomainEvent, TimerError> {
        let session = self.session.as_ref().ok_or(TimerError::NoActiveSession)?;
        if session.status != SessionStatus::Completed {
            return Err(TimerError::NotCompleted);
        }
        let from = self.phase;
        self.phase = self.next_phase();
        self.session = None;
        Ok(DomainEvent::Advanced {
            from,
            to: self.phase,
        })
    }

    pub fn restore_for_recovery(&mut self, sample: ClockSample) {
        if let Some(session) = self.session.as_mut() {
            if session.status == SessionStatus::Running || session.status == SessionStatus::Paused {
                session.status_before_recovery = Some(session.status);
                session.status = SessionStatus::Recovery;
                session.last_sample_monotonic_ms = sample.monotonic_ms;
                session.last_boot_elapsed_ms = sample.boot_elapsed_ms;
            }
        }
    }

    pub fn resolve_recovery(
        &mut self,
        choice: RecoveryChoice,
        sample: ClockSample,
    ) -> Result<Option<DomainEvent>, TimerError> {
        let session = self.session.as_mut().ok_or(TimerError::NoActiveSession)?;
        if session.status != SessionStatus::Recovery {
            return Err(TimerError::NotRecovering);
        }
        let id = session.session_id.clone();
        match choice {
            RecoveryChoice::Continue => {
                session.status = SessionStatus::Running;
                session.last_sample_monotonic_ms = sample.monotonic_ms;
                session.last_boot_elapsed_ms = sample.boot_elapsed_ms;
                session.status_before_recovery = None;
                Ok(Some(DomainEvent::RecoveryContinued { session_id: id }))
            }
            RecoveryChoice::Discard => {
                let phase = session.phase;
                let outcome = if phase == Phase::Focus {
                    "interrupted"
                } else {
                    "skipped"
                };
                let record = session.to_record(
                    sample.utc_ms,
                    outcome,
                    Some("recovery_discarded".to_owned()),
                );
                self.session = None;
                Ok(Some(DomainEvent::Ended(record)))
            }
        }
    }

    fn next_phase(&mut self) -> Phase {
        match self.phase {
            Phase::Focus => {
                if self.round >= self.settings.rounds_until_long_break {
                    Phase::LongBreak
                } else {
                    Phase::ShortBreak
                }
            }
            Phase::ShortBreak => {
                self.round = self.round.saturating_add(1);
                Phase::Focus
            }
            Phase::LongBreak => {
                self.round = 1;
                Phase::Focus
            }
        }
    }
}

impl TimerSession {
    fn to_record(&self, ended_utc_ms: i64, outcome: &str, reason: Option<String>) -> SessionRecord {
        SessionRecord {
            session_id: self.session_id.clone(),
            phase: self.phase.key().to_owned(),
            target_ms: self.target_ms,
            actual_active_ms: self.actual_active_ms.min(self.target_ms),
            started_utc_ms: self.started_utc_ms,
            ended_utc_ms,
            start_local_date: self.start_local_date.clone(),
            start_offset_minutes: self.start_offset_minutes,
            outcome: outcome.to_owned(),
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(monotonic_ms: u64, utc_ms: i64) -> ClockSample {
        ClockSample {
            monotonic_ms,
            boot_elapsed_ms: monotonic_ms,
            utc_ms,
            local_date: "2026-11-19".to_owned(),
            local_offset_minutes: -360,
        }
    }

    fn short_timer() -> TimerCore {
        TimerCore::new(TimerSettings {
            focus_minutes: 1,
            short_break_minutes: 1,
            long_break_minutes: 2,
            rounds_until_long_break: 4,
            ..TimerSettings::default()
        })
        .unwrap()
    }

    #[test]
    fn pauses_exclude_the_pause_interval() {
        let mut timer = short_timer();
        timer.start("session-1".into(), sample(0, 1_000)).unwrap();
        timer.tick(sample(20_000, 21_000));
        timer.pause(sample(30_000, 31_000)).unwrap();
        assert_eq!(timer.session.as_ref().unwrap().remaining_ms, 30_000);

        timer.tick(sample(300_000, 301_000));
        assert_eq!(timer.session.as_ref().unwrap().remaining_ms, 30_000);
        timer.resume(sample(300_000, 301_000)).unwrap();
        timer.tick(sample(330_000, 331_000));
        assert_eq!(timer.session.as_ref().unwrap().remaining_ms, 0);
        assert_eq!(timer.status(), TimerStatus::Completed);
    }

    #[test]
    fn completion_is_emitted_once_even_when_ticks_repeat() {
        let mut timer = short_timer();
        timer.start("session-1".into(), sample(0, 1_000)).unwrap();
        let first = timer.tick(sample(65_000, 66_000));
        assert_eq!(first.completed.unwrap().actual_active_ms, 60_000);
        assert!(timer.tick(sample(90_000, 91_000)).completed.is_none());
        assert_eq!(timer.status(), TimerStatus::Completed);
    }

    #[test]
    fn sleep_gap_requires_recovery_and_does_not_consume_remaining_time() {
        let mut timer = short_timer();
        timer
            .start("session-1".into(), sample(1_000, 1_000))
            .unwrap();
        timer.tick(sample(2_000, 2_000));
        let before_sleep = timer.session.as_ref().unwrap().remaining_ms;
        let wake = timer.tick(ClockSample {
            monotonic_ms: 3_000,
            boot_elapsed_ms: 62_000,
            utc_ms: 62_000,
            ..sample(3_000, 62_000)
        });
        assert!(wake.recovery_required);
        assert_eq!(wake.recovery_reason, Some(RecoveryReason::SystemSleep));
        assert_eq!(timer.status(), TimerStatus::Recovery);
        assert_eq!(timer.session.as_ref().unwrap().remaining_ms, before_sleep);
    }

    #[test]
    fn monotonic_clock_regression_requires_recovery() {
        let mut timer = short_timer();
        timer
            .start("session-1".into(), sample(5_000, 5_000))
            .unwrap();
        let result = timer.tick(sample(4_000, 6_000));
        assert!(result.recovery_required);
        assert_eq!(
            result.recovery_reason,
            Some(RecoveryReason::MonotonicClockReset)
        );
        assert_eq!(timer.status(), TimerStatus::Recovery);
    }

    #[test]
    fn recovery_requires_a_decision_and_does_not_charge_offline_time() {
        let mut timer = short_timer();
        timer.start("session-1".into(), sample(0, 1_000)).unwrap();
        timer.tick(sample(15_000, 16_000));
        timer.restore_for_recovery(sample(0, 1_000));
        assert_eq!(timer.status(), TimerStatus::Recovery);
        assert_eq!(timer.session.as_ref().unwrap().remaining_ms, 45_000);

        assert_eq!(timer.tick(sample(300_000, 99_000_000)).elapsed_ms, 0);
        assert_eq!(timer.status(), TimerStatus::Recovery);
        assert_eq!(timer.session.as_ref().unwrap().remaining_ms, 45_000);
        timer
            .resolve_recovery(RecoveryChoice::Continue, sample(300_000, 99_000_000))
            .unwrap();
        assert_eq!(timer.tick(sample(301_000, 99_001_000)).elapsed_ms, 1_000);
    }

    #[test]
    fn four_focus_rounds_schedule_a_long_break() {
        let mut timer = short_timer();
        for round in 1..=4 {
            assert_eq!(timer.round, round);
            timer
                .start(format!("focus-{round}"), sample(0, 1_000))
                .unwrap();
            timer.tick(sample(60_000, 61_000));
            timer.advance().unwrap();
            if round < 4 {
                assert_eq!(timer.phase, Phase::ShortBreak);
                timer
                    .start(format!("break-{round}"), sample(0, 1_000))
                    .unwrap();
                timer.tick(sample(60_000, 61_000));
                timer.advance().unwrap();
            }
        }
        assert_eq!(timer.phase, Phase::LongBreak);
    }

    #[test]
    fn existing_settings_default_to_normal_window_layer() {
        let settings: TimerSettings = serde_json::from_str(
            r#"{"focus_minutes":40,"short_break_minutes":5,"long_break_minutes":15,"rounds_until_long_break":4}"#,
        )
        .unwrap();
        assert_eq!(settings.focus_minutes, 40);
        assert!(!settings.topmost);
    }

    #[test]
    fn settings_validation_matches_approved_limits() {
        let mut settings = TimerSettings {
            focus_minutes: 180,
            short_break_minutes: 60,
            long_break_minutes: 120,
            rounds_until_long_break: 12,
            ..TimerSettings::default()
        };
        assert!(settings.validate().is_ok());
        settings.short_break_minutes = 61;
        assert_eq!(settings.validate(), Err(TimerError::InvalidSettings));
        settings.short_break_minutes = 60;
        settings.long_break_minutes = 121;
        assert_eq!(settings.validate(), Err(TimerError::InvalidSettings));
    }
}
