---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/examples/register-and-mint
título: تسجيل نطاق وسك الأصول
descripción: يوضح إنشاء النطاقات المصرح بها وتسجيل الأصول والسك الحتمي.
fuente: crates/ivm/docs/examples/13_register_and_mint.ko
---

يوضح إنشاء النطاقات المصرح بها وتسجيل الأصول والسك الحتمي.

## جولة دفتر الأستاذ

- تأكد من وجود حساب الوجهة (مثل `soraカタカナ...`) بما يعكس مرحلة الإعداد في كل بدء سريع للـ SDK.
- Para el hogar `register_and_mint`, coloque el ROSE y el 250 y el aire acondicionado.
- Haga clic en el botón `client.request(FindAccountAssets)` y `iroha_cli ledger assets list --account soraカタカナ...`.

## أدلة SDK ذات صلة

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [Aplicación del SDK de Python](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[Actualización Kotodama](/norito-snippets/register-and-mint.ko)

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
    let to = account!("soraカタカナ...");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```