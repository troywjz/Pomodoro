mod application;
mod domain;
mod platform;

use std::cell::{Cell, RefCell};
use std::io;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use application::{Application, ApplicationError};
use domain::{DomainEvent, RecoveryChoice, TimerSettings, TimerStatus};
use platform::{clock_sample, logical_width, monitor_metrics, physical_window_width};
use slint::{ComponentHandle, Timer, TimerMode};
use uuid::Uuid;

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let origin = Instant::now();
    let data_dir = data_directory()?;
    let mut args = std::env::args_os().skip(1);
    if let Some(command) = args.next() {
        let destination = match (command.to_str(), args.next(), args.next()) {
            (Some("--export-json"), Some(path), None) => PathBuf::from(path),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Usage: pomodoro [--export-json <path>]",
                )
                .into());
            }
        };
        if !data_dir.join("pomodoro.sqlite3").is_file() {
            return Err(
                io::Error::new(io::ErrorKind::NotFound, "Pomodoro database not found").into(),
            );
        }
        let store = pomodoro_storage::Store::open(&data_dir)?;
        store.export_json(&destination)?;
        return Ok(());
    }
    let application = Rc::new(RefCell::new(Application::open(
        &data_dir,
        clock_sample(origin),
    )?));
    let feedback = Rc::new(RefCell::new(String::new()));
    let dark = Rc::new(Cell::new(platform::system_prefers_dark_theme()));
    let expanded = Rc::new(Cell::new(true));

    let main_window = PomodoroWindow::new()?;
    let settings_window = SettingsWindow::new()?;
    let about_window = AboutWindow::new()?;

    main_window.set_dark(dark.get());
    main_window.set_expanded(expanded.get());
    settings_window.set_dark(dark.get());
    about_window.set_dark(dark.get());
    sync_main_window(&main_window, &application.borrow(), &feedback.borrow());
    sync_settings_window(&settings_window, &application.borrow());

    let (monitor_width, dpi) = monitor_metrics(&main_window.window());
    resize_main_window(&main_window, monitor_width, dpi, expanded.get());

    let main_weak = main_window.as_weak();
    let settings_weak = settings_window.as_weak();
    let about_weak = about_window.as_weak();

    {
        let application = application.clone();
        let feedback = feedback.clone();
        let main_weak = main_weak.clone();
        main_window.on_start_requested(move || {
            let now = clock_sample(origin);
            let result = application
                .borrow_mut()
                .start(Uuid::new_v4().to_string(), now);
            set_result_feedback(&feedback, result.map(|_| ()), "开始");
            refresh_main(&main_weak, &application, &feedback);
        });
    }
    {
        let application = application.clone();
        let feedback = feedback.clone();
        let main_weak = main_weak.clone();
        main_window.on_pause_requested(move || {
            let result = application.borrow_mut().pause(clock_sample(origin));
            let completed = matches!(result, Ok(DomainEvent::Completed(_)));
            if completed {
                feedback.borrow_mut().clear();
            } else {
                set_result_feedback(&feedback, result.map(|_| ()), "暂停");
            }
            refresh_main(&main_weak, &application, &feedback);
        });
    }
    {
        let application = application.clone();
        let feedback = feedback.clone();
        let main_weak = main_weak.clone();
        main_window.on_resume_requested(move || {
            let result = application.borrow_mut().resume(clock_sample(origin));
            set_result_feedback(&feedback, result.map(|_| ()), "继续");
            refresh_main(&main_weak, &application, &feedback);
        });
    }
    {
        let application = application.clone();
        let feedback = feedback.clone();
        let main_weak = main_weak.clone();
        main_window.on_end_requested(move || {
            let result = application.borrow_mut().end_early(clock_sample(origin));
            set_result_feedback(&feedback, result.map(|_| ()), "结束");
            refresh_main(&main_weak, &application, &feedback);
        });
    }
    {
        let application = application.clone();
        let feedback = feedback.clone();
        let main_weak = main_weak.clone();
        main_window.on_next_requested(move || {
            let result = application.borrow_mut().advance(clock_sample(origin));
            set_result_feedback(&feedback, result.map(|_| ()), "下一段");
            refresh_main(&main_weak, &application, &feedback);
        });
    }
    {
        let application = application.clone();
        let feedback = feedback.clone();
        let main_weak = main_weak.clone();
        main_window.on_recovery_continue_requested(move || {
            let result = application
                .borrow_mut()
                .resolve_recovery(RecoveryChoice::Continue, clock_sample(origin));
            set_result_feedback(&feedback, result.map(|_| ()), "恢复");
            refresh_main(&main_weak, &application, &feedback);
        });
    }
    {
        let application = application.clone();
        let feedback = feedback.clone();
        let main_weak = main_weak.clone();
        main_window.on_recovery_pause_requested(move || {
            let result = application
                .borrow_mut()
                .resolve_recovery(RecoveryChoice::KeepPaused, clock_sample(origin));
            set_result_feedback(&feedback, result.map(|_| ()), "保持暂停");
            refresh_main(&main_weak, &application, &feedback);
        });
    }
    {
        let application = application.clone();
        let feedback = feedback.clone();
        let main_weak = main_weak.clone();
        main_window.on_recovery_discard_requested(move || {
            let result = application
                .borrow_mut()
                .resolve_recovery(RecoveryChoice::Discard, clock_sample(origin));
            set_result_feedback(&feedback, result.map(|_| ()), "放弃");
            refresh_main(&main_weak, &application, &feedback);
        });
    }
    {
        let expanded = expanded.clone();
        let main_weak = main_weak.clone();
        main_window.on_toggle_requested(move || {
            expanded.set(!expanded.get());
            if let Some(window) = main_weak.upgrade() {
                window.set_expanded(expanded.get());
                let (monitor_width, dpi) = monitor_metrics(&window.window());
                resize_main_window(&window, monitor_width, dpi, expanded.get());
            }
        });
    }
    {
        let application = application.clone();
        let settings_weak = settings_weak.clone();
        main_window.on_settings_requested(move || {
            if let Some(window) = settings_weak.upgrade() {
                sync_settings_window(&window, &application.borrow());
                window.show().ok();
            }
        });
    }
    {
        let about_weak = about_weak.clone();
        let dark = dark.clone();
        main_window.on_about_requested(move || {
            if let Some(window) = about_weak.upgrade() {
                window.set_dark(dark.get());
                window.show().ok();
            }
        });
    }

    connect_settings(
        &settings_window,
        &application,
        &feedback,
        &main_weak,
        origin,
    );
    {
        let settings_weak = settings_window.as_weak();
        let application = application.clone();
        let feedback = feedback.clone();
        let main_weak = main_weak.clone();
        settings_window.on_close_requested(move || {
            if let Some(window) = settings_weak.upgrade() {
                cancel_settings(&window, &application, &feedback, &main_weak);
            }
        });
    }
    {
        let about_weak = about_weak.clone();
        about_window.on_close_requested(move || {
            if let Some(window) = about_weak.upgrade() {
                window.hide().ok();
            }
        });
    }

    let tick_timer = Timer::default();
    {
        let application = application.clone();
        let feedback = feedback.clone();
        let main_weak = main_weak.clone();
        tick_timer.start(TimerMode::Repeated, Duration::from_millis(200), move || {
            let tick = application.borrow_mut().tick(clock_sample(origin));
            if tick.completed.is_some() {
                feedback.borrow_mut().clear();
            }
            refresh_main(&main_weak, &application, &feedback);
        });
    }

    let backup_timer = Timer::default();
    {
        let application = application.clone();
        let feedback = feedback.clone();
        let main_weak = main_weak.clone();
        backup_timer.start(TimerMode::Repeated, Duration::from_secs(30), move || {
            let now = clock_sample(origin);
            application.borrow_mut().maybe_auto_backup(now.utc_ms);
            refresh_main(&main_weak, &application, &feedback);
        });
    }

    let platform_timer = Timer::default();
    {
        let main_weak = main_weak.clone();
        let dark = dark.clone();
        let expanded = expanded.clone();
        let settings_weak = settings_weak.clone();
        let about_weak = about_weak.clone();
        let mut last_metrics = (monitor_width, dpi);
        platform_timer.start(
            TimerMode::Repeated,
            Duration::from_millis(1_000),
            move || {
                let system_dark = platform::system_prefers_dark_theme();
                if dark.replace(system_dark) != system_dark {
                    if let Some(window) = main_weak.upgrade() {
                        window.set_dark(system_dark);
                    }
                    if let Some(window) = settings_weak.upgrade() {
                        window.set_dark(system_dark);
                    }
                    if let Some(window) = about_weak.upgrade() {
                        window.set_dark(system_dark);
                    }
                }
                if let Some(window) = main_weak.upgrade() {
                    let metrics = monitor_metrics(&window.window());
                    if metrics != last_metrics {
                        last_metrics = metrics;
                        resize_main_window(&window, metrics.0, metrics.1, expanded.get());
                    }
                }
            },
        );
    }

    main_window.show()?;
    main_window.run()?;
    Ok(())
}

