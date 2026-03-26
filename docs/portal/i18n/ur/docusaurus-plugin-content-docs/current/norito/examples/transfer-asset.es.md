---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/transfer-asset.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/ٹرانسفر اثاثہ
عنوان: اکاؤنٹس کے مابین اثاثہ کی منتقلی
تفصیل: براہ راست اثاثہ منتقلی کا بہاؤ جو SDK کوئیک اسٹارٹ اور لیجر واک تھرو کی عکاسی کرتا ہے۔
ماخذ: مثالوں/منتقلی/منتقلی.KO
---

براہ راست اثاثہ کی منتقلی کا بہاؤ جو SDK کوئیک اسٹارٹ اور لیجر واک تھرو کی عکاسی کرتا ہے۔

## لیجر ٹور

- ہدف اثاثہ کے ساتھ پری فنڈ ایلس (جیسے ٹکڑے کے ذریعے `register and mint` یا SDK کوئیک اسٹارٹ بہاؤ)۔
- ایلس سے باب میں 10 یونٹ منتقل کرنے کے لئے انٹری پوائنٹ `do_transfer` پر عمل کریں ، اجازت `AssetTransferRole` کو پورا کریں۔
- بیلنس سے مشورہ کریں (`FindAccountAssets` ، `iroha_cli ledger assets list`) یا منتقلی کے نتیجے میں مشاہدہ کرنے کے لئے پائپ لائن واقعات کو سبسکرائب کریں۔

## متعلقہ SDK گائیڈز

- [مورچا SDK کوئیک اسٹارٹ] (/sdks/rust)
- [ازگر ایس ڈی کے کوئیک اسٹارٹ] (/sdks/python)
- [جاوا اسکرپٹ SDK کوئیک اسٹارٹ] (/sdks/javascript)

[Kotodama کا ماخذ ڈاؤن لوڈ کریں] (/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of 62Fk4FPcMuLvW5QjDGNF2a4jAmjM from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("<katakana-i105-account-id>"),
      account!("<katakana-i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```