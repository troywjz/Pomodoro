use crate::domain::{
    ClockSample, DomainEvent, RecoveryChoice, TickResult, TimerCore, TimerError, TimerSettings,
};

pub struct Application {
    pub timer: TimerCore,
}

impl Application {
    pub fn new(settings: TimerSettings) -> Result<Self, TimerError> {
        Ok(Self {
            timer: TimerCore::new(settings)?,
        })
    }

    pub fn start(
        &mut self,
        session_id: String,
        now: ClockSample,
    ) -> Result<DomainEvent, TimerError> {
        self.timer.start(session_id, now)
    }

    pub fn tick(&mut self, now: ClockSample) -> TickResult {
        self.timer.tick(now)
    }

    pub fn pause(&mut self, now: ClockSample) -> Result<DomainEvent, TimerError> {
        let tick = self.timer.tick(now.clone());
        if let Some(record) = tick.completed {
            return Ok(DomainEvent::Completed(record));
        }
        self.timer.pause(now)
    }

    pub fn resume(&mut self, now: ClockSample) -> Result<DomainEvent, TimerError> {
        self.timer.resume(now)
    }

    pub fn end_early(&mut self, now: ClockSample) -> Result<DomainEvent, TimerError> {
        self.timer.end_early(now)
    }

    pub fn advance(&mut self) -> Result<DomainEvent, TimerError> {
        self.timer.advance()
    }

    pub fn resolve_recovery(
        &mut self,
        choice: RecoveryChoice,
        now: ClockSample,
    ) -> Result<Option<DomainEvent>, TimerError> {
        self.timer.resolve_recovery(choice, now)
    }

    pub fn update_settings(&mut self, settings: TimerSettings) -> Result<(), TimerError> {
        settings.validate()?;
        if self.timer.session.is_some() {
            return Err(TimerError::AlreadyRunning);
        }
        self.timer.settings = settings;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Phase;

    fn sample(monotonic_ms: u64, utc_ms: i64) -> ClockSample {
        ClockSample {
            monotonic_ms,
            utc_ms,
            local_date: "2026-11-19".into(),
            local_offset_minutes: -360,
        }
    }

    #[test]
    fn defaults_match_the_approved_cycle() {
        let app = Application::new(TimerSettings::default()).unwrap();
        assert_eq!(app.timer.settings.focus_minutes, 25);
        assert_eq!(app.timer.settings.short_break_minutes, 5);
        assert_eq!(app.timer.settings.long_break_minutes, 15);
        assert_eq!(app.timer.settings.rounds_until_long_break, 4);
        assert_eq!(app.timer.phase, Phase::Focus);
    }

    #[test]
    fn setting_changes_are_rejected_during_a_session() {
        let mut app = Application::new(TimerSettings::default()).unwrap();
        app.start("session-1".into(), sample(0, 1_000)).unwrap();
        let error = app.update_settings(TimerSettings {
            focus_minutes: 30,
            ..TimerSettings::default()
        });
        assert_eq!(error, Err(TimerError::AlreadyRunning));
        assert_eq!(app.timer.settings.focus_minutes, 25);
    }
}
