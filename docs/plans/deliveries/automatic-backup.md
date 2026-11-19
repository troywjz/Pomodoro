# M1 本地数据与自动备份交付

## 范围

- 起始提交：`c5a3c6bfb1e19d541575516eaa7d8068c4a52c90`。
- 执行分支：`task/automatic-backup`。
- 实现 crate：`crates/pomodoro-storage`，包名 `pomodoro-storage`。
- 公开接口：`docs/contracts/m1-storage-port.md` 的 M1 Storage Port v1；启动备份结果读取器已补入接口。
- 变更边界：只写入 storage crate、本交付记录和接口勘误。

## 实现内容

- SQLite 持久化活动 checkpoint、设置快照、终结 session 历史、分析事件、revision 与 `command_receipts`。
- `commit` 使用单一 `IMMEDIATE` 事务完成 global/session/settings CAS、业务行、revision、事件和回执；同 ID 同 hash 回放首次回执，同 ID 异 hash 返回冲突。终结历史与调用方提供的最终 checkpoint 一起提交；最终 checkpoint 为空时清除旧活动 checkpoint。
- 已有有效库打开时生成启动备份；终结 session 成功提交后生成备份；运行期间由 `maybe_auto_backup` 执行 24 小时且有变更的备份检查。SQLite 在线备份完成后执行 `integrity_check`、schema 与表可读检查，再用不覆盖已存在文件的方式发布到 `backups`。备份失败保留成功业务回执，并由 `BackupStatus::Failed` 提供错误文本及重试路径。
- 自动备份不会清理旧备份或历史行。JSON 导出在一致性读事务中包含 state、全部设置版本、session 历史和事件，并以临时文件完成原子落盘；写入前拒绝规范化后落在主库、WAL/SHM/journal sidecar 和应用 `backups` 子树的目的地。

## 技术依赖

- `rusqlite 0.43.0` 启用 `backup` 与 `bundled`，使用在线备份 API 和随 crate 构建的 SQLite；备份句柄需独立源/目标连接并通过 `run_to_completion` 完成。[rusqlite backup 文档](https://docs.rs/rusqlite/0.43.0/rusqlite/backup/index.html)、[SQLite Online Backup API](https://sqlite.org/backup.html)
- JSON 序列化使用 `serde 1.0` 与 `serde_json 1.0.154`；测试使用 `tempfile 3.27.0`。

## 验证记录

- 使用项目提供的 Rust 1.100.0 Windows MSVC 工具链运行 crate 测试；未安装第二套工具链。
- `cargo fmt` 完成。
- `cargo test --manifest-path crates/pomodoro-storage/Cargo.toml -- --nocapture`：8 项通过，0 项失败。
- 测试覆盖启动/终结自动备份、SQLite `integrity_check` 与行数读取、幂等回执、异 payload 冲突、陈旧 revision、终结历史与最终快照原子提交/重启读回/失败回滚、多个 session 与事件的 UTC 偏移、多版配置快照保留、导出路径保护及文件字节不变、备份目录故障、失败后的显式重试、文件名冲突与旧有效备份保留。

## 交接边界

- 下游基线应读取 `open_backup_status()`，向界面呈现启动备份结果；之后按周期调用 `maybe_auto_backup(now_utc_ms)`。
- `session_record` 代表终结记录；同请求可携带最终 checkpoint，二者同事务写入。终结请求未带 checkpoint 时才清除旧活动 checkpoint。涉及活动快照或终结记录的提交需携带 `expected_session_revision`；设置更新需携带 `expected_settings_revision`。
- schema 当前版本为 1。统计口径、outcome 含义及清理操作仍由产品/数据管理任务定义；本 crate 不提供历史或备份自动删除入口。
