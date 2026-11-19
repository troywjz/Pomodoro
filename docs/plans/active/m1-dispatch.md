# M1首轮有限任务

输入：M0汇总 `e530957`、[用户批准与新增要求](../../product/approval-20261118.md)、[存储接口v1](../../contracts/m1-storage-port.md)。本说明所在管理提交作为三个执行任务的共同起点。每项使用全新session与独立工作树，执行优先工具当前确认可用的`gpt-6-luna`。

| 任务 | 唯一写入范围 | 交付终点与依赖 |
| --- | --- | --- |
| 产品与交互设计：精简SVG v2.2 | `docs/design/ui-v2/**`、`docs/product/**`、`docs/acceptance.md`的设计AC、`docs/plans/deliveries/ui-minimal.md`；Notion产品页 | 浅深各状态精简图稿、实际渲染/目视、保留必要错误与恢复反馈；独立提交与PR |
| 本地数据：自动备份与分析保留 | `crates/pomodoro-storage/**`、`docs/contracts/m1-storage-port.md`勘误、`docs/plans/deliveries/automatic-backup.md` | 独立crate、实际SQLite自动备份/恢复读回/保留/导出测试；根Cargo接入由基线任务完成；独立提交与PR |
| M1可运行基线：唯一构建入口与最短流程 | 根`Cargo.toml`/`Cargo.lock`/`build.rs`/`rust-toolchain.toml`、`src/**`、`ui/**`、`tests/**`、`docs/plans/deliveries/m1-foundation.md` | 准备必要Rust工具链，Rust+Slint Windows构建，开始→暂停→继续→完成→保存→重启恢复；接入存储任务的已测试提交，独立PR |

## 按实际修订拆分的平台与视图任务

初版基线与真实Store接入形成检查点后，进一步冻结唯一写入边界：

| 任务 | 唯一写入 | 固定输入与输出 |
| --- | --- | --- |
| Windows活动时钟 | `src/platform.rs`、`docs/plans/deliveries/m1-windows-clock.md` | 输入`4096985`；保持clock_sample签名，提供清醒monotonic_ms及含睡眠boot_elapsed_ms，保留主题/几何帮助函数 |
| Slint响应布局/键盘语义 | `ui/main.slint`、`docs/plans/deliveries/m1-view.md` | 输入`10c1f63`与SVG v2.2；比例布局、无灰度占位、键盘/读屏语义、设置草稿保存/取消 |
| 基线集成与领域/协调 | 根构建入口、`src/domain.rs`、`src/application.rs`、`src/main.rs`、基线测试和交付记录 | ClockSample冻结为monotonic_ms/boot_elapsed_ms/utc_ms/local_date/local_offset_minutes；根Cargo feature唯一维护，接入两个独立增量并验证真实应用 |

SettingsWindow保留现有字段，新增`save-requested(int,int,int,int)`及`cancel-requested()`；四值对应focus/short/long/rounds，范围分别1—180/1—60/1—120分钟及1—12轮。视图按键只改草稿，协调器负责一次事务保存/历史、取消回滚和错误反馈。

独立增量PR以固定`review/m1-checkpoint`分支作清晰的评审基线；最终可运行基线PR堆叠在存储任务之上，并汇总已审查的平台/视图提交。各PR均请求jasonaiseeker，最终集成和合并由用户决定。

共同规则：只读最小任务说明与直接依赖；不读取整段旧角色历史；不覆盖他人工作树。根Cargo/构建/依赖入口唯一写入者为M1基线任务，存储任务维护自身crate manifest。工具链在本地忽略目录准备，实际路径仅本地交接。

Lead复核源提交、实际验证、公开文件与提交历史后推送并分别建PR，邀请`jasonaiseeker`。基线PR在存储PR之上形成明确依赖，PR不自动合并。一次只读审查使用指定提交与必要临时产物；任务交接及集成后按生命周期归档。
