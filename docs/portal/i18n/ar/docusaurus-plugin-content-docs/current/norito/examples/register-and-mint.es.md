---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/register-and-mint.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/register-and-mint
العنوان: مسجل dominio y acuñar activos
الوصف: عرض إنشاء النطاقات بالأذونات وسجل الأنشطة والضغط المحدد.
المصدر: صناديق/ivm/docs/examples/13_register_and_mint.ko
---

قم بإظهار إنشاء النطاقات بالأذونات، وسجل الأنشطة، والنقر المحدد.

## Recorrido del libro mayor

- التأكد من وجود حساب الوجهة (على سبيل المثال `i105...`)، مما يعكس عملية التكوين في كل Quickstart من SDK.
- استدعاء نقطة الدخول `register_and_mint` لإنشاء تعريف نشط ROSE والحصول على 250 وحدة لـ Alice في معاملة واحدة فقط.
- تحقق من الأرصدة المتوسطة `client.request(FindAccountAssets)` أو `iroha_cli ledger assets list --account i105...` لتأكيد نجاح الإحصاء.

## أدلة SDK ذات الصلة

- [Quickstart del SDK de Rust](/sdks/rust)
- [Quickstart del SDK de Python](/sdks/python)
- [بدء التشغيل السريع لـ SDK لـ JavaScript](/sdks/javascript)

[تنزيل مصدر Kotodama](/norito-snippets/register-and-mint.ko)

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