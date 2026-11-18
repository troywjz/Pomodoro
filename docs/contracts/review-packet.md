# 共享契约评审包

版本：`M0-Contracts v1.0.1` 候选（2026-11-18）。状态：待产品语义输入及用户发布/许可决策；公共字段定义只维护于[模块接口与数据契约提案](../interface-contract.md)。本修订回应独立审查 dd4402a。

## 候选统一方向

| 议题 | 技术候选 | 状态 / 验收样例 |
|---|---|---|
| ClockSample | `active_elapsed_ms`、`boot_elapsed_ms` 为同一次启动内固定原点的累计单调读数；协调器取相邻已接纳样本差值 | 重复值幂等忽略、递增值只累计一次、回退读数冻结并进入恢复；重启后重置读数基线 |
| 运行态与持久 revision | 每个可见 Tick 只推进进程内 `runtime_seq`；持久 `session_revision` 只由 checkpoint CAS 成功推进 | 注入 checkpoint 成功/失败，核对 clean/saving/unsaved、未保存差额与重启基准 |
| 跨重启幂等 | `command_receipts` 保存 command_id、payload_hash、首次结果；与业务写入/revision 同事务 | 模拟 COMMIT 后响应前崩溃再重试，返回原结果且只写一条业务记录；同 ID 不同 payload 返回冲突 |
| 恢复/退出 | 最近成功 checkpoint 为重启恢复基准；持久命令以 receipt 核验结果 | 强杀恢复和退出写失败需应用/Windows测试；本包给出事务样例，结果待执行 |
| Geometry | 承接已确定的展开/收起同宽、所在显示器物理宽度10%；按 `floor(width_px/10 + 0.5)` 后换算 DIP | 产品方向已经确定；显示器识别、DPI API和跨屏事件顺序作为工程细节完成 Windows 实测 |
| Repository/schema | SQLite 单库、事务内 schema/CAS/receipt；活动 WAL 使用一致性备份机制 | 迁移失败回滚、重复业务命令与幂等回执联测 |

## 产品语义依赖（交产品与交互设计组定义）

| 字段/议题 | 需要产品组明确的语义 | 契约处理方式 |
|---|---|---|
| 统计 X5 | 哪些 outcome 计入 completed focus；`actual_active_ms` 对 interrupted/skipped 的含义；日期归属和跨日规则 | 保留 `SessionRecord.actual_active_ms` 字段槽位，算法与查询验收样例等待产品输入 |
| 清理 X6 | 清历史/永久清除范围；活动段结束行为；最终确认与取消节点顺序；部分失败反馈 | Storage 提供分步事务与失败回执，操作顺序跟随产品定义 |
| 导出 X8 | Settings/活动快照包含范围；CSV列、隐私字段、导出用途和恢复边界 | `ExportEnvelope` 冻结版本骨架，内容范围与格式等待产品定义 |
| 计时状态 | Idle目标、完成轮次、长休规则、恢复选择后活动段处理 | 命令/快照保留字段槽位，枚举语义和验收样例等待产品输入 |

## 用户决策：发布与许可

| 决策 | 推荐 | 可比较选项与影响 |
|---|---|---|
| 应用发布形态及 Slint 许可路线 | 若 Pomodoro 保持 MIT 发行，选择 Slint 官方 Community License 并满足署名条件 | GPLv3 路线要求完整作品按 GPLv3 分发；付费 Seat License 提供商业授权和标准支持。请用户确定应用采用 MIT/其他许可、GPLv3 完整作品路线或商业 Seat License，并确认相应署名/费用安排。 |

## 工程验证项：由 M1 收敛

| 项目 | 当前推荐 | 验证/批准范围 |
|---|---|---|
| Slint renderer | Windows Winit + `renderer-skia` 作为中文桌面首轮候选 | 官方 Windows 资料列出 Skia 的 `skunicode`/ICU 依赖，字体文档说明系统字体自动发现及自定义字体嵌入。M1 准入先检查简繁中文字体覆盖与回退、中文 UI/路径、屏幕阅读器输出；随后对 Windows 目标机、Hyper-V、GPU/CPU兼容性、视觉质量、资源与打包进行实测。软件 renderer 当前官方文本范围为西文，列入排除项；FemtoVG-WGPU 作为工程对照。Skia 的 MSVC/运行库和包体成本纳入比较，renderer/features 按实测证据收敛后列入架构批准范围。
| Rust 工具链 | M1 开工时最新 stable 并锁定 | 官方 Release Notes 在核对日列出 Rust 1.100.0（2026-11-12）；M1 用实际依赖编译验证并锁定适配版本。
| SQLite 构建 | rusqlite `bundled` | 固定 crate/SQLite 版本并生成第三方许可清单；对系统 SQLite 方案比较安装体积与环境依赖。
| SQLite durability | WAL + `synchronous=FULL` 用于业务关键事务 | 对照 WAL + `NORMAL` 的提交延迟；FULL 每笔事务追加同步以增强掉电持久性。目标 Windows 文件系统需故障注入，记录实际差异后批准默认值。

## 来源、状态与下游

- 工程输入：PR #21 基线 `4dcefb23035017571d0e134e7a370401b279f1f9`；M0 分工 v2 `e35f5f2d7ddda901e4839338ff308077d3427a82`；独立审查基准 `dd4402a`。
- 产品草案：`docs/product/requirements-v0.2.md`；评审登记 X5–X8：`docs/quality/review-register.md`。UI 四项已确定方向保持原记录，10%同宽要求属于既定输入。
- 官方一手资料（核对日 2026-11-18）：Rust [Release Notes](https://doc.rust-lang.org/releases.html)；Slint [FAQ/license](https://slint.dev/faqs)、[license selection](https://slint.dev/get-started)、[backends/renderers](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/)、[font handling](https://docs.slint.dev/latest/docs/slint/guide/development/fonts/)、[1.19 release](https://slint.dev/blog/slint-1.19-released)、[Windows packaging / Skia ICU](https://docs.slint.dev/latest/docs/slint/guide/platforms/desktop/windows/packaging/)；SQLite [`synchronous`](https://www.sqlite.org/pragma.html)、[WAL](https://www.sqlite.org/wal.html)；rusqlite [bundled feature](https://docs.rs/crate/libsqlite3-sys/0.38.2)。正式版本锁定时依具体依赖与发行组合复核。
- 本轮验证范围：文档字段与时序交叉审阅、相对链接、diff 检查。应用构建、Unicode/辅助技术实测、Windows/Hyper-V、崩溃注入、renderer资源和SQLite持久性均为 M1 待执行项。
- 下一步：产品与交互设计组补齐 X5/X6/X8 和计时状态语义；用户裁决发布/许可路线；M1 按工程准入表测试 renderer/工具链/SQLite 并提交证据后冻结架构版本。
