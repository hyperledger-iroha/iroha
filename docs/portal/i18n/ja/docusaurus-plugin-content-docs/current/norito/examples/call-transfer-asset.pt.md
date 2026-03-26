---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
スラッグ: /norito/examples/call-transfer-asset
タイトル: 呼び出し側転送は Kotodama のパーティをホストします
説明: ほとんどのエントリポイント Kotodama ホスト `transfer_asset` com validacao inline de Metadados を実行するための手順。
ソース: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

ほとんどのエントリポイント Kotodama ホスト `transfer_asset` com validacao inline de Metados を実行するためのコマンドが実行されます。

## ロテイロ・ド・リヴロ・ラザオ

- 資金調達と契約 (例 `<katakana-i105-account-id>`) は、`CanTransfer` と同等の権限を譲渡するための契約を締結します。
- エントリポイント `call_transfer_asset` を転送し、`<katakana-i105-account-id>` からの 5 つのデータを転送し、自動オンチェーン ポード エンボルバー チャマダをホストに反映します。
- `FindAccountAssets` または `iroha_cli ledger assets list --account <katakana-i105-account-id>` を介して、メタデータ登録の確認と転送の状況を確認するイベントを検査してください。

## SDK 関係に関する情報

- [SDK Rust のクイックスタート](/sdks/rust)
- [SDK Python のクイックスタート](/sdks/python)
- [SDK JavaScript のクイックスタート](/sdks/javascript)

[バイシェ ア フォンテ Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("<katakana-i105-account-id>"),
      account!("<katakana-i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```