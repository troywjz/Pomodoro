# 工程契约入口

状态：统一评审入口，业务接口候选待批准。组织重整统一维护责任，历史候选保留研究状态。

| 主题 | 维护责任 | 阅读入口 |
| --- | --- | --- |
| Command/TimerSnapshot/Clock/应用时序 | 计时与应用流程 | [统一候选](../interface-contract.md)、[评审包](review-packet.md) |
| Geometry/Theme/Windows生命周期 | Windows平台 | [评审包](review-packet.md)；按需查[平台研究](../archive/role-studies/backend-window/window-adapter-contract.md)、[托盘研究](../archive/role-studies/backend-tray/tray-lifecycle.md) |
| Repository/JSONL/导出与清理 | 本地数据 | [当前 JSONL 接口](m1-storage-port.md)；[评审包候选](review-packet.md)；按需查[数据库研究](../archive/role-studies/database/repository-proposal.md) |
| 视图输入与回调 | Slint视图 | 批准接口与[UI v2](../design/ui-v2/ui-v2-review.md)；按需查[视图研究](../archive/role-studies/frontend/slint-ui-proposal.md) |

公共字段由维护者提出，Lead检查受影响任务并记录用户批准版本。任务引用具体commit和接口版本，聊天结论经记录后成为稳定输入。
