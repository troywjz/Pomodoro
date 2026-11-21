# Pomodoro

A compact, frameless Pomodoro timer for Windows, built with Rust and Slint.

[English](README.md) | [简体中文](README.zh-CN.md)

[Download Pomodoro v0.2.1 for Windows x64](https://github.com/troywjz/Pomodoro/releases/tag/v0.2.1)

## Features

- Focus, short-break, and long-break timers with pause, resume, and recovery.
- A compact window that follows the Windows light or dark theme and can stay on top.
- Local append-only JSONL storage for sessions, timer events, checkpoints, and settings history.
- Automatic JSONL backups and JSON export for later analysis.

## Install and run

1. Download and extract `Pomodoro-v0.2.1-windows-x64.zip` from the [v0.2.1 release](https://github.com/troywjz/Pomodoro/releases/tag/v0.2.1).
2. Double-click `Pomodoro.exe`.
3. If Windows reports that `VCRUNTIME140.dll` or `MSVCP140.dll` is missing, install Microsoft's [Visual C++ x64 Redistributable](https://aka.ms/vc14/vc_redist.x64.exe), then reopen Pomodoro.

The timer opens in a frameless window. Use the icon inside the ring to start, pause, resume, or advance the timer. The chevron stays in place when you expand or collapse the window. Drag the time display to move the window. A restored timer stays paused while you wait to resume or discard it. **Settings** opens in the timer window and contains the **About** page. Use **Save** or **Cancel** to leave Settings. **Exit** or Alt+F4 saves the current timer state before closing.

## Data and backups

The JSONL-capable build stores data next to `Pomodoro.exe`:

```text
data\
├── journal.jsonl
└── backups\
    └── verified JSONL snapshots
```

Each operation appends a JSON record to `journal.jsonl`; the app replays the log on startup. It retains completed, interrupted, and skipped sessions, active timer checkpoints, timer events, time-zone offsets, and every saved settings revision. Automatic backups run when existing data opens, after a session ends, and every 24 hours when data has changed. The app does not automatically delete history or valid backups.

On its first launch, a JSONL-capable build imports v0.1 data from `%LOCALAPPDATA%\Pomodoro`. After the imported journal is validated, the old SQLite database, its sidecar files, and its backups are preserved under `data\legacy-import`.

The v0.2.0 release uses JSONL storage. When upgrading from v0.1.0, the first launch imports the old data and keeps a verified archive under `data\legacy-import`.

## Export data for analysis

Open PowerShell in the extracted application folder and run:

```powershell
.\Pomodoro.exe --export-json "$env:USERPROFILE\Documents\pomodoro-analysis.json"
```

The JSON export contains a consistent snapshot of timer state, current and previous settings, sessions, and events. Running the export command on existing data follows the normal startup-backup behavior. CSV export and importing an export are not included in v0.2.0.

## Build from source

Install Rust 1.100.0 with the MSVC target and the Visual Studio C++ Build Tools, then run:

```powershell
cargo run --locked
```

To run the test suite and build a release executable:

```powershell
cargo test --locked --all-targets
cargo build --release --locked
```

Set `POMODORO_DATA_DIR` to use a separate data directory for development or testing.

## License

Pomodoro source code is licensed under the MIT License. Slint 1.19 is used under its Royalty-Free Desktop, Mobile, and Web Applications License 2.0. The project provides the required Slint attribution on its [website](https://github.com/troywjz/Pomodoro). Dependency license expressions and included license texts are listed in the release package's `THIRD-PARTY-NOTICES.txt`.

## Validation

The v0.2.0 Windows MSVC build passed the 5 storage tests and `cargo check --locked`. The v0.1.0 Windows manual checks covered keyboard input, settings, always-on-top behavior, close-time saving, restart recovery, automatic backups, and JSON export. Real sleep and hibernate, Hyper-V, multiple-monitor DPI changes, screen-reader narration, and release performance targets remain follow-up validation areas.
