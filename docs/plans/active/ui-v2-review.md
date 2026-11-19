# M0-产品与交互设计：需求与 UI v2评审收尾

责任组：产品与交互设计。当前任务：交付一份 M0 需求与原型评审包，承接 #18 的 UI v2 收尾以及相关未决产品行为；同一 session/工作树按本交付包结束。Lead 负责协调、复核及组织用户审批。

目标：保留四项 UI 要求，澄清行为和指标，维护验收定义，补齐可编辑设计与证据，交用户确认。

## 输入

- [产品v0.2](../../product/requirements-v0.2.md)与 [UI 修订输入](../../ui-revision-v2.md)。
- [SVG](../../design/ui-v2/pomodoro-ui-v2.svg)、[PNG](../../design/ui-v2/pomodoro-ui-v2-preview.png)、[说明](../../design/ui-v2/ui-v2-review.md)。
- [验收](../../acceptance.md)AC-43—46与[未决UI项](../../quality/review-register.md)。
- [契约评审包](../../contracts/review-packet.md)中的产品语义：统计口径、清理确认/取消、导出范围和 idle/recovery 行为；具体技术字段由契约任务维护。只在需要澄清相应行为时按入口查直接来源。
- 执行任务创建时填写统一基线commit；Figma文件4U9TTroE0k4Rkef4K9LIG7归当前设计任务唯一写入。

## 范围

允许写入 `docs/product/**`、`docs/ui-revision-v2.md`、`docs/acceptance.md` 的 AC 定义、`docs/design/ui-v2/**` 和 `docs/plans/deliveries/m0-ui.md`。候选行为集中放 `docs/product/m0-behavior-decisions.md`，写明建议、验收样例和待用户决定项；已批准输入保留版本，AC 原编号保持。应用实测结果由后续独立验证记录。

外部唯一写入范围为 Figma 文件的独立 UI v2 页面及现有 Notion 产品需求页；项目总览和审批记录由 Lead 维护。产品变化交 Lead 同步任务依赖，公共契约由契约任务按批准版本接入。当前保持准备阶段；后续正式派发再编辑候选资料和外部页面。业务源码、Cargo 工程、实现型试验、付费升级和共享变更保持待启动。

## 验收与停止

1. 圆圈与数字同源，全状态明确，idle无目标/已选目标分开。
2. 标签水平垂直居中，整按钮热区、键盘/焦点/禁用态有证据。
3. 浅深覆盖主窗/设置/确认/错误/恢复，Light默认及失败回退明确。
4. 展开收起同宽，四固定屏/DPI与混合屏标注物理/逻辑单位，保持10%。
5. 实际渲染和独立检查；原生Figma同步与用户批准分开。通道受限交具体错误和可编辑本地结果。
6. 需求/指标/取消与导出行为有明确候选口径和验收样例，未决项交用户裁决；将相关版本交契约任务接入。
7. 返回需求与原型评审包 commit、真实验证和未决项后结束本轮，Lead安排独立评审。
