---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c3d1e36d99e18b5986e911a6b240393a92140324142f9edb778d2f966b1712e
source_last_modified: "2026-01-05T09:28:11.909605+00:00"
translation_last_reviewed: 2026-02-07
id: runbooks-index
title: Operator Runbooks Index
description: Canonical entry point for the migrated SoraFS operator runbooks.
sidebar_label: Runbook Index
translator: machine-google-reviewed
---

> 镜像 `docs/source/sorafs/runbooks/` 下的所有者分类帐。
> 每个新的 SoraFS 操作指南在发布后都必须链接到此处
> 门户构建。

使用此页面验证哪些 Runbook 已完成从
源路径和门户副本，以便审阅者可以直接跳转到所需的内容
Beta 预览期间的指南。

## Beta 预览版主机

DocOps 浪潮现已在以下网址推广经过审阅者批准的测试版预览主机：
`https://docs.iroha.tech/`。当将操作员或审阅者指向已迁移的
Runbook，引用该主机名，以便他们执行校验和门控门户
快照。发布/回滚程序位于
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md)。

|运行手册|所有者 |传送门副本|来源 |
|--------|----------|-------------|--------|
|网关和 DNS 启动 |网络 TL、运营自动化、文档/开发版本 | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS 操作手册 |文档/开发版本 | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
|产能调节|财政部/SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Pin 注册表操作 |工具工作组 | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
|节点操作清单 |存储团队，SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
|争议和撤销操作手册 |治理委员会| [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
|暂存清单剧本 |文档/开发版本 | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
|泰开锚可观测性|媒体平台 WG / DA 计划 / 网络 TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## 验证清单

- [x] 门户构建指向此索引的链接（侧边栏条目）。
- [x] 每个迁移的 Runbook 都会列出规范源路径以保留审阅者
  在文档审查期间保持一致。
- [x] 当列出的 Runbook 丢失时，DocOps 预览管道会阻止合并
  从门户输出。

未来的迁移（例如，新的混沌演习或治理附录）应该添加
行添加到上表并更新嵌入的 DocOps 检查表
`docs/examples/docs_preview_request_template.md`。