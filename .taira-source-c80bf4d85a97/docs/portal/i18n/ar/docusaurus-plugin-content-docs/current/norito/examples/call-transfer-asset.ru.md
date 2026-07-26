---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/call-transfer-asset
العنوان: إنشاء علاقة مع المضيف من Kotodama
description: لاحظ كيف يمكن لـ Kotodama أن تستخرج تعليمات المضيف `transfer_asset` باستخدام اختبار قوي.
المصدر: صناديق/ivm/docs/examples/08_call_transfer_asset.ko
---

لاحظ كيف يمكن لنقطة Kotodama استخدام تعليمات المضيف `transfer_asset` باستخدام اختبار حقيقي.

## Почаговый обдод еестра

- عقد المساهمات الكاملة (على سبيل المثال `<i105-account-id>`) الذي يتم تجديده على الميزانية، ويحدد الدور الهام `CanTransfer` أو ما يعادلها.
- اختر هنا `call_transfer_asset` لتحويل 5 سنوات إلى عقد حساب `<i105-account-id>`، بالإضافة إلى ذلك، يمكن للأتمتة الأولية أن تدعم استخدام المضيف.
- التحقق من التوازن عبر `FindAccountAssets` أو `iroha_cli ledger assets list --account <i105-account-id>` وإظهار الاشتراكات للتأكد من حماية البيانات المسجلة إعادة صياغة السياق.

## تطوير شامل SDK

- [Quickstart Rust SDK](/sdks/rust)
- [Quickstart Python SDK](/sdks/python)
- [Quickstart JavaScript SDK](/sdks/javascript)

[تحميل المصدر Kotodama](/norito-snippets/call-transfer-asset.ko)

```kotodama
// Direct builtin call (no raw call syntax) inside a seiyaku.
seiyaku TransferCall {
    kotoage fn pay() authorize("AssetTransferRole") {
        ledger::asset::transfer(
            source: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV", ),
            destination: AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76", ),
            asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
            amount: 10,
            dataspace: DataSpaceId::parse("0"),
        );
    }
}
```
