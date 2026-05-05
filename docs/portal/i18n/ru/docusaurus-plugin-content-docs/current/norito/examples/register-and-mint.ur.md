---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слизень: /norito/examples/register-and-mint
Название: ڈومین رجسٹر کریں اور اثاثے منٹ کریں
описание: اجازت یافتہ ڈومین تخلیق، اثاثہ رجسٹریشن اور ڈیٹرمنسٹک کو ظاہر کرتا ہے۔
источник: crates/ivm/docs/examples/13_register_and_mint.ko
---

Если вы хотите, чтобы вам было удобно, когда вы хотите, کرتا ہے۔

## لیجر واک تھرو

- Получите доступ к быстрому запуску SDK (например, `<i105-account-id>`) и ознакомьтесь с кратким руководством по SDK. مرحلے کی عکاسی کرتا ہے۔
- `register_and_mint` پوائنٹ کال کریں تاکہ ROSE اثاثہ ڈیفینیشن بنے اور ایک ہی Имя Алисы: 250 лет назад
- `client.request(FindAccountAssets)` یا `iroha_cli ledger assets list --account <i105-account-id>` کے ذریعے بیلنس چیک کریں تاکہ کی کامیابی کی تصدیق ہو۔

## Использование SDK

- [Краткий старт Rust SDK](/sdks/rust)
- [Краткий старт Python SDK](/sdks/python)
- [Краткое руководство по JavaScript SDK] (/sdks/javascript)

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