---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
スラッグ: /norito/examples/call-transfer-asset
title: Kotodama سے ہوسٹ ٹرانسفر کال کریں
説明: دکھاتا ہے کہ Kotodama انٹری پوائنٹ کس طرح ہوسٹ کی `transfer_asset` انسٹرکشن کو インライン میٹا ڈیٹا ویلیڈیشن کے ساتھ کال کر سکتا ہے۔
ソース: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

دکھاتا ہے کہ Kotodama انٹری پوائنٹ کس طرح ہوسٹ کی `transfer_asset` انسٹرکشن کو インライン میٹا ڈیٹا ویلیڈیشن کے ساتھ کال کر سکتا ہے۔

## ٩جر واک تھرو

- کنٹریکٹ اتھارٹی (مثلا `<i105-account-id>`) کو اس اثاثے سے فنڈ کریں جسے وہ منتقل کرے گی اور اتھارٹی کو `CanTransfer` رول یا مساوی اجازت دیں۔
- `call_transfer_asset` انٹری پوائنٹ کال کریں تاکہ کنٹریکٹ اکاؤنٹ سے `<i105-account-id>` کو 5 یونٹس منتقل ہوں، یہ اس طریقے کی عکاسی کرتا ہے کہ آن چین آٹومیشن ہوسٹ کالز کو لپیٹ سکتی ❁❁❁❁
- `FindAccountAssets` یا `iroha_cli ledger assets list --account <i105-account-id>` کے ذریعے بیلنس دیکھیں اور ایونٹس چیک کریں تاکہ تصدیق ہو کہ میٹا ڈیٹا گارڈ نے ٹرانسفر کانٹیکسٹ لاگ کیا ہے۔

## SDK の開発

- [Rust SDK クイックスタート](/sdks/rust)
- [Python SDK クイックスタート](/sdks/python)
- [JavaScript SDK クイックスタート](/sdks/javascript)

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