---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/register-and-mint.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/رجسٹر اور ٹکسال
عنوان: ڈومین رجسٹر کریں اور اثاثوں کو جاری کریں
تفصیل: ڈومین کی تشکیل ، اثاثہ جات کی رجسٹریشن ، اور عین مطابق رہائی کو ظاہر کرتا ہے۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/13_register_and_mint.ko
---

اجازت نامہ ڈومین تخلیق ، اثاثوں کی رجسٹریشن ، اور عین مطابق رہائی کو ظاہر کرتا ہے۔

## قدم بہ قدم رجسٹری ٹریورسل

- اس بات کو یقینی بنائیں کہ ہر کوئیک اسٹارٹ ایس ڈی کے میں فراہمی کے مرحلے کو دہراتے ہوئے منزل مقصود اکاؤنٹ (جیسے `<i105-account-id>`) موجود ہے۔
- ایک ہی لین دین میں ایلس کو گلاب اثاثہ تعریف اور ایلس کو 250 یونٹ جاری کرنے کے لئے انٹری پوائنٹ `register_and_mint` پر کال کریں۔
- کامیاب رہائی کی تصدیق کے ل I `client.request(FindAccountAssets)` یا `iroha_cli ledger assets list --account <i105-account-id>` کے ذریعے بیلنس چیک کریں۔

## متعلقہ SDK سبق

- [کوئیک اسٹارٹ مورچا SDK] (/sdks/rust)
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
    let to = account!("<i105-account-id>");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```