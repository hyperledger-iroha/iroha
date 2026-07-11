---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/call-transfer-asset
العنوان: للاتصال بـ الإرسال من Kotodama
description: يوضح كيف يمكن لنقطة Kotodama الاتصال بالاتصال التعرفة `transfer_asset` مع التحقق المضمن من بيانات التعريف.
المصدر: صناديق/ivm/docs/examples/08_call_transfer_asset.ko
---

يوضح كيف يمكن لنقطة الدخول Kotodama الاتصال بالطلبة الخاصة `transfer_asset` مع التحقق المضمن من بيانات التعريف.

## جولة أستاذ الأستاذ

- موّل سلطة العقد (مثلا `<i105-account-id>`) بالأصل الذي ستنقله وامنح السلطة دور `CanTransfer` أو إذنا مكافئا.
- المؤكد أن نقطة الدخول `call_transfer_asset` نقل 5 وحدات من حساب العقد إلى `<i105-account-id>`، بما في ذلك تأكيد دقة التسجيل على السجل لنداءات السجل المدني.
- التحقق من الرصدة عبر `FindAccountAssets` أو `iroha_cli ledger assets list --account <i105-account-id>` وفحص الأحداث لتأكيد بيانات تعريف البيانات الخاصة بسجل اتفاقية النقل.

## دليل SDK ذات صلة

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/call-transfer-asset.ko)

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
