---
id: runbooks-index
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Operator Runbooks Index
description: Canonical entry point for the migrated SoraFS operator runbooks.
sidebar_label: Runbook Index
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> 鏡像 `docs/source/sorafs/runbooks/` 下的所有者分類帳。
> 每個新的 SoraFS 操作指南在發布後都必須鏈接到此處
> 門戶構建。

使用此頁面驗證哪些 Runbook 已完成從
源路徑和門戶副本，以便審閱者可以直接跳轉到所需的內容
Beta 預覽期間的指南。

## Beta 預覽版主機

DocOps 浪潮現已在以下網址推廣經過審閱者批准的測試版預覽主機：
`https://docs.iroha.tech/`。當將操作員或審閱者指向已遷移的
Runbook，引用該主機名，以便他們執行校驗和門控門戶
快照。發布/回滾程序位於
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md)。

|運行手冊|所有者 |傳送門副本|來源 |
|--------|----------|-------------|--------|
|網關和 DNS 啟動 |網絡 TL、運營自動化、文檔/開發版本 | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS 操作手冊 |文檔/開發版本 | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
|產能調節|財政部/SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Pin 註冊表操作 |工具工作組 | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
|節點操作清單 |存儲團隊，SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
|爭議和撤銷操作手冊 |治理委員會| [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
|暫存清單劇本 |文檔/開發版本 | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
|泰開錨可觀測性|媒體平台 WG / DA 計劃 / 網絡 TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## 驗證清單

- [x] 門戶構建指向此索引的鏈接（側邊欄條目）。
- [x] 每個遷移的 Runbook 都會列出規范源路徑以保留審閱者
  在文檔審查期間保持一致。
- [x] 當列出的 Runbook 丟失時，DocOps 預覽管道會阻止合併
  從門戶輸出。

未來的遷移（例如，新的混沌演習或治理附錄）應該添加
行添加到上表並更新嵌入的 DocOps 檢查表
`docs/examples/docs_preview_request_template.md`。