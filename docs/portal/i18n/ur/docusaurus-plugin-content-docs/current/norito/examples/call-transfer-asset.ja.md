---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a99f9e23c9d9413f8624f6f1e0da9b4d1ffb7721a72bfa8e85408b8d9c75234c
source_last_modified: "2026-01-22T15:55:00+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/call-transfer-asset
title: Kotodama سے ہوسٹ ٹرانسفر کال کریں
description: دکھاتا ہے کہ Kotodama انٹری پوائنٹ کس طرح ہوسٹ کی `transfer_asset` انسٹرکشن کو inline میٹا ڈیٹا ویلیڈیشن کے ساتھ کال کر سکتا ہے۔
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

دکھاتا ہے کہ Kotodama انٹری پوائنٹ کس طرح ہوسٹ کی `transfer_asset` انسٹرکشن کو inline میٹا ڈیٹا ویلیڈیشن کے ساتھ کال کر سکتا ہے۔

## لیجر واک تھرو

- کنٹریکٹ اتھارٹی (مثلا `ih58...`) کو اس اثاثے سے فنڈ کریں جسے وہ منتقل کرے گی اور اتھارٹی کو `CanTransfer` رول یا مساوی اجازت دیں۔
- `call_transfer_asset` انٹری پوائنٹ کال کریں تاکہ کنٹریکٹ اکاؤنٹ سے `ih58...` کو 5 یونٹس منتقل ہوں، یہ اس طریقے کی عکاسی کرتا ہے کہ آن چین آٹومیشن ہوسٹ کالز کو لپیٹ سکتی ہے۔
- `FindAccountAssets` یا `iroha_cli ledger assets list --account ih58...` کے ذریعے بیلنس دیکھیں اور ایونٹس چیک کریں تاکہ تصدیق ہو کہ میٹا ڈیٹا گارڈ نے ٹرانسفر کانٹیکسٹ لاگ کیا ہے۔

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
      account!("ih58..."),
      account!("ih58..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```
