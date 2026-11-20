# 模块责任与依赖

状态：职责边界继续沿用 `M0-Contracts v1.0.1`。用户已授权 M1 并于 2026-11-20 更新活动存储为 JSONL；当前实现见[存储交付记录](../plans/deliveries/automatic-backup.md)及[存储接口](../contracts/m1-storage-port.md)。

| 逻辑模块 | 候选责任 | 显式边界 |
|---|---|---|
| domain | 纯计时状态转移、阶段/轮次、revision 计算、接收注入的 `ClockSample` | 不调用 Windows、UI、DB 或时钟全局 API |
| application | 串行命令、expected revision 校验、候选快照、持久化请求/回执、effect 顺序、恢复/退出协调 | 不直接操纵控件或 JSONL 文件细节；不定义产品统计语义 |
| platform/windows | 时钟/系统事件采集、主题/DPI/显示器几何、托盘、窗口、单实例 | 不自行推进业务计时；系统采样能力待实测 |
| storage | Repository、JSONL 追加/CAS、旧版导入、JSONL 自动备份及查询 | 不定义统计口径；字段以统一契约为准 |
| ui | Slint 视图、布局、操作、辅助技术、渲染 | 不维护第二份计时状态；不直接访问平台或存储 API |

```text
platform/windows -> ClockSample, ThemeSnapshot, LogicalRect -> application
ui -> CommandEnvelope -> application -> TimerSnapshot -> ui
application -> PersistRequest -> storage -> PersistReceipt -> application
application -> pure domain transition(Command, ClockSample) -> candidate + effects
```

application 是跨层状态转换的唯一协调者。持久业务转换在 Storage 成功提交且回执通过 command/revision 核验后发布；运行 Tick 更新只推进内存运行态序号，持久 session revision 仅由成功 checkpoint 推进。UI/托盘展示同一 TimerSnapshot，其中标记 clean/saving/unsaved 与最近持久 checkpoint。通知 effects 在业务事务提交后执行。统计由产品与交互设计定义含义，application 组织调用，storage 按字段查询。

| 下游任务 | 依赖契约版本 | 前置输入/验收 |
|---|---|---|
| 产品/交互设计 | M0-Contracts v1.0.1 字段占位 | 冻结统计、清理取消、导出及 Idle/轮次语义，形成产品字段定义与 AC |
| Domain/Application | 用户批准 M1 后采用的 Command、ClockSample、TimerSnapshot、revision/CAS | M1 假时钟、重复/陈旧命令、事务失败、恢复/退出时序 |
| Storage | 用户批准 M1 后采用的实体/schema与 PersistRequest/Receipt | M1 迁移、CAS、重复session、强杀恢复、备份一致性、导出范围 |
| Windows 平台 | ClockSample、ThemeSnapshot、LogicalRect | suspend/resume、漏通知、混合DPI、多屏、主题变化和窗口恢复实测 |
| Slint UI | 批准后的 TimerSnapshot、ThemeSnapshot、Geometry | UI v2已确定方向维持；中文字体、无障碍文本和路径支持作为 M1 renderer 准入；具体 renderer 经目标机与 Hyper-V 工程验证收敛 |

候选依赖及许可证比较见[统一契约第6节](../interface-contract.md#6-依赖边界与模块引用)与[评审包](../contracts/review-packet.md)。Rust/Slint 基线、JSONL 存储与 v0.1 SQLite 导入器已进入 M1/v0.2 实现；renderer、多屏、休眠和资源目标仍按对应验证任务跟踪。
