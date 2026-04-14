---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d7cdc798ae9f1b69608c2b287a76a06a3d8d51116185ff6a5326e755b7b05bd
source_last_modified: "2026-04-08T09:19:38.794575+00:00"
translation_last_reviewed: 2026-04-08
---

---
slug: /norito/examples/call-transfer-asset
title: Kotodama سے ہوسٹ ٹرانسفر کال کریں
description: دکھاتا ہے کہ Kotodama انٹری پوائنٹ کس طرح ہوسٹ کی `transfer_asset` انسٹرکشن کو inline میٹا ڈیٹا ویلیڈیشن کے ساتھ کال کر سکتا ہے۔
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

دکھاتا ہے کہ Kotodama انٹری پوائنٹ کس طرح ہوسٹ کی `transfer_asset` انسٹرکشن کو inline میٹا ڈیٹا ویلیڈیشن کے ساتھ کال کر سکتا ہے۔

## لیجر واک تھرو

- کنٹریکٹ اتھارٹی (مثلا `<i105-account-id>`) کو اس اثاثے سے فنڈ کریں جسے وہ منتقل کرے گی اور اتھارٹی کو `CanTransfer` رول یا مساوی اجازت دیں۔
- `call_transfer_asset` انٹری پوائنٹ کال کریں تاکہ کنٹریکٹ اکاؤنٹ سے `<i105-account-id>` کو 5 یونٹس منتقل ہوں، یہ اس طریقے کی عکاسی کرتا ہے کہ آن چین آٹومیشن ہوسٹ کالز کو لپیٹ سکتی ہے۔
- `FindAccountAssets` یا `iroha ledger asset list all --verbose` کے ذریعے بیلنس دیکھیں اور ایونٹس چیک کریں تاکہ تصدیق ہو کہ میٹا ڈیٹا گارڈ نے ٹرانسفر کانٹیکسٹ لاگ کیا ہے۔

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
      account!("<i105-account-id>"),
      account!("<i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```
