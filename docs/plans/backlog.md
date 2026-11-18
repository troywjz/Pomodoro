# 待办与依赖

本轮 session/工作树配置与责任边界见[M0 任务分配 v2](active/m0-task-allocation.md)。先配置产品与交互设计（M0 需求与原型收尾）和工程契约两个执行任务；独立审查在收到交付 commit 后启动。

| 任务 | 状态 | 依赖 | 交付 |
| --- | --- | --- | --- |
| [M0-UI #18](https://github.com/troywjz/Pomodoro/issues/18)：原型评审收尾 | 本地设计可推进；Figma同步受通道限制 | 产品v0.2、SVG/PNG、UI四要求 | 全状态/主题/尺寸、原生稿、独立检查和用户确认 |
| [M0-Contracts #19](https://github.com/troywjz/Pomodoro/issues/19)：共享契约评审 | Ready文档评审 | [任务说明](active/contracts-review.md)、[评审包](../contracts/review-packet.md)与独立审查项 | 决策、版本及用户架构批准 |
| [M1-Foundation #20](https://github.com/troywjz/Pomodoro/issues/20)：可运行基线 | 等待M0批准 | 完整Notion/Figma和架构/许可/依赖决定 | 唯一构建入口、实际Windows构建、集成测试起点 |

M1后按稳定接口分派计时/应用、最小Windows Shell、视图，存储按依赖接入。实现默认最多3个并行，审查按需。每项有可验证用户结果，研究产出与功能接受分开记录。

旧岗位Issue成果收进[研究归档](../archive/role-studies/README.md)，未决工作转入以上任务，取消永久岗位session配额。
