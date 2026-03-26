---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/register-and-mint.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/register-and-mint
العنوان: مسجل الويب والأثاث
الوصف: سمح بتخليق اليافعة، وتسجيل الأثاث وجميع أنواع الأنسجة.
المصدر: صناديق/ivm/docs/examples/13_register_and_mint.ko
---

سمح لليافتين بالتخلي عن الأثاث التسجيلي وجميع أنواع الأنسجة.

## ليجر واک تھرو

- هذا هو التطبيق المنزلي (مثل `i105...`) الموجود، وهو عبارة عن SDK Quickstart وهو عبارة عن رحلة مستمرة.
- `register_and_mint` بطاقة ائتمان عبر الإنترنت من ROSE وهي عبارة عن أثاث من إيفينشن وأليس 250 يونيو.
- `client.request(FindAccountAssets)` أو `iroha_cli ledger assets list --account i105...` هو ذريعة واحدة لفحص كريات الدم البيضاء.

## مواضيع ذات صلة SDK

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[Kotodama تنزيل التنزيل](/norito-snippets/register-and-mint.ko)

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