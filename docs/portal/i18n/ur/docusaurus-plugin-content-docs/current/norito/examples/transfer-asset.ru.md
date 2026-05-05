---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/transfer-asset.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/ٹرانسفر اثاثہ
عنوان: اکاؤنٹس کے مابین اثاثہ منتقل کریں
تفصیل: ایس ڈی کے کوئیک اسٹارٹ اور رجسٹری واک تھرو کی عکاسی کرنے والا ایک سادہ اثاثہ ٹرانسفر اسکرپٹ۔
ماخذ: مثالوں/منتقلی/منتقلی.KO
---

ایس ڈی کے کے کوئیک اسٹارٹ اور رجسٹری واک تھرو کی عکاسی کرنے والا ایک آسان اثاثہ منتقلی کا منظر۔

## قدم بہ قدم رجسٹری ٹریورسل

- ہدف اثاثہ کے ساتھ پہلے سے بھریں (مثال کے طور پر `register and mint` اسنیپٹ یا کوئیک اسٹارٹ SDK اسٹریمز کے ذریعے)۔
- ایلس سے باب میں 10 یونٹوں کی منتقلی کے لئے انٹری پوائنٹ `do_transfer` پر عمل کریں ، اجازت نامہ `AssetTransferRole`۔
- توازن چیک کریں (`FindAccountAssets` ، `iroha_cli ledger assets list`) یا منتقلی کے نتیجے میں مشاہدہ کرنے کے لئے پائپ لائن واقعات کو سبسکرائب کریں۔

## متعلقہ SDK سبق

- [کوئیک اسٹارٹ مورچا SDK] (/sdks/rust)
- [کوئک اسٹارٹ ازگر ایس ڈی کے] (/sdks/python)
- [کوئیک اسٹارٹ جاوا اسکرپٹ SDK] (/sdks/javascript)

[ماخذ Kotodama ڈاؤن لوڈ کریں] (/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of 62Fk4FPcMuLvW5QjDGNF2a4jAmjM from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("<i105-account-id>"),
      account!("<i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```