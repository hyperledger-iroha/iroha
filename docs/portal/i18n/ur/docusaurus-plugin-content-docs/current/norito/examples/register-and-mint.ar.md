---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/register-and-mint.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/رجسٹر اور ٹکسال
عنوان: ڈومین رجسٹریشن اور اثاثہ منڈنگ
تفصیل: مجاز ڈومینز کی تشکیل ، اثاثوں کی رجسٹریشن ، اور ناگزیر ٹکسال کا مظاہرہ کرتا ہے۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/13_register_and_mint.ko
---

مجاز ڈومینز کی تشکیل ، اثاثوں کی رجسٹریشن ، اور ناگزیر ٹکسال کی وضاحت کرتا ہے۔

## لیجر ٹور

- اس بات کو یقینی بنائیں کہ ہر SDK ہاٹ اسٹارٹ میں سیٹ اپ مرحلے کی عکاسی کرنے کے لئے منزل مقصود اکاؤنٹ (جیسے `i105...`) موجود ہے۔
- ایک ہی لین دین میں ایلس کے لئے گلاب اثاثہ تعریف اور ٹکسال 250 یونٹ بنانے کے لئے انٹری پوائنٹ `register_and_mint` پر کال کریں۔
- کامیاب ٹکسال کی تصدیق کے ل I `client.request(FindAccountAssets)` یا `iroha_cli ledger assets list --account i105...` کے ذریعے بیلنس چیک کریں۔

## متعلقہ SDK گائیڈز

- [مورچا SDK کوئیک اسٹارٹ] (/sdks/rust)
- [ازگر SDK کوئیک اسٹارٹ] (/sdks/python)
- [جاوا اسکرپٹ SDK کوئیک اسٹارٹ] (/sdks/javascript)

[ماخذ Kotodama ڈاؤن لوڈ کریں] (/norito-snippets/register-and-mint.ko)

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