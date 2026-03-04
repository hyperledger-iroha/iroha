---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.pt.md
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

یہ ظاہر کرتا ہے کہ کس طرح Kotodama انٹری پوائنٹ `transfer_asset` میزبان ہدایت کو ان لائن میٹا ڈیٹا کی توثیق کے ساتھ کال کرسکتا ہے۔

## لیجر اسکرپٹ

- معاہدہ اتھارٹی (مثال کے طور پر `ih58...`) کو اس اثاثہ کے ساتھ فنڈ دیں جو اس نے منتقل کیا ہے اور اتھارٹی کو کاغذ `CanTransfer` یا مساوی اجازت دیں۔
- معاہدہ اکاؤنٹ سے 5 یونٹوں کو `ih58...` میں منتقل کرنے کے لئے انٹری پوائنٹ `call_transfer_asset` پر کال کریں ، اس بات کی عکاسی کرتے ہوئے کہ آن چین آٹومیشن میں میزبان کالوں کو کس طرح شامل کیا جاسکتا ہے۔
- `FindAccountAssets` یا `iroha_cli ledger assets list --account ih58...` کے ذریعے توازن چیک کریں اور اس بات کی تصدیق کے لئے واقعات کا معائنہ کریں کہ میٹا ڈیٹا گارڈ نے منتقلی کے سیاق و سباق کو ریکارڈ کیا۔

## متعلقہ SDK گائیڈز

- [مورچا SDK کوئیک اسٹارٹ] (/sdks/rust)
- [ازگر ایس ڈی کے کوئیک اسٹارٹ] (/sdks/python)
- [جاوا اسکرپٹ SDK کوئیک اسٹارٹ] (/sdks/javascript)

[ماخذ Kotodama ڈاؤن لوڈ کریں] (/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("ih58..."),
      account!("ih58..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```