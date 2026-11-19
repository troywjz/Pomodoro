# M1 Windows 时钟交付

## 范围

实现 `clock_sample(origin: Instant) -> ClockSample` 的 Windows 平台采样，并保留现有显示器中心点/DPI、10%宽度、主题辅助函数。Windows 活动时间来自 `QueryUnbiasedInterruptTimePrecise`，不计入睡眠/休眠；含睡眠启动时间来自 `GetTickCount64`。两者以同一进程首次采样值为基线并转换为毫秒。非 Windows 兼容分支使用 `Instant`，两种读数相同，因此该分支不模拟睡眠差额。

首个 Windows 样本建立进程基线；调用方应在创建计时原点后立即采样。系统计数器 API 没有可报告的失败返回值。计数器回退时采用饱和差值，保持读数单调不减；域层负责按两种读数差异触发恢复业务。

## 验证

- Microsoft Learn 确认 unbiased interrupt time 排除睡眠/休眠，`GetTickCount64` 包含启动以来时间。
- 本机 windows-sys 0.61.2 源码确认 API 位于 `Win32_System_WindowsProgramming` 与 `Win32_System_SystemInformation`；调用签名符合绑定。
- Windows `cargo check --locked` 已启动并进入 `skia-bindings` 构建；本轮未观察到最终退出结果。
- 非 Windows 测试覆盖毫秒单位、连续样本单调不减和兼容分支，尚未运行。
- `git diff --check` 通过。

## 边界

交叉编译验证 Windows 目标编译与链接声明。此任务没有触发真实睡眠/休眠，也没有验证 Windows 机器正常读数、可控睡眠读数或 Hyper-V 电源行为；这些属于后续实机验证。

## 集成建议

根 Cargo 的 windows-sys features 需包含 `Win32_System_SystemInformation` 和 `Win32_System_WindowsProgramming`。基线已有后者；前者是本实现新增要求。独立验证可使用 `--features windows-sys/Win32_System_SystemInformation,windows-sys/Win32_System_WindowsProgramming`，本提交不改 Cargo 清单。
