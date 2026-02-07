---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/register-and-mint
título: ڈومین رجسٹر کریں اور اثاثے منٹ کریں
description: اجازت یافتہ ڈومین تخلیق, اثاثہ رجسٹریشن اور ڈیٹرمنسٹک منٹنگ کو ظاہر کرتا ہے۔
fonte: crates/ivm/docs/examples/13_register_and_mint.ko
---

اجازت یافتہ ڈومین تخلیق, اثاثہ رجسٹریشن اور ڈیٹرمنسٹک منٹنگ کو ظاہر کرتا ہے۔

## لیجر واک تھرو

- یقینی بنائیں کہ منزل اکاؤنٹ (مثلا `ih58...`) موجود ہے، جو ہر SDK quickstart کے سیٹ اپ مرحلے کی عکاسی کرتا ہے۔
- `register_and_mint` انٹری پوائنٹ کال کریں تاکہ ROSE اثاثہ ڈیفینیشن بنے اور ایک ہی ٹرانزیکشن میں Alice کو 250 یونٹس منٹ ہوں۔
- `client.request(FindAccountAssets)` یا `iroha_cli ledger assets list --account ih58...` کے ذریعے بیلنس چیک کریں تاکہ منٹنگ کی کامیابی کی تصدیق ہو۔

## Como instalar o SDK

- [Início rápido do Rust SDK](/sdks/rust)
- [início rápido do SDK do Python](/sdks/python)
- [início rápido do SDK JavaScript](/sdks/javascript)

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
    let to = account!("ih58...");
    let asset = asset_definition!("rose#wonderland");
    mint_asset(to, asset, 250);
  }
}
```