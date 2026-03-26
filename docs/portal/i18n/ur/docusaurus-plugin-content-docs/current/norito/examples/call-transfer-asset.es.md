---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/کال ٹرانسفر-اثاثہ
عنوان: Kotodama سے میزبان میزبان کی منتقلی
تفصیل: یہ ظاہر کرتا ہے کہ کس طرح Kotodama انٹری پوائنٹ `transfer_asset` میزبان ہدایت کو ان لائن میٹا ڈیٹا کی توثیق کے ساتھ کال کرسکتا ہے۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/08_Call_transfer_asset.ko
---

یہ ظاہر کرتا ہے کہ کس طرح Kotodama انٹری پوائنٹ آن لائن میٹا ڈیٹا کی توثیق کے ساتھ `transfer_asset` میزبان ہدایت پر کال کرسکتا ہے۔

## لیجر ٹور

- معاہدہ اتھارٹی (مثال کے طور پر `i105...`) کو اس اثاثہ کے ساتھ فنڈ دیں جس کی آپ منتقلی کریں گے اور اس کو `CanTransfer` یا مساوی اجازت دیں گے۔
- کال انٹریپوائنٹ `call_transfer_asset` معاہدے کے اکاؤنٹ سے 5 یونٹوں کو `i105...` میں منتقل کرنے کے لئے ، جس طرح سے آن چین آٹومیشن میں میزبان کالز شامل ہوسکتی ہیں اس کی عکاسی کرتی ہے۔
- `FindAccountAssets` یا `iroha_cli ledger assets list --account i105...` کا استعمال کرتے ہوئے بیلنس چیک کرتا ہے اور اس بات کی تصدیق کرنے کے لئے واقعات کا معائنہ کرتا ہے کہ میٹا ڈیٹا گارڈ نے منتقلی کے سیاق و سباق کو ریکارڈ کیا۔

## متعلقہ SDK گائیڈز

- [مورچا SDK کوئیک اسٹارٹ] (/sdks/rust)
- [ازگر ایس ڈی کے کوئیک اسٹارٹ] (/sdks/python)
- [جاوا اسکرپٹ SDK کوئیک اسٹارٹ] (/sdks/javascript)

[Kotodama کا ماخذ ڈاؤن لوڈ کریں] (/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("soraカタカナ..."),
      account!("soraカタカナ..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```