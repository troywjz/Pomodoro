#![cfg_attr(windows, windows_subsystem = "windows")]

mod application;
mod domain;
mod platform;

use std::cell::{Cell, RefCell};
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant};

use application::{Application, ApplicationError};
use domain::{CLOCK_UTC_OFFSETS, DomainEvent, RecoveryChoice, TimerSettings, TimerStatus};
use platform::{clock_sample, logical_width, monitor_metrics, physical_window_width};
use slint::{CloseRequestResponse, ComponentHandle, Timer, TimerMode};
use uuid::Uuid;

slint::include_modules!();

fn main() -> ExitCode {
    let command_line_mode = std::env::args_os().nth(1).is_some();
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            show_startup_error(&error.to_string(), command_line_mode);
            ExitCode::FAILURE
        }
    }
}

#[cfg(windows)]
fn show_startup_error(message: &str, command_line_mode: bool) {
    if command_line_mode {
        use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
        // The GUI executable never creates a console. Report CLI errors to its caller's console.
        unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
        eprintln!("{message}");
    } else {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
        let caption: Vec<u16> = "Pomodoro 启动失败".encode_utf16().chain([0]).collect();
        let message: Vec<u16> = message.encode_utf16().chain([0]).collect();
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                message.as_ptr(),
                caption.as_ptr(),
                MB_OK | MB_ICONERROR,
            )
        };
    }
}

#[cfg(not(windows))]
fn show_startup_error(message: &str, _: bool) {
    eprintln!("{message}");
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let origin = Instant::now();
    let data_dir = data_directory()?;
    let legacy_dir = legacy_data_directory();
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
        let legacy_database = legacy_dir
            .as_ref()
            .is_some_and(|directory| directory.join("pomodoro.sqlite3").is_file());
        if !data_dir.join("journal.jsonl").is_file()
            && !data_dir.join("pomodoro.sqlite3").is_file()
            && !legacy_database
        {
            return Err(io::Error::new(io::ErrorKind::NotFound, "Pomodoro data not found").into());
        }
        let store = pomodoro_storage::Store::open_with_legacy(
            &data_dir,
            legacy_dir.as_deref().unwrap_or(&data_dir),
        )?;
        store.export_json(&destination)?;
        return Ok(());
    }
    let application = Rc::new(RefCell::new(Application::open_with_legacy(
        &data_dir,
        legacy_dir.as_deref().unwrap_or(&data_dir),
        clock_sample(origin),
    )?));
    let feedback = Rc::new(RefCell::new(String::new()));
    let dark = Rc::new(Cell::new(platform::system_prefers_dark_theme()));

    let main_window = PomodoroWindow::new()?;
    #[cfg(windows)]
    connect_window_drag(&main_window);
    let zones: Vec<_> = CLOCK_UTC_OFFSETS
        .iter()
        .copied()
        .map(utc_offset_label)
        .collect();
    main_window.set_wall_clock_zones(slint::ModelRc::new(slint::VecModel::from(zones)));
    main_window.set_dark(dark.get());
    sync_main_window(&main_window, &application.borrow(), &feedback.borrow());

    let (monitor_width, dpi) = monitor_metrics(&main_window.window());
    resize_main_window(&main_window, monitor_width, dpi);

    let main_weak = main_window.as_weak();

    {
        let application = application.clone();
        let main_weak = main_weak.clone();
        let feedback = feedback.clone();
        main_window.window().on_close_requested(move || {
            let result = application.borrow_mut().prepare_exit(clock_sample(origin));
            if result.is_ok() {
                return CloseRequestResponse::HideWindow;
            }

            if let Some(window) = main_weak.upgrade() {
                let old_height = window.get_target_height();
                window.set_expanded(true);
                window.set_page(0);
                let application = application.borrow();
                *feedback.borrow_mut() = if application.feedback().is_empty() {
                    "退出前保存失败，请重试。".to_owned()
                } else {
                    String::new()
                };
                sync_main_window(&window, &application, &feedback.borrow());
                if window.get_target_height() != old_height {
                    apply_main_size(&window, window.window().scale_factor());
                }
            }
            CloseRequestResponse::KeepWindowShown
        });
    }
    {
        let main_weak = main_weak.clone();
        main_window.on_exit_requested(move || {
            if let Some(window) = main_weak.upgrade() {
                window
                    .window()
                    .dispatch_event(slint::platform::WindowEvent::CloseRequested);
            }
        });
    }

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
        main_window.on_recovery_discard_requested(move || {
            let result = application
                .borrow_mut()
                .resolve_recovery(RecoveryChoice::Discard, clock_sample(origin));
            set_result_feedback(&feedback, result.map(|_| ()), "放弃");
            refresh_main(&main_weak, &application, &feedback);
        });
    }
    {
        let application = application.clone();
        let feedback = feedback.clone();
        let main_weak = main_weak.clone();
        main_window.on_toggle_requested(move || {
            if let Some(window) = main_weak.upgrade() {
                window.set_expanded(!window.get_expanded());
                sync_main_window(&window, &application.borrow(), &feedback.borrow());
                let (monitor_width, dpi) = monitor_metrics(&window.window());
                resize_main_window(&window, monitor_width, dpi);
            }
        });
    }
    connect_settings(&main_window, &application, &feedback, origin);

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
                }
                if let Some(window) = main_weak.upgrade() {
                    let metrics = monitor_metrics(&window.window());
                    if metrics != last_metrics {
                        last_metrics = metrics;
                        resize_main_window(&window, metrics.0, metrics.1);
                    }
                }
            },
        );
    }

    main_window.show()?;
    main_window.run()?;
    Ok(())
}

