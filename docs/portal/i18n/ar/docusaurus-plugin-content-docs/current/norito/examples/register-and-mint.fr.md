---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/register-and-mint.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/register-and-mint
العنوان: تسجيل نطاق وتحرير الأنشطة
الوصف: إنشاء النطاقات باستخدام التفويضات وتسجيل الأنشطة والإطار المحدد.
المصدر: صناديق/ivm/docs/examples/13_register_and_mint.ko
---

قم ببدء إنشاء النطاقات باستخدام التفويضات وتسجيل الأنشطة والإطار المحدد.

## باركور دو ريجيستري

- تأكد من وجود حساب الوجهة (على سبيل المثال `i105...`)، بما يعكس مرحلة الإعداد في كل Quickstart SDK.
- قم باستدعاء نقطة الإدخال `register_and_mint` لإنشاء تعريف نشط ROSE وقم بإعداد 250 وحدة لـ Alice في معاملة واحدة.
- تحقق من المبيعات عبر `client.request(FindAccountAssets)` أو `iroha_cli ledger assets list --account i105...` لتأكيد عملية الطحن.

## أدلة شركاء SDK

- [Quickstart SDK Rust](/sdks/rust)
- [Quickstart SDK Python](/sdks/python)
- [Quickstart SDK JavaScript](/sdks/javascript)

[تحميل المصدر Kotodama](/norito-snippets/register-and-mint.ko)

```text
// Register a new asset and mint some to the specified account.
seiyaku RegisterAndMint {
  kotoage fn register_and_mint() permission(AssetManager) {
    // name, symbol, quantity (precision or supply depending on host), mintable flag
    let name = "rose";
    let symbol = "ROSE";
    let qty = 1000;      // interpretation depends on data model (example only)
    let mintable = 1;    // 1 = mintable, 0 = fixed
    register_asset(name, symbol, qty, mintable);

    // Mint 250 ROSE to Alice
    let to = account!("i105...");
    let asset = asset_definition!("rose#wonderland");
    mint_asset(to, asset, 250);
  }
}
```