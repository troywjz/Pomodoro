# M1 Slint 视图交付记录

输入基线：`10c1f63a1255d76db4b20f58de8c202cdbd1bdab`；设计输入：`4ff538d6995bc0b22b5c2039acd9e57203284b87` 中的 UI v2.2 SVG。

## 本次修改

- 保留 `PomodoroWindow` 现有属性和回调，删除状态与保存标签组成的灰色说明行。错误仍由独立 feedback 展示。
- 按 `target-width / 192px` 缩放主窗展开/收起态布局间距、环形计时器尺寸/线宽、文字和操作按钮高度；按钮获得焦点时显示边框。
- `ActionButton` 保留矩形视觉与指针点击，使用 Slint 1.19 `FocusScope` 接收 Tab 焦点、Enter/Space 激活，并标注 button 角色、标签和禁用状态。
- 保留设置属性及旧 delta/close 回调声明。公开草稿属性名为 `draft-focus`、`draft-short`、`draft-long`、`draft-rounds`。设置操作修改本地草稿，范围为专注 1–180、短休 1–60、长休 1–120、轮数 1–12；保存只发出 `save-requested(int,int,int,int)`，由 Rust 保存成功后隐藏窗口；取消只发出 `cancel-requested()`，由 Rust 回滚并隐藏窗口。
- 设置窗口高度为 350px，容纳 26px 标题、四行设置、保存/取消按钮、纵向间距、内边距和边框。
- Rust 绑定保持冻结，本任务不接线新保存回调。集成方需每次打开设置时同步已保存值至四个 draft 属性，并接入保存回调。同步应用设置属性本身不会重置草稿。

## 几何核算

按 10% 屏宽规则，1366px 屏幕主窗目标宽 137 DIP；192px 基准展开/收起尺寸按比例得到 213.75/62.71 DIP。其余核算：170.67→266.67/78.22 DIP，192→300/88 DIP，204.8→320/93.87 DIP，256→400/117.33 DIP。以上为比例计算，不是窗口实机测量。

## 验证状态

`cargo check --locked` 使用隔离 Rust 1.100 环境及当前工作树绝对路径 `.local/target` 成功通过。Skia 与 Slint 1.19 编译通过；保留已有 `restore_for_recovery` 死代码和未使用 serde_json/sha2 依赖警告。静态 `git diff --check` 通过。真实窗口布局与键盘实测由基线集成验证。
