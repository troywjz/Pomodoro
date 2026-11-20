# 番茄钟

一款使用 Rust 和 Slint 构建的 Windows 无边框悬浮番茄钟。

[English](README.md) | [简体中文](README.zh-CN.md)

[下载 Pomodoro v0.1.0 Windows x64 版本](https://github.com/troywjz/Pomodoro/releases/tag/v0.1.0)

## 功能

- 支持专注、短休、长休，以及暂停、继续和重启恢复。
- 紧凑的无边框窗口，跟随 Windows 浅色或深色主题，并支持窗口置顶。
- 使用本地 SQLite 保存会话、计时事件、恢复检查点和设置历史。
- 自动验证式备份，并支持导出 JSON 供后续分析。

## 安装与运行

1. 从[最新 Release](https://github.com/troywjz/Pomodoro/releases/tag/v0.1.0)下载并解压 `Pomodoro-v0.1.0-windows-x64.zip`。
2. 双击 `Pomodoro.exe`。
3. 如果 Windows 提示缺少 `VCRUNTIME140.dll` 或 `MSVCP140.dll`，请安装微软的 [Visual C++ x64 运行库](https://aka.ms/vc14/vc_redist.x64.exe)，再重新打开程序。

窗口不显示 Windows 标题栏。拖动阶段和轮次标题区域可以移动窗口。在「设置」中调整计时时长、长休间隔或「窗口置顶」。点击「保存」应用设置，点击「取消」放弃更改。点击「退出」或按 Alt+F4 会在关闭前保存当前计时。下次启动时，可以选择继续、保持暂停或放弃当前会话。

## 数据与自动备份

Pomodoro 将数据保存在：

```text
%LOCALAPPDATA%\Pomodoro
├── pomodoro.sqlite3
└── backups\
    └── 已验证的 SQLite 快照
```

数据库会保留已完成、中断和跳过的会话、活动计时检查点、计时事件、时区偏移和每个版本的设置。打开已有数据库、结束一个会话后，以及数据发生变化且距离上次成功备份达到 24 小时时，程序会自动创建并验证备份。在你移除应用数据前，历史记录和有效备份都会保留。

## 导出数据供分析

在解压目录中打开 PowerShell，运行：

```powershell
.\Pomodoro.exe --export-json "$env:USERPROFILE\Documents\pomodoro-analysis.json"
```

JSON 导出包含一致性读取到的计时状态、当前和历史设置、会话及事件。对已有数据库运行导出命令时，也会执行正常的启动备份流程。v0.1.0 尚未提供 CSV 导出或导入功能。

## 从源码构建

安装 Rust 1.100.0 MSVC 工具链和 Visual Studio C++ Build Tools，然后运行：

```powershell
cargo run --locked
```

运行测试并构建 Release 可执行文件：

```powershell
cargo test --locked --all-targets
cargo build --release --locked
```

开发或测试时，可设置 `POMODORO_DATA_DIR` 将数据保存到独立目录。

## 许可证

Pomodoro 源码采用 MIT 许可证。Slint 1.19 使用 Royalty-Free Desktop, Mobile, and Web Applications License 2.0。项目在[网站](https://github.com/troywjz/Pomodoro)提供 Slint 所需署名。依赖许可证标识及随包附带的许可证文本见 Release 中的 `THIRD-PARTY-NOTICES.txt`。

## 验证范围

v0.1.0 的 Windows MSVC 调试构建通过 18 项应用测试和 8 项独立存储测试。Windows 手动检查覆盖键盘操作、设置、窗口置顶、关闭前保存、重启恢复、自动备份和 JSON 导出。真实睡眠与休眠、Hyper-V、多显示器 DPI、屏幕阅读器播报及 Release 资源指标留待后续验证。
