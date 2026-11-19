# M1存储接口 v1

目的：为基线应用与自动备份任务提供同一编译接口；具体字段所有者为存储任务，根Cargo/构建入口唯一写入者为M1基线任务。本接口细化已批准M0事务、回执和数据保留要求。

存储独立crate位于`crates/pomodoro-storage`，包名`pomodoro-storage`，Rust导入名`pomodoro_storage`。基线任务在拿到已测试提交后接入，不创建第二套存储实现。

## 最小公开API

- `Store::open(data_dir: &std::path::Path) -> Result<Store, StoreError>`：主库`pomodoro.sqlite3`，应用内`backups`目录；保留已存在主库。首次建空库不生成空备份，已有有效库启动时自动备份。
- `Store::open_backup_status(&self) -> &BackupStatus`：读取`open`期间对已有有效库执行的自动备份结果。新建空库返回`NotDue`；启动备份失败保留`Store`和主数据，并通过此方法返回可读错误，供界面重试。
- `Store::load_state(&self) -> Result<StoredState, StoreError>`：返回`global_revision: u64`、`session_revision: u64`、`settings_revision: u64`、`checkpoint_json: Option<String>`、`settings_json: Option<String>`。
- `Store::commit(&mut self, request: &CommitRequest) -> Result<CommitOutcome, StoreError>`：原子提交业务变更、事件、revision及命令回执。重复command_id+payload_hash返回原回执，不重复追加记录；不同hash返回冲突。
- `Store::maybe_auto_backup(&mut self, now_utc_ms: i64) -> BackupStatus`：应用定期调用，距离上次成功备份达到24小时且已有变更时自动执行；默认保留所有有效备份。
- `Store::export_json(&self, destination: &Path) -> Result<(), StoreError>`：一致性导出状态、配置、历史及事件，保留源数据。

`CommitRequest`字段：`command_id: String`、`payload_hash: String`、`expected_global_revision: u64`、`expected_session_revision: Option<u64>`、`expected_settings_revision: Option<u64>`、`checkpoint_json: Option<String>`、`settings_json: Option<String>`、`session_record: Option<SessionRecord>`、`event: Option<EventRecord>`、`committed_utc_ms: i64`。

`SessionRecord`字段：`session_id: String`、`phase: String`、`target_ms: u64`、`actual_active_ms: u64`、`started_utc_ms: i64`、`ended_utc_ms: i64`、`start_local_date: String`、`start_offset_minutes: i32`、`outcome: String`、`reason: Option<String>`。

`EventRecord`字段：`event_id: String`、`session_id: Option<String>`、`kind: String`、`occurred_utc_ms: i64`、`local_offset_minutes: i32`、`payload_json: String`。事件用于保留开始、暂停、恢复、完成/中断等分析信息，payload保存本次动作的参数/上下文。

`CommitOutcome`包含`receipt: Receipt`及`backup_status: BackupStatus`；`Receipt`字段为`command_id: String`、`global_revision: u64`、`session_revision: u64`、`settings_revision: u64`、`committed_utc_ms: i64`、`duplicate: bool`。`BackupStatus`至少区分`NotDue`、`Created`、`Failed`，并支持界面呈现可重试错误。

接口澄清：`open`需要返回`Store`以便在启动备份失败时继续使用并保留主数据，原签名没有承载`BackupStatus`的返回槽位，因此增加`open_backup_status`只读访问器；这是启动结果的公开读取入口。重复命令回执的`backup_status`为`NotDue`，重试不重复触发副作用。涉及活动快照/终结记录时必须提供`expected_session_revision`，修改配置时必须提供`expected_settings_revision`。提交终结`session_record`会在同一事务清除活动checkpoint；同一请求同时提供checkpoint和终结记录属于无效请求。

配置保留：每次成功写入`settings_json`均持久保留对应`settings_revision`、完整配置JSON和UTC更新时间；`export_json`输出全部配置版本及当前配置，不按时间或数量清理。

备份失败发生在成功业务提交之后时，业务提交仍返回成功回执，backup_status显式报告失败。备份使用SQLite一致性备份API、完整性校验和成功标记；主库和历史保留。清历史保留活动/配置/回执与应用备份；永久清除的完整流程留在对应数据管理任务，M1基线不提供简化删除入口。

## 必须执行的验证

1. 不手动调用备份入口，启动已有数据库和提交终结session后自动生成可打开、integrity_check通过且保留预期行的备份。
2. 同一command_id重试、同ID异payload、陈旧revision、事务失败与重启读回。
3. 保留多个终结记录、暂停/恢复事件、UTC/偏移及配置快照；导出可供离线分析且不删除历史。
4. 模拟备份目录故障，主数据提交/保留与backup_status的结果符合上述边界。
