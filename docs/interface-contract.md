# 模块接口与数据契约提案

> 候选版本 `M0-Contracts v1.0.1`，状态：待产品语义输入及用户发布/许可决策。本文是公共技术字段和时序的唯一候选字段来源；行为含义依赖产品与交互设计交付。业务实现与 Windows 实测尚未开始。

## 1. 约定与所有权

- 时间单位统一为整数毫秒；持久化 UTC 时间为 RFC 3339 UTC，原始本地偏移以分钟整数保存。运行时单调时钟只在当前启动周期内作差，不持久化其原点。
- ID 使用不透明、不可复用的 UUID；枚举采用带版本的稳定字符串。外部序列化必须包含 `schema_version`。
- `domain` 定义纯值类型和状态转移；`application` 唯一串行协调命令与副作用；`storage` 只负责原子提交和查询；Windows 平台采集系统事件；Slint 只消费快照并发出命令。
- 需求含义、统计口径、用户行为及验收定义由产品与交互设计组提出。本契约不冻结含产品语义的字段枚举值；标记 `PRODUCT_INPUT` 的项在输入到位前不得视为已定。

## 2. 唯一字段字典

| 类型 | 候选字段（唯一拼写） | 所有者 / 说明 |
|---|---|---|
| `ClockSample` | `active_elapsed_ms:u64`, `boot_elapsed_ms:u64`, `utc_ms:i64`, `local_offset_minutes:i16` | `active_elapsed_ms` 与 `boot_elapsed_ms` 是当前开机周期内分别从固定原点读取的累计单调值，非增量。前者来自应用选定的活动时间源；挂起/恢复确认时重置本 session 的差值基线。后者用于检测系统睡眠。UTC 与偏移用于记录。Windows API、采样精度和漏通知阈值待实测。 |
| `CommandEnvelope` | `command_id:Uuid`, `expected_session_revision:u64?`, `expected_settings_revision:u64?`, `payload:Command` | Application 协议。仅涉及对应实体时携带相应期望 revision；重试沿用同一 `command_id`。 |
| `TimerSnapshot` | `session_id:Uuid?`, `phase:Phase?`, `state:TimerState`, `target_ms:u64?`, `accumulated_active_ms:u64`, `remaining_ms:u64?`, `runtime_seq:u64`, `session_revision:u64`, `persistence_state:clean\|saving\|unsaved`, `last_checkpoint_utc_ms:i64?` | `runtime_seq` 是应用进程内每次可见运行态更新的单调序号；`session_revision` 是最后已提交 checkpoint 的持久 CAS 版本。UI/托盘读取同一快照。Idle 目标选择语义为 `PRODUCT_INPUT`。 |
| `SessionRecord` | `session_id`, `phase`, `target_ms`, `actual_active_ms`, `started_utc_ms`, `ended_utc_ms`, `start_local_date`, `start_offset_minutes`, `outcome`, `reason?` | Storage 持久记录；`outcome`、目标如何归属及各 outcome 的 `actual_active_ms` 算法均为 `PRODUCT_INPUT`。唯一约束 `session_id`。 |
| `Settings` | `schema_version:u32`, `settings_revision:u64`, `focus_ms:u64`, `short_break_ms:u64`, `long_break_ms:u64`, `long_break_interval:u8`, `topmost:bool`, `sound_enabled:bool`, `theme_mode:ThemeMode`, `window_rect:LogicalRect?`, `monitor_hint:String?` | Storage 持久配置。取值边界/默认值由已批准需求冻结；修改配置不修改活动快照。 |
| `LogicalRect` | `x_dip:f64`, `y_dip:f64`, `width_dip:f64`, `height_dip:f64` | Windows 平台提供并解释逻辑坐标。 |
| `ThemeSnapshot` | `mode:light\|dark`, `source:system\|fallback`, `revision:u64` | Windows 平台采集，UI 消费；系统查询失败采用已记录的 fallback。具体首次查询/订阅失败策略依产品/UI规格并待平台验证。 |
| `PersistRequest` | `command_id`, `payload_hash`, `expected_global_revision:u64`, `expected_session_revision:u64?`, `expected_settings_revision:u64?`, `checkpoint_payload:DurableSession?`, `session_append:SessionRecord?`, `settings_update:Settings?`, `result_payload` | Application → Storage。业务变更、revision、命令幂等回执在同一 SQLite 事务写入。`DurableSession` 不含 `runtime_seq`、`persistence_state`，这些仅为进程运行态/展示元数据。 |
| `PersistReceipt` | `command_id`, `payload_hash`, `global_revision:u64`, `session_revision:u64?`, `settings_revision:u64?`, `committed_utc_ms:i64`, `result_payload` | Storage 仅在事务提交成功后返回；失败返回结构化错误及当前持久 revision。重试从已存命令回执恢复原结果。 |
| `ExportEnvelope` | `schema_version:u32`, `exported_utc_ms:i64`, `settings:Settings?`, `sessions:SessionRecord[]` | 导出稳定容器骨架；是否包含设置/活动快照、CSV列及导出权限均为 `PRODUCT_INPUT`。导入不属于本轮候选能力。 |

