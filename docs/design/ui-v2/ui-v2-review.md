# Pomodoro UI v2 设计评审稿

- 状态：草案，待用户评审；未同步至 Figma。
- 来源：GitHub Issue [#7](https://github.com/troywjz/Pomodoro/issues/7)、`docs/ui-revision-v2.md`。
- 评审附件：[可编辑 SVG 原型](pomodoro-ui-v2.svg)。SVG 中的画板、文字、圆圈、路径、按钮均为独立矢量元素，可在 Figma 中导入后继续编辑。
- Figma 目标文件：<https://www.figma.com/design/4U9TTroE0k4Rkef4K9LIG7/Pomodoro>。Figma MCP 当前 Starter 计划返回调用额度耗尽；`whoami` 显示关联计划为 Starter。Figma 读取/写入尚未执行，额度恢复后待导入到独立 `UI v2 · Review draft` 页面，保留现有 v1 页面。

## 设计决定候选

1. 浅色为首次运行及系统主题读取失败时的回退主题。系统主题为深色时应用深色变量；运行期间系统主题变化后切换颜色，不重置计时或窗口状态。
2. 主窗口宽度由当前所在显示器的物理像素宽度决定：`Wpx = round_half_up(display_width_px / 10)`；逻辑宽度为 `Wdip = Wpx / (DPI / 96)`。正数 half-up 定义为 `floor(value + 0.5)`。展开和收起的宽度都等于同一个 W。SVG 示范使用 192 logical px 宽，作为 1920 px@100% 规格样例。
3. 展开稿按 `192 × 300` logical px 绘制，收起稿按 `192 × 88` logical px 绘制。它们是结构比例样例，真实高度随 W 等比缩放。圆圈直径、字体、间距、圆角、按钮高度均以 W 为基准；窄屏按 10% 规则自然收缩，不引入额外宽度上下限。
4. 倒计时圆圈固定置于时间数字前，弧长表达 `remaining / target`，运行时更新；暂停时冻结，到期时归零。示范显示 24:18 / 25:00，剩余比率为 `1458 / 1500 = 0.972`；半径 34 的进度弧与余弧为 207.646708 / 5.981592，半径 26 对应弧长为 158.788659 / 4.574159。两种画面均使用 97.2% 比例。数字仍作为准确时间读数，并提供“剩余时间”可访问名称及阶段文字。
5. 设置和展开/收起是分离入口。主操作按钮的标签在按钮矩形内水平及垂直居中；按钮全矩形属于点击和键盘焦点热区。SVG 使用中文界面文案：专注、暂停、重置、结束、设置、收起/展开；专注阶段以“结束”呈现中止当前段，跳过动作保留给休息阶段。各可交互状态在下方矩阵定义。

## 显示规格矩阵

物理宽度使用 `round_half_up(屏幕物理宽度 / 10)`；逻辑宽度使用 `Wpx / 缩放因子`。Slint 逻辑尺寸的最终量化方式仍须前端接口冻结。下列显示器宽度按常见面板规格给出，验收应记录 Windows 实际报告值。

| 情况 | 显示器物理尺寸 | 缩放 | Wpx 目标 | Wdip 候选 | 主题/窗口矩阵 |
|---|---:|---:|---:|---:|---|
| A | 1366×768 | 100% | 137 | 137 | Light/Dark × Expanded/Collapsed |
| B | 1920×1080 | 100% | 192 | 192 | Light/Dark × Expanded/Collapsed；SVG 基准 |
| C | 2560×1440 | 125% | 256 | 204.8 | Light/Dark × Expanded/Collapsed |
| D | 3840×2160 | 150% | 384 | 256 | Light/Dark × Expanded/Collapsed |
| E | 混合 DPI 双屏 | 例如 1920×1080@100% + 2560×1440@150% | 192 / 256 | 192 / 170.67 | 在两屏分别验证两主题与两种高度；跨屏重算并回到可见工作区 |

缩放因子定义 `s = DPI / 96`；上表 E 的高 DPI 屏逻辑宽候选按 `256 / 1.5 = 170.67`。窗口中心所在显示器作为当前显示器；移出所有显示器时回到主显示器可见区域。跨屏、缩放变化或断开屏幕后，按新的显示器与 DPI 重新计算尺寸。

## 交互和状态覆盖

| 区域 | 状态 | 设计行为与检查点 |
|---|---|---|
| 主按钮 | Default / Hover / Pressed / Focus / Disabled | 标签在完整矩形中精确居中；完整矩形接受鼠标点击、Tab 聚焦与 Enter/Space 激活；禁用态对比仍清楚 |
| 圆圈与计时 | Idle / Running / Paused / Completed | Idle 为完整轨道；Running 按剩余比例更新；Paused 弧线冻结；Completed 弧线归零且显示完成语义 |
| 系统主题 | 首次加载 / Light / Dark / 运行时切换 / 查询失败 | 初始回退浅色；成功后跟随系统；切换只更新主题变量；失败保留浅色 |
| 窗口 | Expanded / Collapsed / 移屏 / DPI 变化 | 宽度恒定为对应当前显示器的 10%；高度按状态变化；设置入口保持独立可达 |
| 辅助信息 | 错误 / 无障碍读屏 | 错误使用主题语义色且带文本；计时器名称含“剩余时间”、阶段与运行状态，圆弧不作为唯一信息 |

## 颜色变量候选

在 Figma Color collection 建 Light、Dark 两种 mode。页面底色、主文字、次文字、边框、操作按钮、按钮文字、进度弧、轨道和错误状态都绑定语义变量。建议语义键：`color/bg/surface`、`color/text/primary`、`color/text/secondary`、`color/border/default`、`color/action/primary`、`color/action/primary-text`、`color/progress/active`、`color/progress/track`、`color/status/error`。SVG 是可编辑评审稿；实际 Figma 变量绑定待同步时建立并检查。

## 目视检查记录

- 检查材料：本目录 SVG 原型源码与四个画板几何数据。
- Chrome headless 实际渲染预览：[pomodoro-ui-v2-preview.png](pomodoro-ui-v2-preview.png)，1760×1000 PNG。已通过图像预览逐画板目视检查；四种主题/窗口状态完整显示，画板之间无碰撞，文字未截断。
- 最新修正版四个主按钮文字水平、垂直居中；收起态按钮与时间读数保留间距。设置与展开/收起入口可见且分离。
- SVG 画布：1760×1000；四个窗口展示框均宽 384px（以 2×示意 192 logical px），展开高 600px、收起高 176px（2×示意）；窗口边界之间留 40px 间距。正文 28–40px，按钮 28px，时间 60px，辅助标签 20px，按 2×对应逻辑尺寸绘制。
- 主按钮使用 `text-anchor="middle"`、`dominant-baseline="middle"`，文本坐标对齐矩形中心；按钮完整矩形作为点击和键盘热区。热区说明留在本评审文档，不出现在产品画面中。
- 首版渲染经 Lead 评审发现圆弧比例不符、字体未按 2×示意及英文占位文案；已修正。当前 PNG 预览对应最新版本，旧图不作当前验收证据。SVG 浏览器渲染与画板目视检查已完成；Figma 原生画板截图与真实多显示器/DPI 实机检查待 Figma MCP 额度恢复及 Windows 交互原型评审后执行。

## 依赖与待冻结字段

- 前端：`Geometry { display_physical_width_px, dpi, scale, width_px, width_dip }`；状态切换输入 `expanded/collapsed`；主题输入 `system_light/system_dark/fallback_light`。
- 窗口适配：当前显示器选择、DPI 读取、跨屏重新布局、主屏回退和可见工作区修正。
- 计时：阶段目标值、剩余值、暂停/完成态快照；UI 圆弧仅为剩余值的可视化。
- 待冻结：逻辑坐标量化时机、极窄屏文字折行/隐藏次级信息顺序、展开高度系数、完成态主按钮标签、错误色的最终无障碍对比值。

## 验收状态

- SVG 图层可编辑、四种主题/窗口画板齐备：已产出本地评审附件；首版按 Lead 目视评审意见修订，当前版为最新验收版本。
- 至少五种显示规格可追溯矩阵：已列规格和计算结果，E 项显示器组合为候选实机配置。
- Figma 独立页面与变量：待额度恢复，未写入。
- 主流程截图实际渲染、字体/bounds/重叠目视复核：最新修正版 SVG 浏览器渲染检查已完成；Figma 原生截图与多显示器实机检查待执行。
- 独立测试角色审查：待 team/testing-v2 读取并记录。

## M0 全状态候选补充

- 新增可编辑状态矩阵：[ui-v2-state-matrix.svg](ui-v2-state-matrix.svg)。浏览器渲染预览：[ui-v2-state-matrix-preview.png](ui-v2-state-matrix-preview.png)。原四画板 SVG/PNG 保留为输入，不覆盖。
- 候选矩阵覆盖 Light 主窗（无目标 idle、running、paused、completed）及 Dark 设置、清理确认、备份错误、recovery；界面结构以 Light/Dark 语义 token 配对，系统查询成功跟随系统，查询前和查询失败使用 Light。Light 弹窗/恢复与 Dark 主窗的几何沿用同一组件与状态规则，主题变量见下方映射。
- 主题映射覆盖主窗、设置、确认、错误、圆圈、按钮、文字、焦点环；失败消息同时说明操作结果和下一步。错误色使用文字与边框/背景共同表达，不单独依赖颜色。
- 无目标 idle 使用空轨道与“选择阶段”；已选目标仍为 idle，显示满轨和“开始”；recovery 在用户选择前冻结。读屏名称组合剩余时间、阶段、精确读数和运行状态，圆弧为辅助视觉信息。
- 全按钮矩形是鼠标热区和键盘焦点框；Tab 按视觉顺序移动，Enter/Space 激活焦点按钮；默认、悬停、按下、焦点和禁用状态按相同标签中心定位。设置与展开/收起保持独立入口。
- 显示矩阵示范物理宽/逻辑宽：137/137、192/192、256/204.8、384/256；混合双屏 192/192 ↔ 256/170.67。展开与收起同宽；高度及内容变化。物理宽按屏幕物理像素除以 10 后 half-up，再按 DPI 转逻辑尺寸。
- 本地 SVG 由 Microsoft Edge headless Chromium 渲染为 PNG，并在图像预览中检查布局；这是规格原型渲染。Figma MCP 当前实际返回 `You've reached the Figma MCP tool call limit on the Starter plan. Upgrade your plan for more tool calls`，所以本轮未写入 Figma。Figma 原生画板、变量绑定与实际多显示器/DPI应用验证仍待完成。
- 对照产品候选行为见 [M0 行为决策](../../product/m0-behavior-decisions.md)。上述推荐和状态均待用户批准；交付内容为规格候选，未进行应用实现或运行验证。
