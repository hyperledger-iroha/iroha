---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/کال ٹرانسفر-اثاثہ
عنوان: Kotodama سے میزبان ٹرانسفر
تفصیل: یہ ظاہر کرتا ہے کہ کس طرح انٹری پوائنٹ Kotodama بلٹ ان میٹا ڈیٹا چیکنگ کے ساتھ میزبان ہدایت `transfer_asset` کو کال کرسکتا ہے۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/08_Call_transfer_asset.ko
---

یہ ظاہر کرتا ہے کہ کس طرح انٹری پوائنٹ Kotodama بلٹ میں میٹا ڈیٹا چیکنگ کے ساتھ میزبان ہدایت `transfer_asset` پر کال کرسکتا ہے۔

## لیجر ٹور

- معاہدہ اتھارٹی (مثال کے طور پر `i105...`) کو اس اثاثہ کے ساتھ فنڈ دیں جس کی آپ منتقلی کر رہے ہیں اور اتھارٹی کو `CanTransfer` یا مساوی اجازت کا کردار دیں۔
- انٹری پوائنٹ `call_transfer_asset` کو 5 نوڈ اکاؤنٹ یونٹوں کو `i105...` پر منتقل کرنے کے لئے کال کریں ، جس طرح آن چین آٹومیشن میزبان کالوں کو گھیرے میں لے کر آئینہ دار ہے۔
- `FindAccountAssets` یا `iroha_cli ledger assets list --account i105...` کے ذریعے توازن چیک کریں اور واقعات کی جانچ پڑتال کریں تاکہ اس بات کی تصدیق کی جاسکے کہ میٹا ڈیٹا گارڈ نے منتقلی کے سیاق و سباق کو ریکارڈ کیا۔

## متعلقہ SDK گائیڈز

- [مورچا SDK کوئیک اسٹارٹ] (/sdks/rust)
- [ازگر SDK کوئیک اسٹارٹ] (/sdks/python)
- [جاوا اسکرپٹ SDK کوئیک اسٹارٹ] (/sdks/javascript)

[ماخذ Kotodama ڈاؤن لوڈ کریں] (/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("i105..."),
      account!("i105..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```