# Pomodoro Windows 架构提案

> 技术候选仍待用户批准；本轮组织责任已按[模块边界](architecture/modules.md)重整，跨角色待决项统一在[评审包](contracts/review-packet.md)处理。

> 实现更新（2026-11-20）：用户已指定活动数据存为程序旁的追加式 JSONL，v0.2.0 已发布。SQLite 数据库只用于从 v0.1 版本迁移，细节见[存储交付记录](plans/deliveries/automatic-backup.md)。本文余下 Windows、托盘、DPI 与资源章节保留候选/验证状态。

**状态：待审批（技术研究草案）**
**关联：** [GitHub Issue #2](https://github.com/troywjz/Pomodoro/issues/2)
**目标环境：** Windows 11 x64；主机检测到 MSVC，当前 Rust 未在 PATH 中。`HypervisorPresent=True` 记录为环境线索，Hyper-V 场景仍需实际验收。

## 目标与约束

- Rust + Slint Windows 桌面番茄钟，支持常规交互桌面中的悬浮置顶、托盘、可靠计时及本地历史。
- MIT 为仓库现有许可，保留根目录 `LICENSE`。Slint 许可候选为 Royalty-free Desktop, Mobile, and Web Applications License 2.0；其分发条件要求应用 About 中展示 `AboutSlint` widget，或在公开网页展示 Slint attribution badge。该路线待用户审批，且需核对锁定版本和其他依赖的许可兼容性。
- 本文所有方案属于提案。批准前不进入业务代码、Cargo 工程或实现型试验。

## 建议分层

| 层 | 职责 | 提案 |
| --- | --- | --- |
| UI | 倒计时显示、交互、设置 | Slint + Windows `winit` backend；renderer 在 Skia 与 software 之间选择，版本和 feature 待审批及实测 |
| 应用状态 | 专注/短休/长休、开始/暂停/恢复/结束、恢复提示 | 独立于 UI 的状态模型，状态转换后写持久化层 |
| 计时 | 单调时间差、睡眠策略和恢复策略 | 睡眠时显式暂停；唤醒后询问用户是否恢复；崩溃重启后询问是否恢复；唤醒或重启不自动判定完成 |
| Windows 适配 | 电源消息、topmost、DPI、托盘和窗口定位 | 通过窄接口隔离平台 API，消息投递至 UI 线程 |
| 本地存储 | 设置、活动会话和完成历史 | `data\journal.jsonl` 追加式日志、`data\backups` 验证备份；v0.1 SQLite 只读迁移 |

## 计时语义

Rust `Instant` 文档说明它单调不减，同时注明系统 suspend 是否计入 elapsed time 取决于平台和 Rust 版本。Windows `QueryUnbiasedInterruptTime` 明确排除睡眠与休眠时间；其 Precise 变体列在 Microsoft 同一 API 家族中。`WM_POWERBROADCAST` 可提供 suspend/resume 通知。

因此，提案采用显式睡眠暂停：收到 suspend 通知时记录暂停；恢复通知后显示确认恢复。对未收到 suspend 通知的崩溃恢复，提供明确的“恢复 / 放弃”选择，不自动补算完成。锁屏期间继续计时。墙钟用于会话记录、恢复提示和检测系统时间调整；检测到改时后不据此自动扣减当前倒计时。Windows 高精度无偏时间可作为单调 elapsed time 候选，与 `Instant` 对照评估。

需注意，Windows 文档分别描述消息和时间 API；消息可能受睡眠流程影响，实际通知覆盖需在目标 Windows 和 Hyper-V 环境验证。

## Slint、窗口与托盘

Slint 官方文档提供 Windows `winit` backend 和 renderer 选择说明。建议先以该 backend 为候选。Skia 或 software renderer 的依赖、启动速度、GPU 兼容、透明窗体和资源消耗需实测后决定，版本锁定等待审批。

Slint 当前 `SystemTrayIcon` API 文档描述 Windows 使用 `Shell_NotifyIcon`，可设置图标、tooltip、菜单及点击回调。Rust API 的 `system-tray` feature 默认启用；Slint 1.18.1 的 `run_event_loop` 文档说明可见托盘图标维持 event loop。建议先评估原生 Slint API，只有验收发现功能缺口时再考虑独立托盘库。依赖版本及 feature 锁定待审批。后台工作通过 Slint `invoke_from_event_loop` 回到 UI 线程，避免跨线程直接操作组件。

Win32 `SetWindowPos(HWND_TOPMOST)` 使窗体位于非 topmost 窗口之上且失焦后维持该层级。产品承诺范围建议限定为常规交互桌面中的普通 topmost。Windows desktop 隔离、UAC/Winlogon 安全桌面、独占全屏和 VMConnect 来宾桌面单独测试并明确可见性范围。置顶与激活分开处理，避免启动或定时事件抢焦点。

## DPI 与多屏

建议评估 Per-Monitor V2 DPI awareness。Windows 会在 DPI 变化时发送 `WM_DPICHANGED` 和建议矩形；位置和尺寸需根据新 DPI 更新。窗口位置按逻辑坐标保存，显示器消失或坐标越界时回退至主屏工作区。托盘图标按高 DPI 准备尺寸。跨屏、缩放变化、热插拔、任务栏位置及隐藏窗口恢复均列入验收。

## 存储选择

当前实现将每个持久操作保存为一条 JSONL commit frame，文件锁串行化写入，启动时按顺序重放。完整历史与备份默认持续保留。首次从 v0.1 升级时只读导入 SQLite 数据，校验日志后保留旧库及备份归档。详细契约见[存储接口](contracts/m1-storage-port.md)。

## 资源目标（尚未测量）

统一目标：空闲 CPU ≤ 0.5%，运行 CPU ≤ 1%，冷启动 ≤ 1 秒，安装后磁盘占用 ≤ 30 MiB，常驻内存 ≤ 60 MiB。全部为目标值，需在 Release 构建和约定环境中测量。测试记录硬件、Windows 版本、renderer、构建配置、测量方法和结果。界面倒计时建议每秒更新一次，降低无效刷新。

## 验收矩阵

下表是系统场景汇总；完整步骤与预期统一以[验收矩阵](acceptance.md)为准，覆盖全部 AC。接口、错误、迁移及配置校验以[模块契约](interface-contract.md)为准。

| 类别 | 验收场景 |
| --- | --- |
| 环境 | Windows 11 x64；目标 MSVC/Rust/Slint 版本确定后记录完整版本 |
| 窗口 | 常规桌面 topmost、失焦、隐藏/显示、启动恢复、焦点保持 |
| 桌面边界 | UAC 提示、安全桌面、独占全屏、VMConnect 窗口与全屏；注明进程运行于宿主或来宾 |
| 托盘 | 左键显示/隐藏、菜单操作、退出清理、Explorer 重启后图标恢复 |
| 计时 | 锁屏继续、睡眠前暂停、唤醒确认、改系统时间、崩溃重启确认、暂停/恢复/重置 |
| 显示 | 单屏、多屏混合 DPI、跨屏、DPI 调整、热插拔、主屏变化、边缘与任务栏 |
| 持久化 | 状态转换写入、异常退出、损坏数据库、迁移、磁盘写入失败、历史增长 |
| 性能 | 空闲与运行 CPU、内存、冷启动和安装目录体积，对照本节目标记录 |
| 数据管理 | JSON/CSV导出、导出与备份失败、清空历史、永久清除、跨日及跨时区统计 |
| 交互与辅助技术 | 键盘、焦点、屏幕阅读器、离线操作、字段校验 |
| 恢复与迁移 | 周期快照、版本升级回滚、更新版本库拒写、损坏库保留与备份恢复 |

## 官方资料

- Slint [Winit backend](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/)
- Slint [Backends & Renderers](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/)
- Slint Rust [SystemTrayIcon](https://docs.slint.dev/latest/docs/slint/reference/window/systemtrayicon/)
- Slint Rust [`invoke_from_event_loop`](https://docs.slint.dev/latest/docs/rust/slint/fn.invoke_from_event_loop)
- Slint Rust [`run_event_loop`](https://docs.slint.dev/latest/docs/rust/slint/fn.run_event_loop)
- Slint Rust [Cargo features](https://docs.slint.dev/latest/docs/rust/slint/docs/cargo_features/)（含默认启用的 `system-tray`）
- Slint [Royalty-free License 2.0](https://slint.dev/agreements/slint-royalty-free-license.pdf)
- Rust [`std::time::Instant`](https://doc.rust-lang.org/stable/std/time/struct.Instant.html)
- Microsoft [QueryUnbiasedInterruptTime](https://learn.microsoft.com/en-us/windows/win32/api/realtimeapiset/nf-realtimeapiset-queryunbiasedinterrupttime)
- Microsoft [WM_POWERBROADCAST messages](https://learn.microsoft.com/en-us/windows/win32/power/wm-powerbroadcast-messages)
- Microsoft [PBT_APMRESUMEAUTOMATIC](https://learn.microsoft.com/en-us/windows/win32/power/pbt-apmresumeautomatic)
- Microsoft [SetWindowPos](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos)
- Microsoft [Extended Window Styles](https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles)
- Microsoft [Desktops](https://learn.microsoft.com/en-us/windows/win32/winstation/desktops)
- Microsoft [Hyper-V VMConnect](https://learn.microsoft.com/en-us/windows-server/virtualization/hyper-v/virtual-machine-connection)
- Microsoft [Per-Monitor DPI development](https://learn.microsoft.com/en-gb/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows)
- Microsoft [WM_DPICHANGED](https://learn.microsoft.com/en-au/windows/win32/hidpi/wm-dpichanged)
- Microsoft [Notification Area](https://learn.microsoft.com/en-us/windows/win32/shell/notification-area)
- SQLite [Write-Ahead Logging](https://www.sqlite.org/wal.html)
