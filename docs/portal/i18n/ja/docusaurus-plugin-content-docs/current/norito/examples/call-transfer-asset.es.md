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

- Fondea la autoridad del contrato (por ejemplo `<i105-account-id>`) con el activo que transferirá y otórgale el rol `CanTransfer` と同等の権限を与えられます。
- ラマのエントリポイント `call_transfer_asset` は、`<i105-account-id>` とコントラクトの 5 つのユニットを転送し、オンチェーンのエンボルバー ラマダのホストを参照して自動化されます。
- 中央値 `FindAccountAssets` または `iroha_cli ledger assets list --account <i105-account-id>` を検査し、メタデータ登録のメタデータを確認するための検査を行います。

## SDK 関係に関する情報

- [SDK および Rust のクイックスタート](/sdks/rust)
- [Python の SDK クイックスタート](/sdks/python)
- [JavaScript の SDK のクイックスタート](/sdks/javascript)

[Kotodama](/norito-snippets/call-transfer-asset.ko) をダウンロードしてください

```kotodama
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
    kotoage fn pay() authorize("AssetTransferRole") {
        ledger::asset::transfer(
            source: AccountId::parse(
                "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            ),
            destination: AccountId::parse(
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            ),
            asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
            amount: Amount::from_i64(10),
            dataspace: DataSpaceId::parse("0"),
        );
    }
}
```
