---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/کال ٹرانسفر-اثاثہ
عنوان: Kotodama سے میزبان ٹرانسفر
تفصیل: یہ ظاہر کرتا ہے کہ کس طرح Kotodama انٹری پوائنٹ بلٹ ان میٹا ڈیٹا چیکنگ کے ساتھ `transfer_asset` میزبان ہدایت کو کال کرسکتا ہے۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/08_Call_transfer_asset.ko
---

ظاہر کرتا ہے کہ کس طرح Kotodama انٹری پوائنٹ بلٹ ان میٹا ڈیٹا چیکنگ کے ساتھ `transfer_asset` میزبان ہدایت پر کال کرسکتا ہے۔

## قدم بہ قدم رجسٹری ٹریورسل

- کسی معاہدے کے اتھارٹی (جیسے `ih58...`) کو اثاثہ کے ساتھ تقویت بخشیں وہ اتھارٹی کو `CanTransfer` یا مساوی اجازت کی منتقلی اور جاری کرے گی۔
- کال انٹری پوائنٹ `call_transfer_asset` کو معاہدہ اکاؤنٹ سے 5 یونٹوں کو `ih58...` میں منتقل کرنے کے لئے ، اس بات کی عکاسی کرتی ہے کہ آن چین آٹومیشن میزبان کالوں کو کیسے لپیٹ سکتا ہے۔
- `FindAccountAssets` یا `iroha_cli ledger assets list --account ih58...` کے ذریعے بیلنس چیک کریں اور اس بات کی تصدیق کے ل investims واقعات کا جائزہ لیں کہ میٹا ڈیٹا گارڈ نے منتقلی کا سیاق و سباق ریکارڈ کیا ہے۔

## متعلقہ SDK سبق

- [کوئیک اسٹارٹ مورچا SDK] (/sdks/rust)
- [کوئیک اسٹارٹ ازگر ایس ڈی کے] (/sdks/python)
- [کوئیک اسٹارٹ جاوا اسکرپٹ SDK] (/sdks/javascript)

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