# Issue #35 倒计时圆弧修复交付

## 执行者身份

- 角色：执行 session，负责 Slint 视图缺陷修复；本轮同时承担集成与验收。按[工作入口](../../../AGENTS.md)，Lead 使用用户当前选定模型，本次即该会话模型。
- 运行形态：WorkBuddy Agent 会话（Windows 桌面环境），模型 `DeepSeek-V4.1-Flash`。
- 起始基线：`8b4a1ca`；执行分支 `fix/issue-35-progress-ring`；修复提交 `178b5b7`；合并提交 `5699719`。
- 记录时间：2026-11-21。

## 范围

- 关联 Issue：[#35 剩余时间较少时倒计时圆弧显示为斜线](https://github.com/troywjz/Pomodoro/issues/35)。
- 集成单：[PR #36](https://github.com/troywjz/Pomodoro/pull/36)，已合并到 `main`。
- 修改路径：`ui/main.slint`、`src/main.rs`（新增测试模块）。
- 未触及：计时模型、应用协调器、存储与备份、窗口与平台层。

## 问题与根因

剩余时间较少时，进度描边呈现为横穿圆环内部的斜线，而不是贴合圆轨的短圆弧。

`ProgressRing` 中圆轨与进度两层 `Path` 都未指定 `viewbox`。Slint 在缺少 `viewbox` 时按路径自身的包围盒缩放，剩余进度越短，小圆弧的包围盒越小，被拉伸铺满整个 52 DIP 区域，视觉上退化为弦线。进度比例、角度计算与计时逻辑本身正确，问题只在渲染坐标系。

判定依据：使用同版本 `slint-viewer 1.19.0` 对未修改的 `main` 离屏渲染，稳定复现同一斜线，排除截图误差与真机特例。

## 实现内容

- `ui/main.slint`：为两层 `Path` 固定 `viewbox-width` / `viewbox-height`（展开 52、收起 44），与既有路径坐标 `M 26 2 A 24 24 …`、`M 22 2 A 20 20 …` 对齐。组件自身尺寸保持不变，缩放只由 `layout-scale` 决定。
- `src/main.rs`：新增 `progress_path_tests`，覆盖 `0`、负值、`1`、超出 `1`（`1.1`）、用户反馈场景 `93/1500`，以及大弧标志在 `0.5` 前后的切换。

选择固定坐标系而非改写路径计算，原因是缺陷属于视图坐标约定缺失，修改 `progress_path` 会掩盖问题并把回归风险转嫁给计时与界面两侧。

## 技术依赖

- Slint 1.19.0，`Path` 元素的 `viewbox-*` 属性。
- 无新增第三方依赖；测试使用标准 `#[cfg(test)]` 模块。

## 验证记录

- `cargo test --locked --all-targets`：22 项通过，0 项失败，含新增 3 项路径测试；合并后的 `main` 上重跑同样 22 项通过。
- `cargo fmt --all -- --check`：通过。
- `slint-viewer 1.19.0 --check`：UI 编译通过。
- 实际渲染：修复前复现斜线；修复后在同一用户场景下为圆轨顶端的小圆弧。另覆盖展开/收起、深/浅主题、`0%`、`50%`、`99.9%`、`100%`。
- 环境：本机无全局 Rust 工具链，测试使用临时目录内按 `rust-toolchain.toml` 安装的 1.100.0 MSVC 工具链。Skia 预编译包下载需经本机代理；直连失败时构建脚本会回落全量编译并因缺少 LLVM 中止，这是环境问题，不是代码缺陷。

## 证据

- 修复前后对比：<https://github.com/user-attachments/assets/5e11ed8c-ae90-4312-8f23-231f4fbf9ecb>
- 六组渲染矩阵（浅色小进度、深色收起小进度、深色 50%、浅色 99.9%、深色 0%、浅色 100%）：<https://github.com/user-attachments/assets/bceacc2c-f47a-4300-b5d5-c3b5005274a1>
- 结论与验证说明已回写 Issue #35。

## 未决问题与真实阻塞

- 未在真机 Windows 上手动跑完一轮计时至尾声的完整流程；视觉证据来自同版本渲染器离屏渲染，二者渲染管线一致但不等同实机验收。
- 修复分支基于 `8b4a1ca` 建立，期间 `main` 已推进到 v0.2.0 与 JSONL 存储（`5de77a5`）。合并后已在最新 `main` 上重跑完整测试并重新渲染确认，但该基线差异需要在后续任务中留意。
- `crates/pomodoro-storage` 不是 workspace 成员，其独立测试未纳入本次运行。

## 交接边界与维护提醒

- 两层 `Path` 的 `viewbox` 不可删除，删除即回到按包围盒缩放的旧行为。
- 调整圆环尺寸只需改组件 `width` / `height`，`viewbox` 等比缩放即可；若改动路径坐标，必须同步 `viewbox`，否则再次出现同类变形。
- 修复工作树与远端 `fix/issue-35-progress-ring` 分支已无待办，可归档；工作树删除使用仓库原生 `git worktree remove`，不在主目录内直接删目录。
- 下游无接口或行为变化：Slint 属性、Rust 回调与存储契约均未改动。
