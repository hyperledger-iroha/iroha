---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c30c710be94cd99f3c7a0484040155bf63ff4dc0d464d76237bddc8bf589ef26
source_last_modified: "2025-11-07T11:59:47.168250+00:00"
translation_last_reviewed: 2026-01-30
---

---
slug: /norito/examples/register-and-mint
title: ڈومین رجسٹر کریں اور اثاثے منٹ کریں
description: اجازت یافتہ ڈومین تخلیق، اثاثہ رجسٹریشن اور ڈیٹرمنسٹک منٹنگ کو ظاہر کرتا ہے۔
source: crates/ivm/docs/examples/13_register_and_mint.ko
---

اجازت یافتہ ڈومین تخلیق، اثاثہ رجسٹریشن اور ڈیٹرمنسٹک منٹنگ کو ظاہر کرتا ہے۔

## لیجر واک تھرو

- یقینی بنائیں کہ منزل اکاؤنٹ (مثلا `<katakana-i105-account-id>`) موجود ہے، جو ہر SDK quickstart کے سیٹ اپ مرحلے کی عکاسی کرتا ہے۔
- `register_and_mint` انٹری پوائنٹ کال کریں تاکہ ROSE اثاثہ ڈیفینیشن بنے اور ایک ہی ٹرانزیکشن میں Alice کو 250 یونٹس منٹ ہوں۔
- `client.request(FindAccountAssets)` یا `iroha_cli ledger assets list --account <katakana-i105-account-id>` کے ذریعے بیلنس چیک کریں تاکہ منٹنگ کی کامیابی کی تصدیق ہو۔

## متعلقہ SDK گائیڈز

- [Rust SDK quickstart](/sdks/rust)
- [Python SDK quickstart](/sdks/python)
- [JavaScript SDK quickstart](/sdks/javascript)

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
    let to = account!("<katakana-i105-account-id>");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```
