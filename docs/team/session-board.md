# 第二轮任务与验收看板

11个独立Codex session已实际创建并启动，各自工作树检出下列独立分支。用户可在Pomodoro项目侧边栏按session名称直接进入对话；工作树保留供持续调整。

| Session名称 | Issue | 分支 | 首轮验收重点 |
| --- | --- | --- | --- |
| Pomodoro 产品 | [#6](https://github.com/troywjz/Pomodoro/issues/6) | team/product-v2 | 四项要求、DPI尺寸、AC映射、Notion读回 |
| Pomodoro 设计 | [#7](https://github.com/troywjz/Pomodoro/issues/7) | team/design-v2 | 新版Figma、圆圈/居中/浅深主题/同宽比例 |
| Pomodoro 前端 | [#8](https://github.com/troywjz/Pomodoro/issues/8) | team/frontend-v2 | Slint组件、属性/回调、布局与辅助技术契约 |
| Pomodoro 后端·计时 | [#9](https://github.com/troywjz/Pomodoro/issues/9) | team/backend-timer | 纯状态机、Clock、准确进度、幂等 |
| Pomodoro 后端·恢复 | [#10](https://github.com/troywjz/Pomodoro/issues/10) | team/backend-recovery | 睡眠/锁屏/崩溃、快照与恢复决策 |
| Pomodoro 后端·窗口适配 | [#11](https://github.com/troywjz/Pomodoro/issues/11) | team/backend-window | 10%宽度、DPI/跨屏、系统主题与置顶 |
| Pomodoro 后端·托盘 | [#12](https://github.com/troywjz/Pomodoro/issues/12) | team/backend-tray | 单实例、显示隐藏、退出与Explorer恢复 |
| Pomodoro 后端·统计 | [#13](https://github.com/troywjz/Pomodoro/issues/13) | team/backend-statistics | 完成口径、实际专注、跨日与时区 |
| Pomodoro 后端·数据管理 | [#14](https://github.com/troywjz/Pomodoro/issues/14) | team/backend-data | 导出、备份、清空与永久删除边界 |
| Pomodoro 数据库 | [#15](https://github.com/troywjz/Pomodoro/issues/15) | team/database | schema、事务/revision、迁移与Repository |
| Pomodoro 测试 | [#16](https://github.com/troywjz/Pomodoro/issues/16) | team/testing-v2 | 独立检查、显示主题矩阵与缺陷证据 |

当前处于规格/设计阶段。各角色首轮提交后，Lead读取实际文件和差异，再交独立测试session复查。当前启动状态和后续成果验收分开记录；用户直接修改某角色要求后，Lead协调受影响的依赖。

本机路径与session标识由本地忽略目录登记表保存；公开看板只保留角色、Issue和分支。新版Notion与Figma确认、关键架构批准后启动业务代码，合并及发布继续由用户审批。
