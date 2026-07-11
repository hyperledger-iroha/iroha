---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
スラッグ: /norito/examples/call-transfer-asset
タイトル: استدعاء نقل المضيف من Kotodama
説明: يوضح كيف يمكن لنقطة دخول Kotodama استدعاء تعليمة المضيف `transfer_asset` مع التحقق المضمن重要です。
ソース: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

يوضح كيف يمكن لنقطة دخول Kotodama استدعاء تعليمة المضيف `transfer_asset` مع التحقق المضمن منやあ。

## جولة دفتر الأستاذ

- موّل سلطة العقد (مثلا `<i105-account-id>`) بالأصل الذي ستنقله وامنح السلطة دور `CanTransfer` أو إذناああ。
- `call_transfer_asset` 5 وحساب العقد إلى `<i105-account-id>` بما يعكس最高のパフォーマンスを見せてください。
- 評価 `FindAccountAssets` أو `iroha_cli ledger assets list --account <i105-account-id>` وافحص الأحداث لتأكيد أن حارس بياناتありがとうございます。

## SDK の開発

- [Rust SDK](/sdks/rust)
- [Python SDK](/sdks/python)
- [JavaScript SDK](/sdks/javascript)

[Kotodama](/norito-snippets/call-transfer-asset.ko)

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
