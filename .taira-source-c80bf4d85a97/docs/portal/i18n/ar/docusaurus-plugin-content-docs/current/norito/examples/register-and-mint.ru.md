---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/register-and-mint.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/register-and-mint
العنوان: تسجيل النطاق وشراء الأنشطة
description: عرض إنشاء النطاقات بالتحديد والتسجيل النشط وتحديد الخيارات.
المصدر: صناديق/ivm/docs/examples/13_register_and_mint.ko
---

يعرض إنشاء النطاقات بالتحديدات والتسجيل النشط وتحديد الخيارات.

## Почаговый обдод еестра

- تأكد من أن اسم الحساب (على سبيل المثال `<i105-account-id>`) موجود، وهو ما يعكس التحسينات في Quickstart SDK.
- اختر هنا `register_and_mint` لإنشاء النشاط المقترح ROSE وشراء 250 وحدة لـ Alice في معاملة واحدة.
- تحقق من التوازن عبر `client.request(FindAccountAssets)` أو `iroha_cli ledger assets list --account <i105-account-id>` للتحقق من صحة البيانات بنجاح.

## تطوير شامل SDK

- [Quickstart Rust SDK](/sdks/rust)
- [Quickstart Python SDK](/sdks/python)
- [Quickstart JavaScript SDK](/sdks/javascript)

[تحميل النسخة Kotodama](/norito-snippets/register-and-mint.ko)

```kotodama
// Register a new asset and mint some to the specified account.
seiyaku RegisterAndMint {
    kotoage fn register_and_mint() authorize("AssetManager") {
        // name, symbol, quantity (precision or supply depending on host), mintable flag
        let asset = AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
        let symbol = "ROSE";
        let qty = 1000;
        // interpretation depends on data model (example only)
        let mintable = 1;
        // 1 = mintable, 0 = fixed
        ledger::asset::register(
            asset_definition: asset,
            name: symbol,
            scale: qty,
            mintable: mintable,
        );
        // Mint 250 ROSE to Alice
        let to = AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV", );
        ledger::asset::mint(account: to, asset_definition: asset, amount: 250);
    }
}
```
