# M1 可运行基线交接（WIP 检查点）

## 输入与范围

- 起点：`c5a3c6bfb1e19d541575516eaa7d8068c4a52c90`。
- 本分支：`task/m1-foundation`。
- 已接入存储任务提交：`37e0b549c75d0de31fcd8e06bd3cf1c019560189`，后续修订 `462ad46d69159420ac6aec01453747df9cdd0954` 已合并；公开接口含 `31e1a8531eb5e9b9a3426f4c2c77173fa57e2366` 与 `b80cc6c` 勘误。
- UI 输入：精简浅深 SVG `4ff538d6995bc0b22b5c2039acd9e57203284b87`，采用 v2.2 的主计时窗、圆环在读数前、默认浅色跟随系统、展开/收起同宽。
- 路径边界：根 Cargo/build 入口、`src/**`、`ui/**`及本交付记录；不修改 `crates/pomodoro-storage/**`。

## 当前实现

- Rust 1.100.0 MSVC 工具链、Slint 1.19.0；Slint 启用 Winit 与 Skia CPU renderer。工具链仅安装到被忽略的本机目录，进程代理也只在本机下载/构建命令使用。
- 纯计时模型含运行、暂停、恢复、完成、手动下一段及四轮长休序列；配置默认为专注/短休/长休 25/5/15 分钟、每四轮长休。
- `Application` 接入真实 SQLite `Store`，恢复 settings/checkpoint，提交设置历史、session 事件、checkpoint、revision 和回执；应用运行期间每 10 秒尝试保存 checkpoint、每 30 秒调用自动备份检查，终结记录走存储自动备份路径。
- 活动计时更新使用注入的清醒单调时间；领域现已增加含睡眠启动时间差和时钟回退检测，检测后写入 recovery checkpoint 并要求用户选择恢复方式。根 Cargo 已启用 Windows `WindowsProgramming` API feature，平台适配等待独立短任务。
- 数据目录默认为 Windows `%LOCALAPPDATA%\Pomodoro`；测试可通过 `POMODORO_DATA_DIR` 指向独立临时目录。
- About 窗口包含 Slint 署名；主计时窗显示保存状态和失败反馈。

## 实际验证

- 在加入真实存储依赖、扩展清醒/启动双时钟字段之前，`cargo test application::tests::defaults_and_settings_are_saved_with_a_receipt -- --exact`：1 项通过。
- 同一阶段 `cargo test application::tests::pause_resume_and_restart_preserve_checkpoint_without_offline_time -- --exact`：1 项通过。
- 同一阶段 `cargo check`：通过。Slint 首次编译下载 Skia 官方预编译依赖时使用了本机进程代理。
- 初始纯计时基线提交 `ac78f94be82017ae52eb42900777381bc5ca2345` 的 `cargo test --no-fail-fast`：8 项通过；该结果覆盖存储接入前的领域实现。
- `cargo test --manifest-path crates/pomodoro-storage/Cargo.toml -- --nocapture` 在合并 `462ad46` 后：8 项通过，包含备份/导出保护、失败重试、revision、历史保留及重启读回。
- 根 crate 两个真实存储集成项在新增双时钟字段前分别通过：默认配置/回执 1 项，暂停/恢复与重启读回 1 项。

## 未完成与当前验证边界

- 本检查点新增的 `ClockSample.boot_elapsed_ms` 尚未与 Windows 平台适配器合并；平台任务需提供清醒单调毫秒与含睡眠启动毫秒。当前工作树的最新增量尚未编译。
- 存储最终文档提交 `1ed17f74c31e87dd3292eb82d62c9f945c47c32` 已 merge；实现修订 `462ad46` 已在独立存储测试中通过。
- Rust 侧已绑定设置 Save/Cancel 回调：保存接收四个值、成功后提交配置快照并关闭，保存失败保留窗口；取消回退到已保存配置。新 Slint 回调声明待 `task/m1-view-responsive` 合并，根 crate 编译安排在平台字段与视图回调合并后。
- 小屏比例布局、键盘/读屏操作由 UI 短任务修订 `ui/main.slint`；当前窗口 PNG 和可访问性结果待复核。
- 本检查点未执行完整 `cargo test`、存储集成 `cargo build`、Windows GUI 启动/退出 smoke、窗口截图、系统睡眠、Hyper-V 或多屏/DPI 实机验证。
- 未推送远程分支、未创建 PR、未请求评审。

## 下游接口约定

领域与应用使用 `ClockSample { monotonic_ms, boot_elapsed_ms, utc_ms, local_date, local_offset_minutes }`。`monotonic_ms` 表示清醒活动时间；`boot_elapsed_ms` 表示含睡眠的启动累计时间。平台适配器负责填充两者，应用以两者的时间差识别系统睡眠；计时不扣除睡眠时间，发生睡眠后转入 recovery。
