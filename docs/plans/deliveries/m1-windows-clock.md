# M1 Windows 时钟交付

## 范围

实现 `clock_sample(origin: Instant) -> ClockSample` 的 Windows 平台采样，并保留现有显示器中心点/DPI、10%宽度、主题辅助函数。Windows 活动时间来自 `QueryUnbiasedInterruptTimePrecise`，不计入睡眠/休眠；含睡眠启动时间来自 `GetTickCount64`。两者以同一进程首次采样值为基线并转换为毫秒。非 Windows 兼容分支使用 `Instant`，两种读数相同，因此该分支不模拟睡眠差额。

首个 Windows 样本建立进程基线；调用方应在创建计时原点后立即采样。系统计数器 API 没有可报告的失败返回值。饱和减法使相对读数保持非负；若计数器回退导致后续样本下降，域/app 比较样本后进入 Recovery。

## 验证

- 验证输入提交：`feef7ffafa972159592ea019773900c697a154fb`。
- Lead 在独立验证工作树以该提交为源码运行 `cargo check --locked --features windows-sys/Win32_System_SystemInformation,windows-sys/Win32_System_WindowsProgramming`，5.92 秒成功退出 0；该结果属于 Windows 目标类型与接口静态检查。编译 warning 为 Windows 分支原 `origin` 参数未使用和既有 export 方法未使用。此提交将参数名改为 `_origin`，接口类型与语义不变。
- 本轮 PowerShell P/Invoke 直接读取 Windows API 两次，中间等待约120毫秒：活动计数差约126.06毫秒，启动计数差125毫秒。此检查确认正常运行时两种原生读数可读取并递增；不包含睡眠行为验证。
- Microsoft Learn 确认 unbiased interrupt time 排除睡眠/休眠，`GetTickCount64` 包含启动以来时间。本机 windows-sys 0.61.2 源码确认 API 位于 `Win32_System_WindowsProgramming` 与 `Win32_System_SystemInformation`，调用签名符合绑定。
- 非 Windows 单元检查覆盖毫秒单位、连续样本单调不减和兼容分支，尚未运行。

## 边界

独立 `cargo check` 验证 Windows 目标的类型与接口。完整可执行链接和 GUI 由基线 build/smoke 记录验证。此任务没有触发真实睡眠/休眠，也没有验证可控睡眠读数或 Hyper-V 电源行为；这些属于后续实机验证。

## 集成建议

根 Cargo 的 windows-sys features 需包含 `Win32_System_SystemInformation` 和 `Win32_System_WindowsProgramming`。基线已有后者；前者是本实现新增要求。独立验证通过命令行启用这两个 feature，本提交不改 Cargo 清单。
