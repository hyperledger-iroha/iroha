---
lang: ru
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44f492a78fdaef451a024dd9b6ca6dbe2112d2fb2ba4e26729823c216403976b
source_last_modified: "2025-11-04T12:26:02.942847+00:00"
translation_last_reviewed: 2026-01-30
---

SDK クイックスタートと台帳ウォークスルーを反映した、わかりやすい資産移転フローです。

## 台帳ウォークスルー

- 対象資産を Alice に事前付与します（例: `register and mint` スニペットや SDK クイックスタートのフロー）。
- `do_transfer` エントリポイントを実行して Alice から Bob へ 10 単位を移転し、`AssetTransferRole` 権限を満たします。
- `FindAccountAssets` や `iroha_cli ledger assets list` で残高を確認するか、パイプラインイベントを購読して移転結果を観測します。

## 関連 SDK ガイド

- [Rust SDK クイックスタート](/sdks/rust)
- [Python SDK クイックスタート](/sdks/python)
- [JavaScript SDK クイックスタート](/sdks/javascript)

[Kotodama ソースをダウンロード](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of rose#wonderland from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("ih58..."),
      account!("ih58..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```
