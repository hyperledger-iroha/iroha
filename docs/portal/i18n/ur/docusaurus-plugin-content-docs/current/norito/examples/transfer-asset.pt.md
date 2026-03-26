---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/transfer-asset.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/ٹرانسفر اثاثہ
عنوان: اکاؤنٹس کے مابین اثاثہ کی منتقلی
تفصیل: براہ راست اثاثہ کی منتقلی کا بہاؤ جو SDK کوئیک اسٹارٹ اور لیجر اسکرپٹس کو آئینہ دیتا ہے۔
ماخذ: مثالوں/منتقلی/منتقلی.KO
---

براہ راست اثاثہ منتقلی کا بہاؤ جو SDK کوئیک اسٹارٹ اور لیجر اسکرپٹس کو آئینہ دیتا ہے۔

## لیجر اسکرپٹ

- ہدف اثاثہ کے ساتھ پری فنڈ ایلس (مثال کے طور پر `register and mint` اسنیپٹ یا SDK کوئیک اسٹارٹ بہاؤ کے ذریعے)۔
- انٹری پوائنٹ `do_transfer` پر عمل کریں تاکہ 10 یونٹوں کو ایلس سے باب میں منتقل کریں ، اجازت سے ملاقات کریں `AssetTransferRole`۔
- بیلنس سے مشورہ کریں (`FindAccountAssets` ، `iroha_cli ledger assets list`) یا منتقلی کے نتائج کا مشاہدہ کرنے کے لئے پائپ لائن واقعات کو سبسکرائب کریں۔

## متعلقہ SDK گائیڈز

- [مورچا SDK کوئیک اسٹارٹ] (/sdks/rust)
- [ازگر ایس ڈی کے کوئیک اسٹارٹ] (/sdks/python)
- [جاوا اسکرپٹ SDK کوئیک اسٹارٹ] (/sdks/javascript)

[ماخذ Kotodama ڈاؤن لوڈ کریں] (/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of 62Fk4FPcMuLvW5QjDGNF2a4jAmjM from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("soraカタカナ..."),
      account!("soraカタカナ..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```