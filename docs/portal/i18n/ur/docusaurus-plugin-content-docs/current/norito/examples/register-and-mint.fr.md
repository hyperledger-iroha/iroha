---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/register-and-mint.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/رجسٹر اور ٹکسال
عنوان: ڈومین اور ٹکسال کے اثاثوں کو رجسٹر کریں
تفصیل: اجازتوں کے ساتھ ڈومینز بنانے ، اثاثوں کو رجسٹر کرنے ، اور عصبی ٹائپنگ کا مظاہرہ کرتا ہے۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/13_register_and_mint.ko
---

اجازتوں کے ساتھ ڈومینز بنانے ، اثاثوں کو رجسٹر کرنے ، اور عصبی ٹائپنگ کا مظاہرہ کرتا ہے۔

## رجسٹری براؤزنگ

- اس بات کو یقینی بنائیں کہ منزل مقصود اکاؤنٹ (جیسے `<katakana-i105-account-id>`) موجود ہے ، ہر SDK کوئیک اسٹارٹ میں سیٹ اپ مرحلے کی عکاسی کرتا ہے۔
- ایک ہی ٹرانزیکشن میں ایلس کے لئے روز اثاثہ تعریف اور ٹکسال 250 یونٹ بنانے کے لئے انٹری پوائنٹ `register_and_mint` کی انووک کریں۔
- کی اسٹروک کے کامیاب ہونے کی تصدیق کرنے کے لئے `client.request(FindAccountAssets)` یا `iroha_cli ledger assets list --account <katakana-i105-account-id>` کے ذریعے بیلنس چیک کریں۔

## متعلقہ SDK گائیڈز

- [کوئک اسٹارٹ SDK مورچا] (/sdks/rust)
- [کوئک اسٹارٹ ازگر ایس ڈی کے] (/sdks/python)
- [کوئیک اسٹارٹ جاوا اسکرپٹ SDK] (/sdks/javascript)

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
    let to = account!("<katakana-i105-account-id>");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```