# 待办与依赖

本轮 session/工作树配置与责任边界见[M0 任务分配 v2](active/m0-task-allocation.md)。先配置产品与交互设计（M0 需求与原型收尾）和工程契约两个执行任务；独立审查在收到交付 commit 后启动。

| 任务 | 状态 | 依赖 | 交付 |
| --- | --- | --- | --- |
| [M0-UI #18](https://github.com/troywjz/Pomodoro/issues/18)：原型评审收尾 | Review：v2.1 本地候选已过独立复核；待用户评审，Figma 额度限制 | 产品v0.2、行为候选、浅深九状态 SVG/PNG、UI四要求 | 用户行为/设计确认、原生同步与下游批准 |
| [M0-Contracts #19](https://github.com/troywjz/Pomodoro/issues/19)：共享契约评审 | Review：v1.0.1 已过独立文档复核，待用户路线/许可/启动批准 | [任务说明](active/contracts-review.md)、[评审包](../contracts/review-packet.md)、产品批准版本 | 用户批准后冻结相应输入版本 |
| [M1-Foundation #20](https://github.com/troywjz/Pomodoro/issues/20)：可运行基线 | 等待M0批准 | 完整Notion/Figma和架构/许可/依赖决定 | 唯一构建入口、实际Windows构建、集成测试起点 |

M1后按稳定接口分派计时/应用、最小Windows Shell、视图，存储按依赖接入。实现默认最多3个并行，审查按需。每项有可验证用户结果，研究产出与功能接受分开记录。

旧岗位Issue成果收进[研究归档](../archive/role-studies/README.md)，未决工作转入以上任务，取消永久岗位session配额。

当前用户输入见[评审与启动决定](active/m0-user-decisions.md)，本轮证据见[独立审查记录](../quality/m0-review-20261118.md)。
