---
lang: zh-hans
direction: ltr
source: docs/portal/docs/intro.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# 欢迎来到 SORA Nexus 开发者门户

SORA Nexus 开发者门户捆绑了交互式文档、SDK
Nexus 运算符和 Hyperledger Iroha 的教程和 API 参考
贡献者。它通过提供实践指南来补充主要文档网站
并直接从该存储库生成规格。登陆页面现在带有
主题 Norito/SoraFS 入口点、签名的 OpenAPI 快照和专用
Norito 流媒体参考，以便贡献者可以找到流媒体控制平面
无需挖掘根规范即可签订合同。

## 你可以在这里做什么

- **学习 Norito** – 从概述和快速入门开始了解
  序列化模型和字节码工具。
- **Bootstrap SDK** – 立即关注 JavaScript 和 Rust 快速入门；蟒蛇，
  随着食谱迁移，Swift 和 Android 指南将加入其中。
- **浏览 API 参考** – Torii OpenAPI 页面呈现最新的 REST
  规范和配置表链接回规范的 Markdown
  来源。
- **准备部署** – 操作运行手册（遥测、结算、Nexus
  覆盖）正在从 `docs/source/` 移植并将登陆此站点
  迁移工作取得进展。

## 当前状态

- ✅ 主题 Docusaurus v3 登陆，刷新版式，渐变驱动
  英雄/卡片，以及包含 Norito 流摘要的资源图块。
- ✅ Torii OpenAPI 插件连接到 `npm run sync-openapi`，带有签名快照
  由 `buildSecurityHeaders` 强制执行的检查和 CSP 防护。
- ✅ 在 CI 中运行预览和探测覆盖范围 (`docs-portal-preview.yml` +
  `scripts/portal-probe.mjs`)，现在控制流文档，SoraFS 快速入门，
  以及发布工件之前的参考清单。
- ✅ Norito、SoraFS 和 SDK 快速入门以及参考部分已在
  侧边栏；来自 `docs/source/` 的新导入（流、编排、运行手册）
  按其创作方式登陆此处。

## 参与其中

- 请参阅 `docs/portal/README.md` 以了解本地开发命令（`npm install`、
  `npm run start`、`npm run build`）。
- 内容迁移任务与 `DOCS-*` 路线图项目一起跟踪。
  欢迎贡献——移植 `docs/source/` 的部分并添加页面
  到侧边栏。
- 如果添加生成的工件（规格、配置表），请记录构建
  命令，以便未来的贡献者可以轻松刷新它。