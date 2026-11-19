# Pomodoro

轻量级Windows悬浮番茄钟，目标使用Rust+Slint，覆盖可靠计时、置顶、托盘、Hyper-V工作场景和本地数据。

M0已获用户确认，当前按独立任务/工作树推进M1实现与验证。设计、自动备份、Windows平台、Slint视图和基线集成分别交付评审；运行代码与验证结果以对应PR提交为准。

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev)

- [项目状态与下一步](docs/project-state.md)
- [文档地图](docs/README.md)
- [产品v0.2](docs/product/requirements-v0.2.md)
- [UI v2](docs/design/ui-v2/ui-v2-review.md) · [SVG](docs/design/ui-v2/pomodoro-ui-v2.svg) · [PNG](docs/design/ui-v2/pomodoro-ui-v2-preview.png)
- [契约评审](docs/contracts/review-packet.md)
- [验收计划](docs/acceptance.md) · [评审登记](docs/quality/review-register.md)
- [协作规则](docs/team/README.md) · [历史研究](docs/archive/role-studies/README.md)
- [Notion](https://www.notion.so/41be33444f5f80698ee7d99318343c08) · [Figma](https://www.figma.com/design/4U9TTroE0k4Rkef4K9LIG7/Pomodoro) · [GitHub任务](https://github.com/troywjz/Pomodoro/issues)

Figma不可用时采用可编辑SVG；[精简设计PR #23](https://github.com/troywjz/Pomodoro/pull/23)与[自动备份PR #25](https://github.com/troywjz/Pomodoro/pull/25)已分别提交。M0批准及新增要求见[批准记录](docs/product/approval-20261118.md)。PR合并和正式发布由用户决定。

项目自身源码保持MIT；Slint按已选的[Royalty-free 2.0桌面路线](https://slint.dev/terms-and-conditions#slint-royalty-free-license-2-0)使用，以上为官方提供的公开网页署名徽章。其他依赖保留各自许可与版权信息。
