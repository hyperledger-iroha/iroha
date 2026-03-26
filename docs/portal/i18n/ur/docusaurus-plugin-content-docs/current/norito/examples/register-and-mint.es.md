---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/register-and-mint.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/رجسٹر اور ٹکسال
عنوان: ڈومین اور ٹکسال کے اثاثے رجسٹر کریں
تفصیل: اجازت نامہ ڈومین تخلیق ، اثاثوں کی رجسٹریشن ، اور عین مطابق ٹکسال کا مظاہرہ کرتا ہے۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/13_register_and_mint.ko
---

اجازت نامہ ڈومین تخلیق ، اثاثوں کی رجسٹریشن ، اور عین مطابق ٹکسال کا مظاہرہ کرتا ہے۔

## لیجر ٹور

- اس بات کو یقینی بنائیں کہ ہدف اکاؤنٹ موجود ہے (مثال کے طور پر `i105...`) ، ہر SDK کوئیک اسٹارٹ میں ترتیب کے مرحلے کی عکاسی کرتا ہے۔
- ایک ہی لین دین میں ایلس کے لئے روز اثاثہ تعریف اور ٹکسال 250 یونٹ بنانے کے لئے انٹری پوائنٹ `register_and_mint` کی انووک کریں۔
- اس بات کی تصدیق کے لئے `client.request(FindAccountAssets)` یا `iroha_cli ledger assets list --account i105...` کا استعمال کرتے ہوئے بیلنس چیک کریں۔

## متعلقہ SDK گائیڈز

- [مورچا SDK کوئیک اسٹارٹ] (/sdks/rust)
- [ازگر ایس ڈی کے کوئیک اسٹارٹ] (/sdks/python)
- [جاوا اسکرپٹ SDK کوئیک اسٹارٹ] (/sdks/javascript)

[Kotodama کا ماخذ ڈاؤن لوڈ کریں] (/norito-snippets/register-and-mint.ko)

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