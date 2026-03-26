---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09faa38009213d29d57d63e4f56a4b83463eb5e2a83476eeeff87b77c86e8eb3
source_last_modified: "2026-01-22T15:55:00+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/call-transfer-asset
title: Kotodama からホスト転送を呼び出す
description: Kotodama のエントリポイントがホストの `transfer_asset` 命令を、インラインのメタデータ検証付きで呼び出せることを示します。
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Kotodama のエントリポイントがホストの `transfer_asset` 命令を、インラインのメタデータ検証付きで呼び出せることを示します。

## 台帳ウォークスルー

- コントラクトの権限者（例: `soraカタカナ...`）に転送対象の資産を用意し、権限者に `CanTransfer` ロールまたは同等の権限を付与します。
- `call_transfer_asset` エントリポイントを呼び出して、コントラクトアカウントから `soraカタカナ...` に 5 単位を転送します。オンチェーン自動化がホスト呼び出しをラップする方法を反映しています。
- `FindAccountAssets` または `iroha_cli ledger assets list --account soraカタカナ...` で残高を確認し、イベントを調べてメタデータガードが転送コンテキストを記録したことを確かめます。

## 関連 SDK ガイド

- [Rust SDK クイックスタート](/sdks/rust)
- [Python SDK クイックスタート](/sdks/python)
- [JavaScript SDK クイックスタート](/sdks/javascript)

[Kotodama ソースをダウンロード](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("soraカタカナ..."),
      account!("soraカタカナ..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```
