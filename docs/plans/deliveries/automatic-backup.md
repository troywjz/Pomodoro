# M1 本地数据与自动备份交付

## 范围

- 起始提交：`8b4a1ca`。
- 执行分支：`task/jsonl-append-store`。
- 实现 crate：`crates/pomodoro-storage`，包名 `pomodoro-storage`。
- 公开接口：`docs/contracts/m1-storage-port.md` 的 M1 Storage Port v1；启动备份结果读取器已补入接口。
- 变更边界：storage crate、应用启动路径接入、本交付记录和接口勘误。

## 实现内容

- JSONL 持久化活动 checkpoint、设置快照、终结 session 历史、分析事件、revision 与幂等 command receipt。每次业务操作追加一个含 request delta 和 receipt 的 JSON frame。
- `commit` 在文件锁内重读最新日志并完成 global/session/settings CAS、业务变更、revision、事件和回执校验，再追加一个 frame；同 ID 同 hash 回放首次回执，同 ID 异 hash 返回冲突。终结历史与调用方提供的最终 checkpoint 在同一个 frame 中提交；终结请求没有新 checkpoint 时清除旧活动 checkpoint。
- 已有有效数据打开时生成启动备份；终结 session 成功提交后生成备份；运行期间由 `maybe_auto_backup` 执行 24 小时且有变更的备份检查。备份复制完整 JSONL 并回放验证后发布到 `backups`。自动备份不会清理旧备份或历史行。JSON 导出包含 state、全部设置版本、session 历史和事件，并以临时文件原子落盘。
- v0.1 SQLite只用于一次性迁移：执行完整性检查并导入状态、设置历史、session、事件和receipt，回放验证JSONL后归档旧库、sidecar及SQLite备份到`legacy-import`。`Store::open_with_legacy`接收新旧数据目录，应用层可将活动数据放在程序目录旁，同时从旧默认目录迁入。

## 技术依赖

- `fs2 0.4.3`提供跨进程journal文件锁；`rusqlite 0.43.0`仅用于只读一次性迁移。
- JSON序列化使用`serde`与`serde_json`；测试使用`tempfile`。

## 验证记录

- 使用项目提供的 Rust 1.100.0 Windows MSVC 工具链运行 crate 测试。
- `cargo fmt` 完成。
- `cargo test --manifest-path crates/pomodoro-storage/Cargo.toml`：5 项通过，0 项失败。
- 测试覆盖append/restart、幂等回执、revision冲突、部分尾帧恢复、JSON导出与SQLite迁移历史/配置版本/events/receipts及旧备份归档。

## 交接边界

- 下游基线应读取 `open_backup_status()`，向界面呈现启动备份结果；之后按周期调用 `maybe_auto_backup(now_utc_ms)`。
- `session_record` 代表终结记录；同请求可携带最终 checkpoint，二者写入同一 commit frame。终结请求未带 checkpoint 时才清除旧活动 checkpoint。涉及活动快照或终结记录的提交需携带 `expected_session_revision`；设置更新需携带 `expected_settings_revision`。
- schema 当前版本为 1。统计口径、outcome 含义及清理操作仍由产品/数据管理任务定义；本 crate 不提供历史或备份自动删除入口。