`TimerSnapshot.session_revision` 在该活动 session 生命周期内按成功 checkpoint 单调递增；新 session revision 从 1 开始。`runtime_seq` 只在当前进程内递增，用于视图丢弃乱序更新，重启归零。`Settings.settings_revision` 仅在配置事务成功后递增。`global_revision` 每次成功持久化事务递增，数据库元数据中单一维护。revision 溢出返回不可恢复存储错误，不回绕。初始空库的 global/settings revision 为 0。

SQLite schema 包含 `command_receipts(command_id PRIMARY KEY, payload_hash, result_payload, global_revision, committed_utc_ms)`。`payload_hash` 是规范化命令体的 SHA-256。回执、命令业务变更、历史记录与 revision 同事务提交。查询到同 ID 且同 hash 时返回原持久结果；同 ID 且不同 hash 时返回 `CommandIdConflict`，不执行任何副作用。若进程在提交后、响应前崩溃，重启后的重试从 `command_receipts` 返回首次结果，业务变更不重做。回执在应用数据生命周期内保留；经用户确认的永久清除可整体移除数据库与回执表，相关导出文件按产品清理定义处理。自动按时间清理回执不纳入候选。

## 3. 命令、CAS 与回执

命令为 `Start(phase,duration_ms)`, `Pause(reason)`, `Resume`, `Tick(ClockSample)`, `Reset`, `SkipBreak`, `ResumeDetected(reason)`, `Recover(choice)`, `UpdateSettings(patch)`, `ExitRequested`, `ConfirmExit(choice)`。枚举细节中与产品行为有关的值保持 `PRODUCT_INPUT`；窗口/托盘动作归一成同一命令协议。

持久化业务命令顺序：协调器串行接收 → 先按 command_id 查询持久 receipt → 同 ID 同 hash 返回已存结果、同 ID 异 hash 返回 `CommandIdConflict` → 对新 ID 校验实体 revision → Domain 计算候选状态 → Storage 在单事务内 CAS 写业务行、revision 与 command receipt → 返回 `PersistReceipt` → Application 核验 command/revision → 发布持久状态并执行通知/窗口 effects。命令重试沿用 command_id。陈旧 CAS 返回 `RevisionConflict(current revisions)`，调用方刷新后由用户重新发起命令。

普通 Tick 使用独立运行态流程。样本的 `active_elapsed_ms` 是累计读数；对同一 session 接纳的严格递增样本计算 `delta = current - previous`，将 `delta` 加入内存运行态并推进 `runtime_seq`。相同样本是重复项，忽略且不推进序号；较小读数视为时钟源错误，冻结运行态并进入恢复提示；会话切换或恢复确认时重置样本基线，不把不同开机周期的读数相减。Tick 更新可见运行快照，不递增持久 `session_revision`，也不触发每 Tick 写库。

达到候选 checkpoint 间隔或发生必须保存的状态转换时，以当前运行态创建 `DurableSession` checkpoint，并用最后已提交 `session_revision` 做 CAS。成功后递增持久 revision、记录 checkpoint 时间及包含已累计活动量的快照，随后 UI/托盘标记该数据为已保存。提交期间视图可展示最新运行态，同时暴露 `persistence=Saving` 与最近持久 checkpoint；提交失败时冻结运行推进，保留该运行态为 `Unsaved`，显示最后已持久数据与未保存差额，允许重试或退出恢复流程。重启恢复只读最后成功 checkpoint；未提交运行态不作为恢复基准。`command_receipts.result_payload` 保存重建命令结果所需的持久字段；恢复运行态时重新生成 `runtime_seq` 和展示状态。checkpoint 的频率与可见故障提示需在应用测试中验证。

