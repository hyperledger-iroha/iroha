---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
スラッグ: /norito/examples/call-transfer-asset
タイトル: ホスト宛先の呼び出し側転送 Kotodama
説明: Kotodama のエントリポイントに対するホスト `transfer_asset` のメタデータの検証の確認。
ソース: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

ホスト `transfer_asset` のメタデータの検証に関する Kotodama のエントリ ポイントの確認。

## 市長のレコリード

- Fondea la autoridad del contrato (por ejemplo `i105...`) con el activo que transferirá y otórgale el rol `CanTransfer` と同等の権限を与えられます。
- ラマのエントリポイント `call_transfer_asset` は、`i105...` とコントラクトの 5 つのユニットを転送し、オンチェーンのエンボルバー ラマダのホストを参照して自動化されます。
- 中央値 `FindAccountAssets` または `iroha_cli ledger assets list --account i105...` を検査し、メタデータ登録のメタデータを確認するための検査を行います。

## SDK 関係に関する情報

- [SDK および Rust のクイックスタート](/sdks/rust)
- [Python の SDK クイックスタート](/sdks/python)
- [JavaScript の SDK のクイックスタート](/sdks/javascript)

[Kotodama](/norito-snippets/call-transfer-asset.ko) をダウンロードしてください

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("i105..."),
      account!("i105..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```