fn connect_settings(
    settings_window: &SettingsWindow,
    application: &Rc<RefCell<Application>>,
    feedback: &Rc<RefCell<String>>,
    main_weak: &slint::Weak<PomodoroWindow>,
    origin: Instant,
) {
    {
        let settings_weak = settings_window.as_weak();
        settings_window.on_focus_delta(move |delta| {
            if let Some(window) = settings_weak.upgrade() {
                window.set_focus_minutes((window.get_focus_minutes() + delta).clamp(1, 180));
            }
        });
    }
    {
        let settings_weak = settings_window.as_weak();
        settings_window.on_short_delta(move |delta| {
            if let Some(window) = settings_weak.upgrade() {
                window.set_short_minutes((window.get_short_minutes() + delta).clamp(1, 60));
            }
        });
    }
    {
        let settings_weak = settings_window.as_weak();
        settings_window.on_long_delta(move |delta| {
            if let Some(window) = settings_weak.upgrade() {
                window.set_long_minutes((window.get_long_minutes() + delta).clamp(1, 120));
            }
        });
    }
    {
        let settings_weak = settings_window.as_weak();
        settings_window.on_rounds_delta(move |delta| {
            if let Some(window) = settings_weak.upgrade() {
                window.set_rounds((window.get_rounds() + delta).clamp(1, 12));
            }
        });
    }

    {
        let settings_weak = settings_window.as_weak();
        let application = application.clone();
        let feedback = feedback.clone();
        let main_weak = main_weak.clone();
        settings_window.on_save_requested(move |focus, short_break, long_break, rounds| {
            let settings = TimerSettings {
                focus_minutes: u32::try_from(focus).unwrap_or(0),
                short_break_minutes: u32::try_from(short_break).unwrap_or(0),
                long_break_minutes: u32::try_from(long_break).unwrap_or(0),
                rounds_until_long_break: u8::try_from(rounds).unwrap_or(0),
            };
            let result = application
                .borrow_mut()
                .update_settings(settings, clock_sample(origin));
            let saved = result.is_ok();
            set_result_feedback(&feedback, result, "设置");
            if saved {
                if let Some(window) = settings_weak.upgrade() {
                    sync_settings_window(&window, &application.borrow());
                    window.hide().ok();
                }
            }
            refresh_main(&main_weak, &application, &feedback);
        });
    }

    {
        let settings_weak = settings_window.as_weak();
        let application = application.clone();
        let feedback = feedback.clone();
        let main_weak = main_weak.clone();
        settings_window.on_cancel_requested(move || {
            if let Some(window) = settings_weak.upgrade() {
                cancel_settings(&window, &application, &feedback, &main_weak);
            }
        });
    }
}