候选 checkpoint 周期为 15 秒。旧 checkpoint revision 被拒绝，不得覆盖状态转换结果。通知按 session_id + 完成事件幂等，通知失败不回滚已提交事务。

幂等事务样例：`UPDATE active_session ... WHERE session_revision = expected; INSERT session_history ...; UPDATE schema_meta SET global_revision=global_revision+1; INSERT command_receipts(command_id,payload_hash,result_payload,...); COMMIT`。任一步骤失败时整笔回滚。进程若在 COMMIT 后、发送 receipt 前崩溃，重启重试查询 receipt 并返回首次 `result_payload`。

## 4. 事务边界、恢复与退出

| 操作 | 单事务内容 | 提交前后规则 / 失败结果 |
|---|---|---|
| 普通 Tick | 默认仅更新内存运行态与 runtime_seq | 视图显示运行态并注明最近已保存 checkpoint；15秒候选 checkpoint 到达时才持久化。提交失败冻结运行态，显示 Unsaved 差额；恢复基准始终为最后成功 checkpoint。 |
| Pause/Resume/Reset/Skip | 候选 session 状态 + 必要历史记录 + revisions | 成功回执后才发布为持久确认态；历史唯一键防重复。产品定义 outcome/reason。 |
| 到期完成 | 关闭活动段、插入唯一 SessionRecord、更新轮次/状态与 revisions | 一次事务；成功后发布完成快照，再发通知。轮次含义待产品输入。 |
| Settings 修改 | 完整 Settings 新值与 settings/global revision | CAS 成功后发布配置。活动段参数快照语义待产品输入。 |
| suspend/崩溃恢复 | 最后持久 checkpoint 为恢复基准 | suspend 通知内尽力 checkpoint；失败保留此前可靠快照。启动或唤醒时进入恢复提示，确认前不推进。用户选项及活动段处理依产品输入。 |
| Exit | 用户选择导致的暂停/保存/丢弃事务 | 事务 receipt 前不结束 event loop；写失败保持窗口与最后快照并显示可重试错误。取消节点和是否需二次确认由产品组定义。 |

Migration 在接受业务命令前执行：一致性备份成功后在事务内升级 schema；备份或迁移失败保留原文件并进入可恢复错误界面。永久清除及清空历史的具体顺序、备份范围与取消语义为 `PRODUCT_INPUT`，本契约只要求执行批准流程时每一步有显式事务边界和失败回执。

## 5. 几何、主题与存储

`GeometryRequest` 输入为物理显示器宽度 `monitor_width_px` 与缩放比例 `dpi/96`；输出目标物理宽度按 `floor(width_px/10 + 0.5)`，再换算 DIP。展开/收起消费同一宽度。产品已确定展开/收起同宽、所在显示器物理宽度10%的方向；本契约沿用该决定。显示器中心点/跨屏事件顺序、API坐标空间与 DPI 边界行为属于工程接口细节，待 Windows 实测。

SQLite 候选单库包含 settings、active_session、session_history、schema_meta；历史 session_id 唯一，revision CAS 在事务内执行。备份采用 SQLite 一致性备份 API/在线备份机制，禁止直接复制活动 WAL 数据库主文件。删除与导出先使用一致性读快照；各自内容范围由产品输入定义。

## 6. 依赖边界与模块引用

```text
platform/windows --ClockSample / ThemeSnapshot / LogicalRect--> application
ui --CommandEnvelope--> application --TimerSnapshot--> ui
application --PersistRequest--> storage --PersistReceipt--> application
domain: Command + ClockSample -> candidate TimerSnapshot + effects (pure)
```

Domain 不依赖 Slint、Windows、SQLite；UI 不访问 storage/platform API；storage 不定义产品统计口径；Windows 层不推进倒计时。Application 依赖 domain、storage 抽象及 platform 事件端口，负责顺序、幂等和回执核验。

