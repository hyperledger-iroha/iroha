---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/register-and-mint.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/register-and-mint
العنوان: المسجل dominio e cunhar ativos
الوصف: قم بإظهار إنشاء نطاقات من خلال السماح أو تسجيل المهام أو تحديد الحتمية.
المصدر: صناديق/ivm/docs/examples/13_register_and_mint.ko
---

قم بإظهار إنشاء نطاقات من خلال السماح أو تسجيل المهام أو تحديد الحتمية.

## Roteiro do livro razao

- ضمان وجود حساب الوجهة (على سبيل المثال `soraカタカナ...`)، وبدء عملية التكوين في كل Quickstart من SDK.
- قم باستدعاء نقطة الدخول `register_and_mint` لإنشاء تعريف لـ ROSE وجمع 250 وحدة لـ Alice في معاملة واحدة.
- تحقق من الرسائل عبر `client.request(FindAccountAssets)` أو `iroha_cli ledger assets list --account soraカタカナ...` لتأكيد نجاح العملية.

## أدلة SDK ذات الصلة

- [بدء التشغيل السريع لـ SDK Rust](/sdks/rust)
- [البدء السريع لـ SDK Python](/sdks/python)
- [بدء التشغيل السريع لـ SDK JavaScript](/sdks/javascript)

[اضغط على الخط Kotodama](/norito-snippets/register-and-mint.ko)

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