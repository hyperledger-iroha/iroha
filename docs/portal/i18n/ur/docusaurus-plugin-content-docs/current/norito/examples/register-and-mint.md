---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/register-and-mint.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/register-and-mint
title: ڈومین رجسٹر کریں اور اثاثے منٹ کریں
description: اجازت یافتہ ڈومین تخلیق، اثاثہ رجسٹریشن اور ڈیٹرمنسٹک منٹنگ کو ظاہر کرتا ہے۔
source: crates/ivm/docs/examples/13_register_and_mint.ko
---

اجازت یافتہ ڈومین تخلیق، اثاثہ رجسٹریشن اور ڈیٹرمنسٹک منٹنگ کو ظاہر کرتا ہے۔

## لیجر واک تھرو

- یقینی بنائیں کہ منزل اکاؤنٹ (مثلا `<i105-account-id>`) موجود ہے، جو ہر SDK quickstart کے سیٹ اپ مرحلے کی عکاسی کرتا ہے۔
- `register_and_mint` انٹری پوائنٹ کال کریں تاکہ ROSE اثاثہ ڈیفینیشن بنے اور ایک ہی ٹرانزیکشن میں Alice کو 250 یونٹس منٹ ہوں۔
- `client.request(FindAccountAssets)` یا `iroha_cli ledger assets list --account <i105-account-id>` کے ذریعے بیلنس چیک کریں تاکہ منٹنگ کی کامیابی کی تصدیق ہو۔

## متعلقہ SDK گائیڈز

- [Rust SDK quickstart](/sdks/rust)
- [Python SDK quickstart](/sdks/python)
- [JavaScript SDK quickstart](/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/register-and-mint.ko)

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
