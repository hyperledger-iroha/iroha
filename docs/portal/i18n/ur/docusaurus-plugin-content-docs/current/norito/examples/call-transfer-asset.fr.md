---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.fr.md
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

## رجسٹری براؤزنگ

- معاہدہ اتھارٹی (جیسے `<katakana-i105-account-id>`) کے اثاثے کے ساتھ یہ فراہمی اور اس کو `CanTransfer` یا مساوی اجازت کی فراہمی اور اس کی فراہمی کرے گی۔
- کال انٹری پوائنٹ `call_transfer_asset` کو معاہدہ اکاؤنٹ سے 5 یونٹوں کو `<katakana-i105-account-id>` میں منتقل کرنے کے لئے ، اس بات کی عکاسی کرتی ہے کہ آن چین آٹومیشن میزبان کالوں کو کس طرح گھیر سکتا ہے۔
- `FindAccountAssets` یا `iroha_cli ledger assets list --account <katakana-i105-account-id>` کے ذریعے بیلنس چیک کریں اور واقعات کا معائنہ کریں تاکہ اس بات کی تصدیق کی جاسکے کہ میٹا ڈیٹا گارڈ نے منتقلی کے تناظر میں لاگ ان کیا ہے۔

## متعلقہ SDK گائیڈز

- [کوئک اسٹارٹ ایس ڈی کے زنگ] (/sdks/rust)
- [کوئیک اسٹارٹ ازگر ایس ڈی کے] (/sdks/python)
- [کوئیک اسٹارٹ جاوا اسکرپٹ SDK] (/sdks/javascript)

[ماخذ Kotodama ڈاؤن لوڈ کریں] (/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("<katakana-i105-account-id>"),
      account!("<katakana-i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```