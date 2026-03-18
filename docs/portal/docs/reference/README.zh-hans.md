---
lang: zh-hans
direction: ltr
source: docs/portal/docs/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b3b2becfdbab1446f8f230ace905de306e1e89147f5a5e578d784be97445d74d
source_last_modified: "2025-12-29T18:16:35.156432+00:00"
translation_last_reviewed: 2026-02-07
title: Reference Index
slug: /reference
translator: machine-google-reviewed
---

本节汇总了 Iroha 的“将其作为规范阅读”材料。这些页面保持稳定，即使
指南和教程不断发展。

## 今天可用

- **Norito编解码器概述** – `reference/norito-codec.md`直接链接到权威
  填充入口表时的 `norito.md` 规范。
- **Torii OpenAPI** – `/reference/torii-openapi` 使用以下方式呈现最新的 Torii REST 规范
- **Torii MCP API** - `/reference/torii-mcp` documents MCP JSON-RPC usage (`initialize`, `tools/list`, `tools/call`) and async job polling for `/v1/mcp`.
  重新记录。使用 `npm run sync-openapi -- --version=current --latest` 重新生成规范（添加
  `--mirror=<label>` 将快照复制到其他历史版本中）。
- **配置表** – 完整的参数目录保存在
  `docs/source/references/configuration.md`。在门户发布自动导入之前，请参考
  用于精确默认值和环境覆盖的 Markdown 文件。
- **文档版本控制** – 导航栏版本下拉列表公开了使用创建的冻结快照
  `npm run docs:version -- <label>`，可以轻松比较不同版本的指南。

## 即将推出

- **Torii REST 参考** – OpenAPI 定义将通过同步到此部分
  启用管道后，`docs/portal/scripts/sync-openapi.mjs`。
- **CLI 命令索引** – 生成的命令矩阵（镜像 `crates/iroha_cli/src/commands`）
  将与规范示例一起登陆此处。
- **IVM ABI 表** – 指针类型和系统调用矩阵（在 `crates/ivm/docs` 下维护）
  一旦文档生成作业连接完毕，将呈现到门户中。

## 保持该索引最新

添加新的参考资料时——生成的 API 文档、编解码器规范、配置矩阵——放置
`docs/portal/docs/reference/` 下的页面并在上面链接。如果页面是自动生成的，请注意
同步脚本，以便贡献者知道如何刷新它。这使参考树保持有用，直到
完全自动生成的导航土地。