#[cfg(windows)]
fn connect_window_drag(window: &PomodoroWindow) {
    let anchor = Rc::new(Cell::new((0, 0)));
    let weak = window.as_weak();
    let start_anchor = anchor.clone();
    window.set_use_pointer_drag(true);
    window.on_drag_started(move |x, y| {
        if let Some(window) = weak.upgrade() {
            let scale = window.window().scale_factor();
            start_anchor.set(((x * scale).round() as i32, (y * scale).round() as i32));
        }
    });
    let weak = window.as_weak();
    window.on_drag_moved(move || {
        if let Some(window) = weak.upgrade() {
            let (x, y) = anchor.get();
            platform::move_window_with_pointer(window.window(), x, y);
        }
    });
}

fn connect_settings(
    window: &PomodoroWindow,
    application: &Rc<RefCell<Application>>,
    feedback: &Rc<RefCell<String>>,
    origin: Instant,
) {
    {
        let weak = window.as_weak();
        let application = application.clone();
        let feedback = feedback.clone();
        window.on_settings_requested(move || {
            if let Some(window) = weak.upgrade() {
                let app = application.borrow();
                let settings = &app.timer.settings;
                window.set_draft_focus(settings.focus_minutes as i32);
                window.set_draft_short(settings.short_break_minutes as i32);
                window.set_draft_long(settings.long_break_minutes as i32);
                window.set_draft_rounds(settings.rounds_until_long_break as i32);
                window.set_draft_topmost(settings.topmost);
                window.set_draft_show_wall_clock(settings.show_wall_clock);
                window.set_draft_wall_clock_zone(
                    CLOCK_UTC_OFFSETS
                        .iter()
                        .position(|&offset| offset == settings.wall_clock_offset_minutes)
                        .map_or(-1, |index| index as i32),
                );
                feedback.borrow_mut().clear();
                sync_main_window(&window, &app, &feedback.borrow());
                show_page(&window, 1);
            }
        });
    }
    {
        let weak = window.as_weak();
        window.on_page_requested(move |page| {
            if let Some(window) = weak.upgrade() {
                show_page(&window, page);
            }
        });
    }
    {
        let weak = window.as_weak();
        let application = application.clone();
        let feedback = feedback.clone();
        window.on_save_requested(
            move |focus, short_break, long_break, rounds, topmost, show_wall_clock, zone| {
                let wall_clock_offset_minutes = usize::try_from(zone)
                    .ok()
                    .and_then(|index| CLOCK_UTC_OFFSETS.get(index))
                    .copied()
                    .unwrap_or(i32::MAX);
                let settings = TimerSettings {
                    focus_minutes: u32::try_from(focus).unwrap_or(0),
                    short_break_minutes: u32::try_from(short_break).unwrap_or(0),
                    long_break_minutes: u32::try_from(long_break).unwrap_or(0),
                    rounds_until_long_break: u8::try_from(rounds).unwrap_or(0),
                    topmost,
                    show_wall_clock,
                    wall_clock_offset_minutes,
                };
                let result = application
                    .borrow_mut()
                    .update_settings(settings, clock_sample(origin));
                let saved = result.is_ok();
                set_result_feedback(&feedback, result, "设置");
                refresh_main(&weak, &application, &feedback);
                if saved && let Some(window) = weak.upgrade() {
                    show_page(&window, 0);
                }
            },
        );
    }
    {
        let weak = window.as_weak();
        let application = application.clone();
        let feedback = feedback.clone();
        window.on_cancel_requested(move || {
            feedback.borrow_mut().clear();
            refresh_main(&weak, &application, &feedback);
            if let Some(window) = weak.upgrade() {
                show_page(&window, 0);
            }
        });
    }
}