fn cancel_settings(
    settings_window: &SettingsWindow,
    application: &Rc<RefCell<Application>>,
    feedback: &Rc<RefCell<String>>,
    main_weak: &slint::Weak<PomodoroWindow>,
) {
    sync_settings_window(settings_window, &application.borrow());
    settings_window.hide().ok();
    feedback.borrow_mut().clear();
    refresh_main(main_weak, application, feedback);
}

fn sync_settings_window(window: &SettingsWindow, application: &Application) {
    let settings = &application.timer.settings;
    window.set_focus_minutes(settings.focus_minutes as i32);
    window.set_short_minutes(settings.short_break_minutes as i32);
    window.set_long_minutes(settings.long_break_minutes as i32);
    window.set_rounds(settings.rounds_until_long_break as i32);
    window.set_draft_focus(settings.focus_minutes as i32);
    window.set_draft_short(settings.short_break_minutes as i32);
    window.set_draft_long(settings.long_break_minutes as i32);
    window.set_draft_rounds(settings.rounds_until_long_break as i32);
}

fn sync_main_window(window: &PomodoroWindow, application: &Application, feedback: &str) {
    let timer = &application.timer;
    let (remaining_ms, target_ms, round) = timer
        .session
        .as_ref()
        .map(|session| (session.remaining_ms, session.target_ms, session.round))
        .unwrap_or((
            timer.settings.target_ms(timer.phase),
            timer.settings.target_ms(timer.phase),
            timer.round,
        ));
    window.set_phase_title(timer.phase.title().into());
    window.set_round_label(format!("第 {round} 轮").into());
    window.set_time_label(format_time(remaining_ms).into());
    window.set_progress(if target_ms == 0 {
        0.0
    } else {
        remaining_ms as f32 / target_ms as f32
    });
    window.set_progress_path(
        progress_path(
            if target_ms == 0 {
                0.0
            } else {
                remaining_ms as f32 / target_ms as f32
            },
            !window.get_expanded(),
        )
        .into(),
    );
    let status = match timer.status() {
        TimerStatus::Idle => "待开始",
        TimerStatus::Running => "进行中",
        TimerStatus::Paused => "已暂停",
        TimerStatus::Completed => "已完成",
        TimerStatus::Recovery => "等待恢复处理",
    };
    window.set_status_label(status.into());
    window.set_save_label(application.save_label().into());
    let application_feedback = application.feedback();
    let combined_feedback = match (feedback.is_empty(), application_feedback.is_empty()) {
        (true, true) => String::new(),
        (false, true) => feedback.to_owned(),
        (true, false) => application_feedback,
        (false, false) => format!("{feedback}；{application_feedback}"),
    };
    window.set_feedback(combined_feedback.into());
    window.set_running(timer.status() == TimerStatus::Running);
    window.set_paused(timer.status() == TimerStatus::Paused);
    window.set_completed(timer.status() == TimerStatus::Completed);
    window.set_recovering(timer.status() == TimerStatus::Recovery);
}

