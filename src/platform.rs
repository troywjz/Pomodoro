use std::time::Instant;

use chrono::{Local, Utc};

use crate::domain::ClockSample;

#[cfg(windows)]
pub fn move_window_with_pointer(window: &slint::Window, offset_x: i32, offset_y: i32) {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

    // Both coordinates are in desktop pixels, even after the window crosses a DPI boundary.
    let mut cursor = POINT::default();
    if unsafe { GetCursorPos(&mut cursor) } != 0 {
        let position = slint::PhysicalPosition::new(cursor.x - offset_x, cursor.y - offset_y);
        if window.position() != position {
            window.set_position(position);
        }
    }
}

#[cfg(windows)]
pub fn clock_sample(_origin: Instant) -> ClockSample {
    use std::sync::OnceLock;

    use windows_sys::Win32::System::SystemInformation::GetTickCount64;
    use windows_sys::Win32::System::WindowsProgramming::QueryUnbiasedInterruptTimePrecise;

    // These counters share the Windows boot origin. Keep a process-local baseline so
    // domain samples start near the caller's Instant origin without including sleep.
    static ORIGIN: OnceLock<(u64, u64)> = OnceLock::new();
    let mut active_100ns = 0_u64;
    unsafe { QueryUnbiasedInterruptTimePrecise(&mut active_100ns) };
    let boot_ms = unsafe { GetTickCount64() };
    let (active_origin, boot_origin) = *ORIGIN.get_or_init(|| (active_100ns, boot_ms));
    let local = Local::now();
    ClockSample {
        monotonic_ms: active_100ns.saturating_sub(active_origin) / 10_000,
        boot_elapsed_ms: boot_ms.saturating_sub(boot_origin),
        utc_ms: Utc::now().timestamp_millis(),
        local_date: local.format("%Y-%m-%d").to_string(),
        local_offset_minutes: local.offset().local_minus_utc() / 60,
    }
}

#[cfg(not(windows))]
pub fn clock_sample(origin: Instant) -> ClockSample {
    let local = Local::now();
    let elapsed = origin.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    ClockSample {
        monotonic_ms: elapsed,
        boot_elapsed_ms: elapsed,
        utc_ms: Utc::now().timestamp_millis(),
        local_date: local.format("%Y-%m-%d").to_string(),
        local_offset_minutes: local.offset().local_minus_utc() / 60,
    }
}

pub fn physical_window_width(monitor_width_px: i32) -> u32 {
    ((f64::from(monitor_width_px) / 10.0) + 0.5)
        .floor()
        .max(1.0) as u32
}

pub fn logical_width(physical_width_px: u32, dpi: u32) -> f32 {
    let scale = if dpi == 0 { 96.0 } else { dpi as f32 } / 96.0;
    physical_width_px as f32 / scale
}

#[cfg(windows)]
pub fn system_prefers_dark_theme() -> bool {
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_DWORD, RRF_RT_REG_DWORD, RegGetValueW,
    };

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let value_name: Vec<u16> = "AppsUseLightTheme"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut value = 1_u32;
    let mut value_type = 0_u32;
    let mut byte_count = std::mem::size_of::<u32>() as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value_name.as_ptr(),
            RRF_RT_REG_DWORD,
            &mut value_type,
            (&mut value as *mut u32).cast(),
            &mut byte_count,
        )
    };
    result == 0 && value_type == REG_DWORD && value == 0
}

#[cfg(not(windows))]
pub fn system_prefers_dark_theme() -> bool {
    false
}

#[cfg(windows)]
pub fn monitor_metrics(window: &slint::Window) -> (u32, u32) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, MonitorFromWindow,
    };
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, GetWindowRect, SM_CXSCREEN,
    };

    let handle_owner = window.window_handle();
    let hwnd = handle_owner
        .window_handle()
        .ok()
        .and_then(|handle| match handle.as_raw() {
            RawWindowHandle::Win32(handle) => {
                Some(handle.hwnd.get() as windows_sys::Win32::Foundation::HWND)
            }
            _ => None,
        });

    let monitor = hwnd
        .and_then(|hwnd| {
            let mut rect = windows_sys::Win32::Foundation::RECT::default();
            let success = unsafe { GetWindowRect(hwnd, &mut rect) };
            if success == 0 {
                None
            } else {
                let center = POINT {
                    x: rect.left + (rect.right - rect.left) / 2,
                    y: rect.top + (rect.bottom - rect.top) / 2,
                };
                let monitor = unsafe { MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST) };
                (!monitor.is_null()).then_some((monitor, hwnd))
            }
        })
        .or_else(|| {
            let desktop =
                unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetDesktopWindow() };
            let monitor = unsafe { MonitorFromWindow(desktop, MONITOR_DEFAULTTONEAREST) };
            (!monitor.is_null()).then_some((monitor, desktop))
        });

    if let Some((monitor, hwnd)) = monitor {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
            let width = (info.rcMonitor.right - info.rcMonitor.left).max(1) as u32;
            let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
            return (width, dpi);
        }
    }

    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) }.max(1) as u32;
    (width, 96)
}

#[cfg(not(windows))]
pub fn monitor_metrics(_window: &slint::Window) -> (u32, u32) {
    (1920, 96)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_width_is_ten_percent_rounded_half_up() {
        assert_eq!(physical_window_width(1366), 137);
        assert_eq!(physical_window_width(1920), 192);
        assert_eq!(physical_window_width(2560), 256);
        assert_eq!(physical_window_width(3840), 384);
    }

    #[test]
    fn physical_width_converts_to_logical_width_at_dpi() {
        assert!((logical_width(256, 120) - 204.8).abs() < 0.001);
        assert!((logical_width(384, 144) - 256.0).abs() < 0.001);
    }

    #[cfg(not(windows))]
    #[test]
    fn fallback_clock_uses_milliseconds_and_advances() {
        let origin = Instant::now();
        let first = clock_sample(origin);
        std::thread::sleep(std::time::Duration::from_millis(3));
        let next = clock_sample(origin);
        assert!(next.monotonic_ms >= first.monotonic_ms);
        assert_eq!(next.boot_elapsed_ms, next.monotonic_ms);
    }
}