| 依赖 | 候选 | 推荐 | 影响与待决 |
|---|---|---|---|
| Rust toolchain | 当前 stable；发布时锁定 `rust-toolchain.toml` | 采用发布时最新稳定版并锁定，避免文档固定将来已过期版本 | 截至 2026-11-18 官方 release 页面列出 Rust 1.100.0（2026-11-12）；M1 需在 Windows 实际构建验证。 |
| Slint backend | `backend-winit` | Windows 桌面用 Winit | 默认 feature 和实际版本随批准的 Slint crate 一并锁定。 |
| Slint renderer | `renderer-skia` / `renderer-femtovg-wgpu` | 中文 UI 首轮候选 `renderer-skia` + Winit；官方 Windows 资料包含 Skia 的 `skunicode`/ICU 依赖说明，字体文档说明系统字体自动发现和自定义字体嵌入。 | M1 准入先核验简繁中文字体覆盖与回退、中文路径、读屏器输出；随后实测目标 Windows、Hyper-V、GPU/CPU渲染、包体、资源与打包。Skia 增加包体并要求 Visual C++ Runtime；FemtoVG-WGPU 作为工程对照。软件 renderer 官方文本限制为西文，排除出中文候选。renderer 最终以实测证据收敛。 |
| Slint license | GPLv3 / royalty-free community license / paid seat license | 若要求应用保持 MIT 且不承担 GPLv3 完整作品分发义务，采用官方 royalty-free Community License 并遵守署名条款 | 产品发布/开源形式由用户决定；GPLv3 路线要求完整衍生作品按 GPLv3 提供。应对选定版本逐项核验 notices 与第三方依赖。 |
| SQLite crate | 系统 SQLite / rusqlite `bundled` | rusqlite bundled | 部署一致、避免依赖用户机器 SQLite 版本；编译时间和二进制体积增加。crate 与 bundled SQLite 版本在建 Cargo 工程时锁定并做许可清单。 |
| SQLite durability | WAL + synchronous NORMAL / WAL + FULL | WAL + FULL，前提是每次业务转换本身事务提交 | 官方 SQLite 说明 WAL+NORMAL 在应用崩溃安全但系统断电可能丢最近提交；FULL 每事务额外同步，提供更强掉电持久性并可能增加延迟。实际 Windows 文件系统/设备仍需故障注入测试。 |

## 7. 来源、依赖与状态

- 内部来源：PR #21 基线 `4dcefb23035017571d0e134e7a370401b279f1f9`；分配修订 `e35f5f2d7ddda901e4839338ff308077d3427a82`；产品草案 `docs/product/requirements-v0.2.md`；原候选 `docs/contracts/review-packet.md`；X5–X8 `docs/quality/review-register.md`。
- 官方资料（核对日 2026-11-18）：[Rust releases](https://doc.rust-lang.org/releases.html)、[Slint license FAQ](https://slint.dev/faqs)、[Slint getting started/licensing](https://slint.dev/get-started)、[Slint backends/renderers](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/)、[Slint font handling](https://docs.slint.dev/latest/docs/slint/guide/development/fonts/)、[Slint 1.19 release](https://slint.dev/blog/slint-1.19-released)、[Slint Windows packaging / Skia ICU](https://docs.slint.dev/latest/docs/slint/guide/platforms/desktop/windows/packaging/)、[SQLite synchronous pragma](https://www.sqlite.org/pragma.html)、[SQLite WAL](https://www.sqlite.org/wal.html)、[rusqlite bundled 文档](https://docs.rs/crate/libsqlite3-sys/0.38.2)。版本和许可须随实际 Cargo 锁定文件及发行组合再次核对。
- 产品输入依赖：统计字段含义/outcome 与日期归属；清理类型、确认/取消时序和部分失败策略；导出包含内容、格式列、隐私及恢复边界；Idle目标语义与完成轮次规则。负责组：产品与交互设计；Lead 统一裁决记录。
- Windows 待验证：时钟采样/睡眠信号、DPI/多屏几何、主题订阅、退出和强杀故障恢复、WAL 文件系统持久性、打包 renderer 与辅助技术。
