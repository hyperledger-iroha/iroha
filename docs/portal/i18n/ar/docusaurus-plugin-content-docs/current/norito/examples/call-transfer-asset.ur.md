---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/call-transfer-asset
العنوان: Kotodama سياحة السفر
الوصف: ميزة Kotodama للنقطة الداخلية التي تم إنشاؤها بواسطة `transfer_asset` للإنترنيت المضمنة في ويلايتشن على شكل سكاكا ہے۔
المصدر: صناديق/ivm/docs/examples/08_call_transfer_asset.ko
---

الإضافة إلى قائمة Kotodama للنقطة الداخلية بالإضافة إلى `transfer_asset` للإنترنيت المضمن أو نقرة واحدة من ويلمنج ہے۔

## ليجر واک تھرو

- تم السماح للبطاقة الإلكترونية (مثل `i105...`) التي تعمل بتقنية الأثاث والألعاب بدورة أو إجراء `CanTransfer`.
- `call_transfer_asset` تنتقل بطاقة الائتمان عبر الإنترنت `i105...` إلى 5 يونيو، أو من خلال هذه المهارة. ما الذي يجعل هذه الآلة أكثر ذكاءً؟
- `FindAccountAssets` أو `iroha_cli ledger assets list --account i105...` لا يمكن نقل أي شيء آخر إلى أي شيء آخر. كان الكنست حاضرًا.

## مواضيع ذات صلة SDK

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[Kotodama تنزيل التنزيل](/norito-snippets/call-transfer-asset.ko)

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