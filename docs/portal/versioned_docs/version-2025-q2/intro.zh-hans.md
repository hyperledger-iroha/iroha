---
lang: zh-hans
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/intro.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e797879d1f77c8cfd62fcc67874d584f6bdeee9395faafe52fc33f26ce2e6a21
source_last_modified: "2025-12-29T18:16:35.904811+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 欢迎来到 SORA Nexus 开发者门户

SORA Nexus 开发者门户捆绑了交互式文档、SDK
Nexus 运算符和 Hyperledger Iroha 的教程和 API 参考
贡献者。它通过提供实践指南来补充主要文档网站
并直接从该存储库生成规格。

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

- ✅ Docusaurus v3 脚手架，带有 Norito 和 SDK 快速入门的实时页面。
- ✅ Torii OpenAPI 插件连接到 `npm run sync-openapi`。
- ⏳ 从 `docs/source/` 迁移剩余指南。
- ⏳ 将预览版本和 linting 添加到文档 CI 中。

## 参与其中

- 请参阅 `docs/portal/README.md` 以了解本地开发命令（`npm install`、
  `npm run start`、`npm run build`）。
- 内容迁移任务与 `DOCS-*` 路线图项目一起进行跟踪。
  欢迎贡献——移植 `docs/source/` 的部分并添加页面
  到侧边栏。
- 如果添加生成的工件（规格、配置表），请记录构建
  命令，以便未来的贡献者可以轻松刷新它。