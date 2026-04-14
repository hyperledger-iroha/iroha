<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dcd8de175a7c5172158a03e1a25b254c90a11e62c173f95b8d9e4a387df6ba09
source_last_modified: "2026-03-26T13:01:47.372931+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/call-transfer-asset
title: 從 Kotodama 呼叫主機傳輸
description: 示範 Kotodama 入口點如何透過內嵌元資料驗證呼叫主機 `transfer_asset` 指令。
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

示範 Kotodama 入口點如何透過內嵌元資料驗證呼叫主機 `transfer_asset` 指令。

## 帳本演練

- 用其將轉移的資產為合約機構（例如合約帳戶的 `<i105-account-id>`）提供資金，並授予該機構 `CanTransfer` 角色或同等權限。
- 呼叫 `call_transfer_asset` 入口點，將 5 個單位從合約帳戶轉移給 Bob (`<i105-account-id>`)，鏡像鏈上自動化包裝主機呼叫的方式。
- 透過 `FindAccountAssets` 或 `iroha_cli ledger asset list --account <i105-account-id>` 驗證餘額並檢查事件以確認元資料防護記錄了傳輸上下文。

## 相關SDK指南

- [Rust SDK 快速入門](/sdks/rust)
- [Python SDK 快速入门](/sdks/python)
- [JavaScript SDK 快速入門](/sdks/javascript)

[下載Kotodama原始碼](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"),
      account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```