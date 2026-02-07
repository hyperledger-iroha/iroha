---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/call-transfer-asset
título: Kotodama سے ہوسٹ ٹرانسفر کال کریں
descripción: دکھاتا ہے کہ Kotodama انٹری پوائنٹ کس طرح ہوسٹ کی `transfer_asset` انسٹرکشن کو inline میٹا ڈیٹا ویلیڈیشن کے ساتھ کال کر سکتا ہے۔
fuente: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

دکھاتا ہے کہ Kotodama انٹری پوائنٹ کس طرح ہوسٹ کی `transfer_asset` انسٹرکشن کو en línea ڈیٹا ویلیڈیشن کے ساتھ کال کر سکتا ہے۔

## لیجر واک تھرو

- کنٹریکٹ اتھارٹی (مثلا `ih58...`) کو اس اثاثے سے فنڈ کریں جسے وہ منتقل کرے گی اور اتھارٹی کو `CanTransfer` رول یا مساوی اجازت دیں۔
- `call_transfer_asset` انٹری پوائنٹ کال کریں تاکہ کنٹریکٹ اکاؤنٹ سے `ih58...` کو 5 یونٹس منتقل ہوں، یہ اس طریقے کی عکاسی کرتا ہے کہ آن چین آٹومیشن ہوسٹ کالز کو لپیٹ سکتی ہے۔
- `FindAccountAssets` یا `iroha_cli ledger assets list --account ih58...` کے ذریعے بیلنس دیکھیں اور ایونٹس چیک کریں تاکہ تصدیق ہو کہ میٹا ڈیٹا گارڈ نے ٹرانسفر کانٹیکسٹ لاگ کیا ہے۔

## متعلقہ SDK گائیڈز

- [Inicio rápido del SDK de Rust](/sdks/rust)
- [Inicio rápido del SDK de Python](/sdks/python)
- [Inicio rápido del SDK de JavaScript](/sdks/javascript)

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