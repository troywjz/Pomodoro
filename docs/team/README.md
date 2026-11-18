# 可独立对话的团队工作区

本轮用户明确要求每个角色拥有独立Git Worktree和独立Codex session，便于查看过程并直接调整。每个session的创建与运行状态由Lead实际检查，登记表记录真实结果。

## 工作分工

| 角色 | 任务分支 | 当前允许写入的文档目录 | 当前成果与验收 |
| --- | --- | --- | --- |
| 产品 | team/product-v2 | docs/team/product/ | 四项设计要求、尺寸/主题规则、验收追踪、Notion需求更新 |
| 设计 | team/design-v2 | docs/team/design/ | 独占Figma新版画板；浅/深主题、同宽展开/收起、圆圈及居中按钮；截图复核 |
| 前端 | team/frontend-v2 | docs/team/frontend/ | Rust+Slint组件/属性/回调与比例布局方案，按设计依赖待实现 |
| 后端·计时 | team/backend-timer | docs/team/backend-timer/ | 状态机、Clock、快照和计时精度契约 |
| 后端·恢复 | team/backend-recovery | docs/team/backend-recovery/ | 电源/锁屏/崩溃恢复和checkpoint协议 |
| 后端·窗口适配 | team/backend-window | docs/team/backend-window/ | 当前显示器、DPI、10%宽度、置顶及系统主题事件契约 |
| 后端·托盘 | team/backend-tray | docs/team/backend-tray/ | Slint原生托盘、单实例、隐藏/退出生命周期契约 |
| 后端·统计 | team/backend-statistics | docs/team/backend-statistics/ | 今日统计、跨日/时区及completed/interrupted/skipped口径 |
| 后端·数据管理 | team/backend-data | docs/team/backend-data/ | JSON/CSV、备份、清空历史与永久清除应用层流程 |
| 数据库 | team/database | docs/team/database/ | SQLite schema、事务、revision、迁移和Repository接口 |
| 测试 | team/testing-v2 | docs/team/testing/ | 独立需求/设计审查、显示主题矩阵、模块测试计划与缺陷记录 |

## 独占范围与依赖

- 产品独占Notion需求正文修订；设计独占指定Figma文件写入；测试只读检查这些来源。其他session先输出各自目录中的方案，避免跨session覆盖。
- 各角色先读AGENTS.md、docs/ui-revision-v2.md及已有规格。只在自己Worktree内修改并按文件白名单提交；所有session当前禁止创建业务源码、Cargo工程或实现型试验。
- 产品/设计完成新版后，Lead冻结Geometry/Theme、TimerSnapshot及Repository公共接口；数据库、统计、数据管理对齐事务与数据结构。恢复依赖计时和数据库，托盘依赖窗口与应用协调器，前端依赖新版原型及上述接口。
- session是可直接对话的用户工作区，保持可用，完成首轮任务后等待反馈，不自动归档。Lead读取真实提交、文件和工具证据进行验收；任一角色自述完成后仍需复核。
- session输出具体问题和下一项可执行动作。关键架构、业务开发阶段切换、PR合并与发布由用户确认，禁止把草案写成已批准。
- 写入外部系统或推送前执行公开信息、文件差异、元数据及相关历史复核；公开仓库使用noreply身份。内部session ID和本机Worktree绝对路径保存在本地忽略目录，不进入公开文档。

Lead在本地.local中维护session登记表。GitHub Issue记录交付和依赖，Notion项目页提供各角色导航。
