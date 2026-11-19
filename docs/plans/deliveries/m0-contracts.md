# M0-Contracts #19 交付记录

- 候选版本：`M0-Contracts v1.0.1`，返修基于独立审查 dd4402a。
- 起始提交：`e35f5f2d7ddda901e4839338ff308077d3427a82`。
- 状态：文档候选已整理；等待产品语义输入、用户批准 M0 候选技术路线/完整设计及 M1 启动授权。M1 工程验证安排在授权后；无业务实现。

## 交付路径

- `docs/interface-contract.md`：统一 ClockSample、CommandEnvelope、TimerSnapshot、SessionRecord、Settings、Geometry、Theme、Repository/receipt、revision/CAS、命令处理、恢复/退出时序和依赖建议；作为唯一公共字段来源。
- `docs/contracts/review-packet.md`：产品输入矩阵、用户架构决策及替代影响、来源和验证状态。
- `docs/architecture/modules.md`：责任依赖图与下游任务依赖。
- `docs/plans/deliveries/m0-contracts.md`：本交付记录。

## v1.0.1 审查返修

- ClockSample 明确采用当前启动周期累计单调读数、差值算法与重复/回退样本行为；运行态 `runtime_seq` 与持久 `session_revision` 分离，定义 checkpoint 的 Saving/Clean/Unsaved 显示及恢复基准。
- 新增 `command_receipts` schema，持久保存 ID、payload hash、首次结果；与业务变更及 revision 同事务提交。覆盖提交后响应前崩溃重试、同 ID 不同内容冲突和随应用数据生命周期保留的边界，并给出事务样例。
- 中文 renderer 推荐更新为 Winit + Skia，依据 Slint 官方 Windows 文档中 `skunicode`/ICU 说明及字体文档；中文字体、路径、辅助技术、目标 Windows 与 Hyper-V 测试列为 M1 准入项。软件 renderer 明确列入中文排除项。
- 已确定的 UI 四项方向保持既定状态，物理屏宽 10% 同宽要求作为当前输入；用户决策表聚焦应用发布/许可，renderer/toolchain/SQLite 由工程证据收敛。
- M1 前置门槛明确为：产品语义输入、用户批准 M0 候选技术路线与完整设计、用户明确授权启动 M1。仅在该授权后创建 Cargo 工程/业务源码/实现型试验并开展工程验证。
- SQLite 单库 schema 清单补入 `command_receipts`，与字段字典和同事务幂等规则一致。

## 已完成的一致性检查

- 统一文档定义字段名；评审包和架构文档链接回唯一字段来源，没有新建并行字段表。
- transaction commit → receipt 核验 → 发布候选状态 → 执行通知/窗口 effects 的顺序一致；CAS 冲突、重复命令、存储失败和退出失败均保留具体结果路径。
- 产品含义为 `PRODUCT_INPUT` 的 X5 统计、X6 清理/取消、X8 导出，以及 Idle/轮次语义均保持待定，已指向产品与交互设计组；本交付未替产品作决定。
- renderer、Slint 授权、SQLite durability、Rust/SQLite crate 作为候选/用户决策项；官方来源和核对日写入文档。
- v1.0.1 返修后对完整差异执行 `git diff --check`、相对链接扫描、字段/时序跨文档检查。当前没有 Cargo 工程，未执行构建、应用测试、中文字体/读屏器、Windows/Hyper-V 实测或故障注入。
- 本次勘误完成完整返修 diff 与相对链接核对；未新增技术候选。

## 待输入与用户决策

- 产品与交互设计：统计 outcome/日期归属与实际时长；清理范围、结束活动与取消顺序；导出字段/列和恢复边界；Idle 与轮次定义。
- 用户：应用发布形态与 Slint 许可路线（MIT 应用 + Community 署名、GPLv3 完整作品或商业 Seat License）。
- M1 工程收敛：锁定 Rust/Slint/crate 版本，核验中文支持；对 Skia/FemtoVG-WGPU 做目标 Windows 与 Hyper-V renderer、辅助技术、资源与打包比较；测试 SQLite durability 并形成许可清单。

## 失败与阻塞

没有执行失败。缺少产品语义与架构批准是候选冻结及下游实现的前置依赖；当前契约将相关字段/行为显式保留待定。交付后停止，等待 Lead 复核并安排独立审查。
