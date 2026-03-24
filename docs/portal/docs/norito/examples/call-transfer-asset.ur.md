---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a91fc8841580a836c80129942df7f79f5bc5dd5f6a72dccf1394b740d02536a5
source_last_modified: "2025-11-23T15:30:33.687233+00:00"
translation_last_reviewed: 2026-01-30
---

---
slug: /norito/examples/call-transfer-asset
title: Kotodama سے ہوسٹ ٹرانسفر کال کریں
description: دکھاتا ہے کہ Kotodama انٹری پوائنٹ کس طرح ہوسٹ کی `transfer_asset` انسٹرکشن کو inline میٹا ڈیٹا ویلیڈیشن کے ساتھ کال کر سکتا ہے۔
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

دکھاتا ہے کہ Kotodama انٹری پوائنٹ کس طرح ہوسٹ کی `transfer_asset` انسٹرکشن کو inline میٹا ڈیٹا ویلیڈیشن کے ساتھ کال کر سکتا ہے۔

## لیجر واک تھرو

- کنٹریکٹ اتھارٹی (مثلا `i105...`) کو اس اثاثے سے فنڈ کریں جسے وہ منتقل کرے گی اور اتھارٹی کو `CanTransfer` رول یا مساوی اجازت دیں۔
- `call_transfer_asset` انٹری پوائنٹ کال کریں تاکہ کنٹریکٹ اکاؤنٹ سے `i105...` کو 5 یونٹس منتقل ہوں، یہ اس طریقے کی عکاسی کرتا ہے کہ آن چین آٹومیشن ہوسٹ کالز کو لپیٹ سکتی ہے۔
- `FindAccountAssets` یا `iroha_cli ledger assets list --account i105...` کے ذریعے بیلنس دیکھیں اور ایونٹس چیک کریں تاکہ تصدیق ہو کہ میٹا ڈیٹا گارڈ نے ٹرانسفر کانٹیکسٹ لاگ کیا ہے۔

## متعلقہ SDK گائیڈز

- [Rust SDK quickstart](/sdks/rust)
- [Python SDK quickstart](/sdks/python)
- [JavaScript SDK quickstart](/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("i105..."),
      account!("i105..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```
