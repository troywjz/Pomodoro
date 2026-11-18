# 模块责任与依赖

状态：`M0-Contracts v1.0.0` 候选；待产品输入及用户架构审批。共享字段唯一来源为[模块接口与数据契约提案](../interface-contract.md)。业务开发与 Windows 实测尚未开始。

| 逻辑模块 | 候选责任 | 显式边界 |
|---|---|---|
| domain | 纯计时状态转移、阶段/轮次、revision 计算、接收注入的 `ClockSample` | 不调用 Windows、UI、DB 或时钟全局 API |
| application | 串行命令、expected revision 校验、候选快照、持久化请求/回执、effect 顺序、恢复/退出协调 | 不直接操纵控件或 SQLite 细节；不定义产品统计语义 |
| platform/windows | 时钟/系统事件采集、主题/DPI/显示器几何、托盘、窗口、单实例 | 不自行推进业务计时；系统采样能力待实测 |
| storage | Repository、SQLite 事务/CAS、schema迁移、一致性备份及查询 | 不定义统计口径；字段以统一契约为准 |
| ui | Slint 视图、布局、操作、辅助技术、渲染 | 不维护第二份计时状态；不直接访问平台或存储 API |

```text
platform/windows -> ClockSample, ThemeSnapshot, LogicalRect -> application
ui -> CommandEnvelope -> application -> TimerSnapshot -> ui
application -> PersistRequest -> storage -> PersistReceipt -> application
application -> pure domain transition(Command, ClockSample) -> candidate + effects
```

application 是跨层状态转换的唯一协调者。持久化状态变更在 Storage 成功提交且回执通过 command/revision 核验后发布；通知和窗口 effects 在提交后执行。计时 UI 展示同一 TimerSnapshot。统计由产品与交互设计定义含义，application 组织调用，storage 按字段查询。

| 下游任务 | 依赖契约版本 | 前置输入/验收 |
|---|---|---|
| 产品/交互设计 | M0-Contracts v1.0.0 字段占位 | 冻结统计、清理取消、导出及 Idle/轮次语义，形成产品字段定义与 AC |
| Domain/Application | 批准后的 Command、ClockSample、TimerSnapshot、revision/CAS | 假时钟、重复/陈旧命令、事务失败、恢复/退出时序 |
| Storage | 批准后的实体/schema与 PersistRequest/Receipt | 迁移、CAS、重复session、强杀恢复、备份一致性、导出范围 |
| Windows 平台 | ClockSample、ThemeSnapshot、LogicalRect | suspend/resume、漏通知、混合DPI、多屏、主题变化和窗口恢复实测 |
| Slint UI | 批准后的 TimerSnapshot、ThemeSnapshot、Geometry | UI v2设计输入、全部状态/错误/辅助技术验收；具体 backend/renderer 由架构批准锁定 |

候选依赖及许可证比较见[统一契约第6节](../interface-contract.md#6-依赖边界与模块引用)与[评审包](../contracts/review-packet.md)。目录结构、crate/API 版本、renderer、许可路线、SQLite durability 均待审批，不表示构建或测试已通过。
