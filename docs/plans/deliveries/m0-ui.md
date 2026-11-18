# M0 需求与原型评审包

状态：候选交付 v2.1，待独立审查与用户决定。业务实现、Cargo 工程和实现型试验保持待启动。

## 交付文件

- `docs/product/m0-behavior-decisions.md`：D-01 中断时长统计、D-02 清理既定输入与新增节点语义、D-03 导出范围且明确无导入/恢复承诺、D-04 idle/recovery 推荐及备选影响、验收样例；全部候选待用户批准。继承 requirements.md §8 的未决提案与未来实测清单。
- `docs/acceptance.md`：细化 AC-43—46 的状态覆盖与执行阶段，保留编号，所有记录仍为待执行。
- `docs/design/ui-v2/ui-v2-states-light.svg` 与 `ui-v2-states-dark.svg`：独立可编辑主题状态图；每张均含无目标 idle、已选目标 idle、running、paused、completed、设置、清理确认、错误和 recovery。
- 对应 `ui-v2-states-light-preview.png`、`ui-v2-states-dark-preview.png`：Edge headless 实际渲染预览。
- `ui-v2-state-matrix.svg/.png` 保留为上一候选版本；不作为 v2.1 当前验收依据。最初 `pomodoro-ui-v2.svg/.png` 继续保留。
- `docs/design/ui-v2/ui-v2-review.md`：记录新增证据、主题映射、Figma 结果和待验收边界。
- v2.1 返修把每个产品窗与外部虚线评审卡片区分；running/paused/recovery 弧按 `1458/1500` 及对应圆周计算。按钮样例明确显示各主题的 default/hover/pressed/disabled/focus，并按矩形中心锚定文案。

## 真实验证

- Microsoft Edge headless Chromium 对 SVG 实际渲染 PNG，随后对完整预览进行目视检查；排版、卡片边界、按钮状态样例、焦点环与尺寸矩阵均可见，SVG XML 解析通过。
- 宽度计算复核：1366@100%=137/137dip；1920@100%=192/192dip；2560@125%=256/204.8dip；3840@150%=384/256dip；混合示例 1920@100%=192/192dip，2560@150%=256/170.67dip。
- 弧长计算复核：remaining/target = 1458/1500 = 0.972；半径 29 周长/弧/余弧 = 182.212374/177.110427/5.101946；半径 25 = 157.079633/152.681403/4.398230。
- 产品窗口几何：展开 192×300 dip，收起 192×88 dip；图中均按 1.25×坐标绘制为 240×375 与 240×110。虚线 520×405 是评审注释框，不是产品窗口。按钮标签锚于各自矩形中心，水平居中且 `dominant-baseline=middle`。
- D-02 依据 `docs/requirements.md` §6 与 AC-23/24/33 分开清历史和永久清除。清历史保留配置/活动；取消与备份失败保留原状态，失败提示结束当前操作并重新发起完整流程。永久清除最终确认前取消保持所有数据/activity；确认后结束活动并删除应用数据/备份。
- D-03 将 JSON/CSV 定位为数据导出；MVP 导入/迁移入口为关闭状态，应用恢复使用应用内备份。已在 Notion 现有需求页更正相关候选摘要，并保持待审批。
- 继承 `docs/requirements.md` §8 的全部待决提案已分类列入“沿用提案 / 明确审批 / 后续实测”；四项已确定 UI 方向单列保留，其余候选仍待批准。
- `git diff --check` 通过。该检查覆盖文档/规格文件，不含应用构建、交互运行或多显示器实机验证。
- Figma 文件 `4U9TTroE0k4Rkef4K9LIG7` 的原生通道检查受阻，实际返回：`You've reached the Figma MCP tool call limit on the Starter plan. Upgrade your plan for more tool calls`。本轮没有写入 Figma v2 页面。
- Notion 现有“产品需求与范围 · v0.2”页面已追加 M0 候选补充，原文保留，页面状态仍为待审批；含四项产品推荐摘要及 Figma 限额状态。

## 推荐用户决定

建议用户审阅并逐项决定 D-01—D-04。产品建议为：中断段贡献实际投入时长但不增加完成轮数；清空历史保留活动段，永久清除在最终确认后结束活动并清除应用管理数据；取消及备份失败结果均保留原数据/状态并提供明确回路；JSON/CSV 作为数据导出，应用恢复使用应用内备份流程；idle 无目标/已选目标分开，recovery 等待用户选择且不扣离线时间。`requirements.md` §8 的默认时长/范围、统计、休眠、提示音、退出、保留、今日指标和资源机器等原有待决项继续待用户批准；资源预算、显示器/DPI、主题与无障碍需未来实测。AC-43—46 及设计比例仍为候选验收定义。

## 未决项与下游

- Figma 原生独立页面与 Light/Dark 变量同步、原生画板截图待通道可用后完成。
- 极窄屏文案优先级、显示器边界跨屏时序与逻辑尺寸量化仍需产品/架构裁决。
- 用户批准行为语义后，Lead 将 D-01—D-04 的批准版本转交契约任务；契约任务维护字段、协议和事务时序。
- 真实应用的主题回退、辅助技术、点击/键盘、多显示器/DPI/拔屏仍待未来实现与独立测试。

## 提交

- 产物提交：`551aeb6`（M0 候选产物提交）。
- Reviewer/Lead 返修：`68c0d6706060c3b9dc3b4b99f48e4e310bf0c808`（v2.1 Light/Dark 状态图、D-02/D-03 与 AC 修订）。
- 本交付记录随最终包提交；实际 HEAD 由 Git 提交历史确定。
