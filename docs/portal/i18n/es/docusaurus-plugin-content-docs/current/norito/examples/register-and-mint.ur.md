---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/examples/register-and-mint
título: ڈومین رجسٹر کریں اور اثاثے منٹ کریں
descripción: اجازت یافتہ ڈومین تخلیق، اثاثہ رجسٹریشن اور ڈیٹرمنسٹک منٹنگ کو ظاہر کرتا ہے۔
fuente: crates/ivm/docs/examples/13_register_and_mint.ko
---

اجازت یافتہ ڈومین تخلیق، اثاثہ رجسٹریشن اور ڈیٹرمنسٹک منٹنگ کو ظاہر کرتا ہے۔

## لیجر واک تھرو

- یقینی بنائیں کہ منزل اکاؤنٹ (مثلا `<i105-account-id>`) موجود ہے، جو ہر SDK inicio rápido کے سیٹ اپ مرحلے کی عکاسی کرتا ہے۔
- `register_and_mint` انٹری پوائنٹ کال کریں تاکہ ROSE اثاثہ ڈیفینیشن بنے اور ایک ہی ٹرانزیکشن میں Alice کو 250 یونٹس منٹ ہوں۔
- `client.request(FindAccountAssets)` یا `iroha_cli ledger assets list --account <i105-account-id>` کے ذریعے بیلنس چیک کریں تاکہ منٹنگ کی کامیابی کی تصدیق ہو۔

## متعلقہ SDK گائیڈز

- [Inicio rápido del SDK de Rust](/sdks/rust)
- [Inicio rápido del SDK de Python](/sdks/python)
- [Inicio rápido del SDK de JavaScript](/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/register-and-mint.ko)

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
    let to = account!("<i105-account-id>");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```