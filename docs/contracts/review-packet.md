# 共享契约评审包

版本：`M0-Contracts v1.0.0` 候选（2026-11-18）。状态：待产品语义输入及用户架构审批。公共字段定义只维护于[模块接口与数据契约提案](../interface-contract.md)；本页记录依赖、建议、替代影响和待决项。

## 候选统一方向

| 议题 | 技术候选 | 状态 / 验收样例 |
|---|---|---|
| Command/TimerSnapshot/Clock | command_id 幂等；ClockSample 一次采样注入；UI/托盘读同一快照 | 字段见统一契约第2节；假时钟推进 1000ms 时，内核剩余量只按一份 sample 变化 |
| revision/CAS | session/settings revision 分域，global revision 表示已提交事务序列；旧 CAS 拒绝 | 同一配置 revision 的两个更新只允许一个提交；陈旧回执不覆盖新快照 |
| 事务/故障 | 业务状态、历史与 revision 同事务；提交 receipt 后发布快照 | 注入提交失败，候选状态不对 UI 宣告已保存；重放同 command_id 只产生一条历史 |
| 恢复/退出 | 持久 checkpoint 是恢复基准；退出事务回执前不关闭 event loop | 强杀后加载最后一次有效快照；退出写失败时进程保持可恢复并显示错误。均待 Windows/应用测试 |
| Geometry/Theme | 屏幕物理宽度按十分之一 half-up 后转 DIP；System theme 经 platform 采集 | 多屏、跨屏、DPI、主题变化均待 Windows 实测；产品公式待架构批准 |
| Repository/schema | SQLite 单库，事务内迁移/CAS，活动 WAL 用一致性备份机制 | 迁移中途失败回滚且保留原库；具体 durability 为用户架构决策 |

## 产品语义依赖（交产品与交互设计组定义）

| 字段/议题 | 需要产品组明确的语义 | 契约处理方式 |
|---|---|---|
| 统计 X5 | 哪些 outcome 计入 completed focus；`actual_active_ms` 对 interrupted/skipped 的含义；按哪一日期归属；跨日规则 | 保留 `SessionRecord.actual_active_ms` 字段槽位，算法与查询验收样例待输入 |
| 清理 X6 | 清历史/永久清除的目标范围；活动段结束行为；最终确认与每个取消节点顺序；部分失败如何反馈 | Storage 暴露分步事务与失败回执；步骤顺序不冻结，待产品语义 |
| 导出 X8 | Settings/活动快照是否包含；固定 CSV 列、隐私字段、导出范围；导出文件是否用于恢复 | `ExportEnvelope` 仅冻结版本骨架；字段包含与 CSV/恢复规则待产品组定义 |
| 计时/UI状态 | Idle 是否预选目标、开始轮次与长休轮次规则、恢复选择对活动段的影响 | 命令和快照保留明确占位；状态语义/API枚举待输入 |

产品含义与技术机制出现冲突时，契约维护者提交问题和技术影响给产品组；Lead 组织用户裁决并记录批准版本。当前字段占位不构成产品决定。

## 架构与依赖决策项（请用户裁决）

| 决策 | 推荐 | 替代方案及实际影响 |
|---|---|---|
| Slint 授权与发布形态 | 若项目保持 MIT 且不采用 GPLv3 完整作品路线，选 Slint 官方 Community License 并按其条款署名 | GPLv3 要求完整作品以 GPLv3 分发；付费 Seat License 提供无 copyleft 路线并含标准支持。用户需确定应用是否开源、许可证及是否接受署名/席位成本 |
| Slint backend/renderer | Windows 首轮使用 Winit backend；先评估软件 renderer，验收不满足再测 FemtoVG-WGPU | Software 更便携但文档列有绘制限制；WGPU 有 GPU/驱动条件；Skia 增加磁盘体积与部署依赖。最终选择依据实际机器视觉、辅助技术与资源测试 |
| Rust 版本 | M1 创建工程时采用当时最新 stable 并锁定工具链 | 评审日官方 Release Notes 显示 1.100.0 于 2026-11-12 发布；若工程依赖尚未适配，可选已验证 stable 并记录原因 |
| SQLite 构建 | rusqlite bundled，固定 crate/SQLite 版本并出具依赖许可证清单 | 系统 SQLite 可缩小应用自带依赖，但用户机器版本/安装成为变量；bundled 增加编译与包体 |
| SQLite durability | WAL + `synchronous=FULL` 用于关键事务 | WAL + `NORMAL` 在应用崩溃下事务安全但系统崩溃/断电时最近提交可能回滚，性能与延迟较好；FULL 每事务同步，延迟增加。推荐 FULL；Windows 文件系统仍须故障注入验证 |

## 来源与验收状态

- 工程输入：PR #21 基线 `4dcefb23035017571d0e134e7a370401b279f1f9`；M0 分工 v2 `e35f5f2d7ddda901e4839338ff308077d3427a82`。
- 产品草案：`docs/product/requirements-v0.2.md`；评审登记：`docs/quality/review-register.md` X5–X8。它们支持需求输入与待决标记，不作为已批准产品定义。
- 官方一手资料核对日 2026-11-18：Rust [Release Notes](https://doc.rust-lang.org/releases.html)；Slint [FAQ/license](https://slint.dev/faqs)、[license selection](https://slint.dev/get-started)、[backend/renderer](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/)；SQLite [`synchronous`](https://www.sqlite.org/pragma.html)、[WAL](https://www.sqlite.org/wal.html)；rusqlite [bundled feature](https://docs.rs/crate/libsqlite3-sys/0.38.2)。正式版本锁定时需重新核对依赖和许可清单。
- 本提交完成的验证限于文档引用、重复字段/时序一致性、候选状态和样例可审阅性。未运行应用、构建 Cargo 工程、Windows 实测或故障注入；这些结果均待相应任务执行。

## 下游依赖

产品与交互设计先补齐语义表，再由 Lead 汇总用户裁决；用户批准架构、许可与 durability 路线后，M1 才锁定 crate/feature 并建立可运行基线。实现任务引用本版本和批准修订，不从归档研究直接复制互相冲突的字段。
