---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/transfer-asset
title: アカウント間の資産移転
description: SDK クイックスタートと台帳ウォークスルーを反映した、わかりやすい資産移転フローです。
source: examples/transfer/transfer.ko
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