fn utc_offset_label(minutes: i32) -> slint::SharedString {
    let sign = if minutes < 0 { "−" } else { "+" };
    let absolute = minutes.abs();
    let hours = absolute / 60;
    let remainder = absolute % 60;
    if remainder == 0 {
        format!("UTC{sign}{hours}").into()
    } else {
        format!("UTC{sign}{hours}:{remainder:02}").into()
    }
}

#[test]
fn wall_clock_applies_fixed_offsets_across_dates() {
    assert_eq!(format_wall_clock(0, -210), "1969/12/31  20 : 30 : 00");
    assert_eq!(format_wall_clock(1, 345), "1970/01/01  05 : 45 : 01");
    assert_eq!(utc_offset_label(345).as_str(), "UTC+5:45");
    assert_eq!(utc_offset_label(-480).as_str(), "UTC−8");
}

fn format_wall_clock(unix_seconds: i64, offset_minutes: i32) -> String {
    let Some(time) = chrono::DateTime::from_timestamp(unix_seconds, 0) else {
        return String::new();
    };
    let Some(offset) = chrono::FixedOffset::east_opt(offset_minutes * 60) else {
        return String::new();
    };
    time.with_timezone(&offset)
        .format("%Y/%m/%d  %H : %M : %S")
        .to_string()
}

fn show_page(window: &PomodoroWindow, page: i32) {
    window.set_page(page);
    apply_main_size(window, window.window().scale_factor());
}

fn sync_main_window(window: &PomodoroWindow, application: &Application, feedback: &str) {
    let timer = &application.timer;
    window.set_show_wall_clock(timer.settings.show_wall_clock);
    if timer.settings.show_wall_clock {
        let utc = chrono::Utc::now().timestamp();
        let text = format_wall_clock(utc, timer.settings.wall_clock_offset_minutes);
        if window.get_wall_clock_label().as_str() != text {
            window.set_wall_clock_label(text.into());
        }
    }
    if window.get_keep_on_top() != timer.settings.topmost {
        window.set_keep_on_top(timer.settings.topmost);
    }
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
            false,
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
        let old_height = window.get_target_height();
        sync_main_window(&window, &application.borrow(), &feedback.borrow());
        if window.get_target_height() != old_height {
            apply_main_size(&window, window.window().scale_factor());
        }
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

fn resize_main_window(window: &PomodoroWindow, monitor_width_px: u32, dpi: u32) {
    let width_px = physical_window_width(monitor_width_px as i32);
    window.set_target_width(logical_width(width_px, dpi));
    apply_main_size(window, if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 });
}

fn apply_main_size(window: &PomodoroWindow, scale: f32) {
    let width = (window.get_target_width() * scale).round().max(1.0) as u32;
    let height = (window.get_target_height() * scale).round().max(1.0) as u32;
    let size = slint::PhysicalSize::new(width, height);
    if window.window().size() != size {
        window.window().set_size(size);
    }
}

fn data_directory() -> Result<PathBuf, io::Error> {
    if let Some(path) = std::env::var_os("POMODORO_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }
    let executable = std::env::current_exe()?;
    let directory = executable.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "The executable directory is unavailable",
        )
    })?;
    Ok(directory.join("data"))
}

fn legacy_data_directory() -> Option<PathBuf> {
    if std::env::var_os("POMODORO_DATA_DIR").is_some() {
        return None;
    }
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(|path| PathBuf::from(path).join("Pomodoro"))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(test)]
mod progress_path_tests {
    use super::progress_path;

    #[test]
    fn empty_and_complete_progress_have_distinct_paths() {
        assert_eq!(progress_path(0.0, false), "");
        assert_eq!(progress_path(-0.1, false), "");
        assert_eq!(
            progress_path(1.0, false),
            "M 26 2 A 24 24 0 1 1 26 50 A 24 24 0 1 1 26 2"
        );
        assert_eq!(
            progress_path(1.1, true),
            "M 22 2 A 20 20 0 1 1 22 42 A 20 20 0 1 1 22 2"
        );
    }

    #[test]
    fn small_remaining_progress_keeps_its_original_circle_geometry() {
        assert_eq!(
            progress_path(93.0 / 1500.0, false),
            "M 26 2 A 24 24 0 0 1 35.115 3.798"
        );
        assert_eq!(
            progress_path(93.0 / 1500.0, true),
            "M 22 2 A 20 20 0 0 1 29.596 3.498"
        );
    }

    #[test]
    fn arc_flag_changes_only_after_halfway() {
        assert!(progress_path(0.5, false).contains(" 0 0 1 "));
        assert!(progress_path(0.500_001, false).contains(" 0 1 1 "));
        assert!(progress_path(0.999_999, false).contains(" 0 1 1 "));
    }
}
