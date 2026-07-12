---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/register-and-mint.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/register-and-mint
العنوان: تسجيل نطاق وسك الأصول
description: يوضح إنشاء النطاقات المصرح وتسجيل الأصول والسك الحتمي.
المصدر: صناديق/ivm/docs/examples/13_register_and_mint.ko
---

يوضح إنشاء النطاقات المصرح بها وتسجيل الأصول والككتمي.

## جولة أستاذ الأستاذ

- التأكد من وجود حساب الوجه (مثل `<i105-account-id>`) بما في ذلك التحقق من صحة كل بدء سريع لـ SDK.
- فوراِ نقطة الدخول `register_and_mint` لتعريف التعريف الأصلي ROSE وسك 250 وحدة لأليس في فارة واحدة.
- تحقق من الرصدة عبر `client.request(FindAccountAssets)` أو `iroha_cli ledger assets list --account <i105-account-id>` لتأكيد نجاح السك.

## دليل SDK ذات صلة

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/register-and-mint.ko)

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
        let to = AccountId::parse(
            "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
        );
        ledger::asset::mint(account: to, asset_definition: asset, amount: 250);
    }
}
```
