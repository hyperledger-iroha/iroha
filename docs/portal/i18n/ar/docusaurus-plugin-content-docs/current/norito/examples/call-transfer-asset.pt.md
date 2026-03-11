---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/call-transfer-asset
العنوان: يستضيف Invocar Transferencia جزء من Kotodama
الوصف: يمكن استخدام نقطة الدخول Kotodama من خلال تعليمات المضيف `transfer_asset` مع التحقق من صحة البيانات الوصفية المضمنة.
المصدر: صناديق/ivm/docs/examples/08_call_transfer_asset.ko
---

يمكن أن يكون مثل نقطة الإدخال Kotodama من خلال تعليمات المضيف `transfer_asset` مع التحقق من صحة البيانات الوصفية المضمنة.

## Roteiro do livro razao

- تمويل التفويض التعاقدي (على سبيل المثال `i105...`) من خلال القيام بنقل والتنازل عن التفويض أو الورق `CanTransfer` أو السماح بما يعادل ذلك.
- اسم نقطة الدخول `call_transfer_asset` لنقل 5 وحدات من عقد الحساب لـ `i105...`، كما يمكن أن يشمل الاتصال التلقائي على السلسلة مكالمات المضيف.
- التحقق من البيانات عبر `FindAccountAssets` أو `iroha_cli ledger assets list --account i105...` وفحص الأحداث للتأكد من أن حارس التعريفات يسجل سياق النقل.

## أدلة SDK ذات الصلة

- [بدء التشغيل السريع لـ SDK Rust](/sdks/rust)
- [البدء السريع لـ SDK Python](/sdks/python)
- [بدء التشغيل السريع لـ SDK JavaScript](/sdks/javascript)

[اضغط على الخط Kotodama](/norito-snippets/call-transfer-asset.ko)

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