fn refresh_main(
    main_weak: &slint::Weak<PomodoroWindow>,
    application: &Rc<RefCell<Application>>,
    feedback: &Rc<RefCell<String>>,
) {
    if let Some(window) = main_weak.upgrade() {
        sync_main_window(&window, &application.borrow(), &feedback.borrow());
    }
}

fn set_result_feedback<T>(
    feedback: &Rc<RefCell<String>>,
    result: Result<T, ApplicationError>,
    action: &str,
) {
    *feedback.borrow_mut() = match result {
        Ok(_) => String::new(),
        Err(error) => format!("{action}失败：{error}"),
    };
}

fn format_time(remaining_ms: u64) -> String {
    let total_seconds = remaining_ms.div_ceil(1_000);
    format!("{:02}:{:02}", total_seconds / 60, total_seconds % 60)
}

fn progress_path(progress: f32, compact: bool) -> String {
    let radius = if compact { 20.0_f32 } else { 24.0_f32 };
    let center = radius + 2.0;
    let normalized = progress.clamp(0.0, 1.0);
    if normalized <= 0.0 {
        return String::new();
    }
    if normalized >= 1.0 {
        return format!(
            "M {center} 2 A {radius} {radius} 0 1 1 {center} {} A {radius} {radius} 0 1 1 {center} 2",
            center * 2.0 - 2.0
        );
    }

    let angle = std::f32::consts::TAU * normalized;
    let x = center + radius * angle.sin();
    let y = center - radius * angle.cos();
    let large_arc = if normalized > 0.5 { 1 } else { 0 };
    format!("M {center} 2 A {radius} {radius} 0 {large_arc} 1 {x:.3} {y:.3}")
}

fn resize_main_window(window: &PomodoroWindow, monitor_width_px: u32, dpi: u32, expanded: bool) {
    let (width_px, height_px, width_dip) = window_dimensions(monitor_width_px, dpi, expanded);
    window.set_target_width(width_dip);
    window
        .window()
        .set_size(slint::PhysicalSize::new(width_px, height_px));
}

fn window_dimensions(monitor_width_px: u32, dpi: u32, expanded: bool) -> (u32, u32, f32) {
    let width_px = physical_window_width(monitor_width_px as i32);
    let width_dip = logical_width(width_px, dpi);
    let layout_scale = width_dip / 192.0;
    let dpi_scale = if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 };
    let base_height_dip = if expanded { 300.0 } else { 88.0 };
    let height_px = (base_height_dip * layout_scale * dpi_scale)
        .round()
        .max(1.0) as u32;
    (width_px, height_px, width_dip)
}

fn data_directory() -> Result<PathBuf, io::Error> {
    if let Some(path) = std::env::var_os("POMODORO_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }

    #[cfg(windows)]
    {
        let local_app_data = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Windows LOCALAPPDATA is unavailable",
            )
        })?;
        return Ok(PathBuf::from(local_app_data).join("Pomodoro"));
    }

    #[cfg(not(windows))]
    {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("pomodoro-data"));
        Ok(base.join("Pomodoro"))
    }
}

#[cfg(test)]
mod tests {
    use super::window_dimensions;

    #[test]
    fn ten_percent_window_scales_height_at_small_and_high_dpi_sizes() {
        assert_eq!(window_dimensions(1366, 96, true), (137, 214, 137.0));
        assert_eq!(window_dimensions(1366, 96, false), (137, 63, 137.0));
        assert_eq!(window_dimensions(2560, 120, true), (256, 400, 204.8));
        assert_eq!(window_dimensions(3840, 144, true), (384, 600, 256.0));
    }
}
