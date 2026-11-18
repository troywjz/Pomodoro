# M0 需求与原型评审包

状态：候选交付，待独立审查与用户决定。业务实现、Cargo 工程和实现型试验保持待启动。

## 交付文件

- `docs/product/m0-behavior-decisions.md`：D-01 中断时长统计、D-02 清理确认/取消顺序、D-03 导出边界、D-04 idle/recovery 推荐及备选影响、验收样例；全部待用户批准。
- `docs/acceptance.md`：细化 AC-43—46 的状态覆盖与执行阶段，保留编号，所有记录仍为待执行。
- `docs/design/ui-v2/ui-v2-state-matrix.svg`：新增可编辑候选状态图，含 Light 主窗状态、Dark 设置/确认/错误/recovery、按钮状态、键盘语义与 5 种屏幕/DPI 算例。
- `docs/design/ui-v2/ui-v2-state-matrix-preview.png`：状态图的浏览器渲染预览。
- `docs/design/ui-v2/ui-v2-review.md`：记录新增证据、主题映射、Figma 结果和待验收边界。
- 原 `pomodoro-ui-v2.svg` 与 `pomodoro-ui-v2-preview.png` 保留未改。

## 真实验证

- Microsoft Edge headless Chromium 对 SVG 实际渲染 PNG，随后对完整预览进行目视检查；排版、卡片边界、按钮状态样例、焦点环与尺寸矩阵均可见，SVG XML 解析通过。
- 宽度计算复核：1366@100%=137/137dip；1920@100%=192/192dip；2560@125%=256/204.8dip；3840@150%=384/256dip；混合示例 1920@100%=192/192dip，2560@150%=256/170.67dip。
- `git diff --check` 通过。该检查覆盖文档/规格文件，不含应用构建、交互运行或多显示器实机验证。
- Figma 文件 `4U9TTroE0k4Rkef4K9LIG7` 的原生通道检查受阻，实际返回：`You've reached the Figma MCP tool call limit on the Starter plan. Upgrade your plan for more tool calls`。本轮没有写入 Figma v2 页面。
- Notion 现有“产品需求与范围 · v0.2”页面已追加 M0 候选补充，原文保留，页面状态仍为待审批；含四项产品推荐摘要及 Figma 限额状态。

## 推荐用户决定

建议用户审阅并逐项决定 D-01—D-04。产品建议为：中断段贡献实际投入时长但不增加完成轮数；清理按摘要、结束活动段确认、最终确认、备份验证、删除的次序执行；JSON 导出带活动快照而 CSV 面向已提交历史分析；idle 无目标/已选目标分开，recovery 等待用户选择且不扣离线时间。AC-43—46 及设计比例仍是候选验收定义。

## 未决项与下游

- Figma 原生独立页面与 Light/Dark 变量同步、原生画板截图待通道可用后完成。
- 极窄屏文案优先级、显示器边界跨屏时序与逻辑尺寸量化仍需产品/架构裁决。
- 用户批准行为语义后，Lead 将 D-01—D-04 的批准版本转交契约任务；契约任务维护字段、协议和事务时序。
- 真实应用的主题回退、辅助技术、点击/键盘、多显示器/DPI/拔屏仍待未来实现与独立测试。

## 提交

- 产物提交：`551aeb6`（M0 候选产物提交）。
- 本交付记录随最终包提交；实际 HEAD 由 Git 提交历史确定。
