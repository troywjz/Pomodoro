# 架构决策记录

**状态：** 本页保留架构候选的审批历史。ADR-006 已由用户于 2026-11-20 更新为 v0.2.0 JSONL 实现；其他候选状态仍以对应审批与项目状态记录为准。

| ID | 主题 | 候选决策 | 状态 / 待确认 |
| --- | --- | --- | --- |
| ADR-001 | 仓库许可 | 保留仓库现有 MIT `LICENSE`；Slint 候选采用 Royalty-free Desktop, Mobile, and Web Applications License 2.0，履行 `AboutSlint` widget 或公开网页 attribution badge 条件 | 待审批；核对目标 Slint 版本和其他依赖许可 |
| ADR-002 | UI backend | Slint `winit` Windows backend | 待审批；依赖版本锁定待批准 |
| ADR-003 | Renderer | Skia 或 software renderer；统一资源目标为空闲 CPU ≤ 0.5%、运行 CPU ≤ 1%、启动 ≤ 1 秒、磁盘 ≤ 30 MiB、内存 ≤ 60 MiB | 待审批、待实测；使用 Release 构建验证透明窗体和兼容性 |
| ADR-004 | 托盘 | 优先评估 Slint 原生 `SystemTrayIcon` 和同一 event loop | 待审批；Rust `system-tray` feature 默认启用，当前官方 API 文档含 Windows Shell notification area 支持；版本待锁定 |
| ADR-005 | 休眠计时 | 睡眠时显式暂停；唤醒后请求用户确认恢复；崩溃后请求用户确认恢复；锁屏继续；改墙钟时不自动扣减 | 待审批；评估 `QueryUnbiasedInterruptTime(Precise)`，结合电源通知验证 |
| ADR-006 | 持久化 | 活动数据采用程序旁 `data\journal.jsonl` 追加式 JSONL；每次操作追加记录，自动备份也为 JSONL。v0.1 SQLite 仅用于一次性只读导入 | 已按用户 2026-11-20 指示实现并随 v0.2.0 发布；旧数据归档于 `data\legacy-import` |
| ADR-007 | DPI 与定位 | 评估 Per-Monitor V2；位置按逻辑坐标保存，显示变化后修正可见范围 | 待审批、待多屏实测 |
| ADR-008 | 置顶范围 | 常规桌面的 `HWND_TOPMOST` 行为为目标；安全桌面、独占全屏和 VMConnect 边界需场景验收 | 待审批；定义产品文案和验收范围 |

## 关键依据

- Rust `Instant` 文档将 suspend 是否计入经过时间列为平台和版本相关行为：[Rust Instant](https://doc.rust-lang.org/stable/std/time/struct.Instant.html)。
- Windows `QueryUnbiasedInterruptTime` 计数排除睡眠和休眠，系统时间服务调整不影响该计数：[Microsoft API 文档](https://learn.microsoft.com/en-us/windows/win32/api/realtimeapiset/nf-realtimeapiset-queryunbiasedinterrupttime)。
- Windows 电源管理消息报告 suspend 与 resume 事件：[Microsoft WM_POWERBROADCAST 文档](https://learn.microsoft.com/en-us/windows/win32/power/wm-powerbroadcast-messages)。
- Slint `SystemTrayIcon` 官方 API 说明 Windows 使用 `Shell_NotifyIcon`，并给出 Rust event loop 示例：[Slint API 文档](https://docs.slint.dev/latest/docs/slint/reference/window/systemtrayicon/)。
- Slint Rust feature 文档列明默认启用的 `system-tray`：[Cargo features](https://docs.slint.dev/latest/docs/rust/slint/docs/cargo_features/)；Slint 1.18.1 的 [run_event_loop 文档](https://docs.slint.dev/latest/docs/rust/slint/fn.run_event_loop)说明可见托盘图标会维持 event loop。
- Royalty-free 2.0 许可文本规定 About widget 或公开网页 badge 归属展示条件：[Slint Royalty-free License 2.0](https://slint.dev/agreements/slint-royalty-free-license.pdf)。
- SQLite WAL 文档描述事务提交与 `synchronous` 的耐久性权衡：[SQLite WAL 文档](https://www.sqlite.org/wal.html)。
- 仓库 MIT 许可全文以根目录现有 [`LICENSE`](../LICENSE) 为准；其变更需要单独决策。
