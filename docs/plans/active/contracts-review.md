# M0-Contracts：共享契约评审包

目标：将跨模块字段和行为组织成一个版本化候选评审包，明确每项建议、来源、影响和待用户决定事项。关联 Issue #19。

## 输入与最小阅读

- 基线：PR #21，`4dcefb23035017571d0e134e7a370401b279f1f9`；执行时记录分配管理提交的实际 HEAD。
- 开工阅读：`AGENTS.md`、`docs/project-state.md`、本说明、[契约入口](../../contracts/README.md)和[评审包](../../contracts/review-packet.md)。
- 直接依赖：[公共候选契约](../../interface-contract.md)、[模块边界](../../architecture/modules.md)、[评审登记](../../quality/review-register.md)中的 X5—X8。
- 涉及具体行为时读取产品 v0.2 的对应段落；冲突字段按契约入口检索具体归档来源，保留源提交，避免整批读取研究。

## 唯一写入范围

- `docs/contracts/**`
- `docs/interface-contract.md`
- `docs/architecture/modules.md`
- 本任务交付文件 `docs/plans/deliveries/m0-contracts.md`

产品需求、UI 附件、项目状态、任务说明和公共验收登记由相应主责维护。Notion/Figma 当前由其他写入者负责。

## 后续执行交付

1. 统一 Command/TimerSnapshot/Clock、revision/CAS、Geometry/Theme、Repository/schema 的候选字段与维护责任，消除互相冲突的重复定义。
2. 对统计 X5、清理及取消 X6、revision X7、导出 X8 逐项给出建议、备选影响和验收样例；候选建议与用户批准分别记录。
3. 整理恢复/退出、持久化回执和失败顺序，指出需要 Windows 实测或应用构建才能验证的部分。
4. 列出依赖/renderer/许可选择的决策项；需要核对当前技术或许可事实时检索官方一手资料，记录版本和来源。
5. 返回候选版本、实际修改范围、验证与未决问题，提交后停止，交 Lead 安排独立审查和用户架构批准。

## 验证与停止条件

- 检查字段引用、相对链接、候选/批准状态与各取消节点的前后一致性。
- 将文档审查、计算样例、未来应用测试各自记录。当前不创建业务源码、Cargo 工程、实现型试验，不安装依赖，不自动推送、合并或发布。
- 本轮首个 session 回合只核对输入、独立分支/工作树及范围，返回就绪状态后停止，等待 Lead 派发后续执行。
- 超出允许路径、涉及用户决策或外部阻塞时，交明确问题和已完成部分，不扩展为其他模块任务。
