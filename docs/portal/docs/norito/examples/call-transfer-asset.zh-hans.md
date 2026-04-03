<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
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
title: 从 Kotodama 调用主机传输
description: 演示 Kotodama 入口点如何通过内联元数据验证调用主机 `transfer_asset` 指令。
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

演示 Kotodama 入口点如何通过内联元数据验证调用主机 `transfer_asset` 指令。

## 账本演练

- 用其将转移的资产为合约机构（例如合约账户的 `<i105-account-id>`）提供资金，并授予该机构 `CanTransfer` 角色或同等权限。
- 调用 `call_transfer_asset` 入口点，将 5 个单位从合约账户转移给 Bob (`<i105-account-id>`)，镜像链上自动化包装主机调用的方式。
- 通过 `FindAccountAssets` 或 `iroha_cli ledger asset list --account <i105-account-id>` 验证余额并检查事件以确认元数据防护记录了传输上下文。

## 相关SDK指南

- [Rust SDK 快速入门](/sdks/rust)
- [Python SDK 快速入门](/sdks/python)
- [JavaScript SDK 快速入门](/sdks/javascript)

[下载Kotodama源码](/norito-snippets/call-transfer-asset.ko)

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