# UI精简 v2.2 交付

- 基线：`c5a3c6bfb1e19d541575516eaa7d8068c4a52c90`
- 范围：产品与交互设计组；SVG规格增量，不含Slint业务实现。
- 批准依据：`docs/product/approval-20261118.md`及M1派发说明。

## 产物

- `docs/design/ui-v2/ui-v2-states-light-v22.svg`与`ui-v2-states-light-v22-preview.png`
- `docs/design/ui-v2/ui-v2-states-dark-v22.svg`与`ui-v2-states-dark-v22-preview.png`
- v2.1源稿及PNG保持原样。

两主题均保留九状态：无目标、已选目标、运行、暂停、完成、设置、清理确认、备份错误和恢复处理。主窗移除目标/读数解释类辅助小字；必要清理失败和恢复处置状态保留。评审状态标签仍在产品窗外。尺寸比例、按钮中心锚点及97.2%圆弧值延续v2.1。

## 验证

- 使用Microsoft Edge headless Chromium从可编辑SVG实际生成浅色与深色PNG预览，并逐图检查九状态、产品窗边界、必要消息和无裁切情况。
- SVG几何复核：展开`240×375` SVG像素对应`192×300`逻辑像素，收起`240×110`对应`192×88`；共享宽度。按钮中心锚点保持`(x + width/2, y + height/2)`。
- running/paused主圆弧`177.110427 / 182.212374 = 97.2%`；recovery圆弧`152.681403 / 157.079633 = 97.2%`。
- 本轮验证范围为设计稿渲染与目视布局；应用交互和Windows窗口验收在实现阶段记录。

## 后续

- Notion产品页按原批准状态记录本轮精简增量。
- Lead完成推送前敏感信息复核，再创建独立PR并邀请`jasonaiseeker`评审；本交付session不推送。
- 建议PR标题：`Design: simplify Pomodoro UI states in v2.2`
- 建议PR描述：`Adds editable Light/Dark nine-state v2.2 SVGs and rendered previews. Removes explanatory gray microcopy from product windows while preserving actionable backup and recovery feedback, state labels outside product boundaries, shared expanded/collapsed width, centered button anchors, and 97.2% progress geometry. Adds design acceptance criterion AC-47. This PR delivers SVG prototype evidence; application interaction and runtime behavior are recorded during implementation validation.`
