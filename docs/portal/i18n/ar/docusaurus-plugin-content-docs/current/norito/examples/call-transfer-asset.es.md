---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/call-transfer-asset
العنوان: استدعاء نقل المضيف من Kotodama
الوصف: يمكنك استخدام نقطة دخول Kotodama للاتصال بتعليمات المضيف `transfer_asset` مع التحقق من صحة البيانات الوصفية عبر الإنترنت.
المصدر: صناديق/ivm/docs/examples/08_call_transfer_asset.ko
---

باستخدام نقطة دخول Kotodama يمكنك الاتصال بتعليمات المضيف `transfer_asset` مع التحقق من صحة البيانات التعريفية عبر الإنترنت.

## Recorrido del libro mayor

- قم بإنشاء ترخيص العقد (على سبيل المثال `i105...`) مع النشاط الذي ينقل ويحرر الدور `CanTransfer` أو تصريح مكافئ.
- اتصل بنقطة الدخول `call_transfer_asset` لنقل 5 وحدات من حساب العقد إلى `i105...`، مما يعكس الطريقة التي يمكن أن تشمل بها الأتمتة على السلسلة مكالمات المضيف.
- التحقق من الأرصدة بين `FindAccountAssets` أو `iroha_cli ledger assets list --account i105...` وفحص الأحداث للتأكد من أن حماية البيانات تسجل سياق النقل.

## أدلة SDK ذات الصلة

- [بدء التشغيل السريع لـ SDK de Rust](/sdks/rust)
- [Quickstart del SDK de Python](/sdks/python)
- [بدء التشغيل السريع لـ SDK لـ JavaScript](/sdks/javascript)

[تنزيل مصدر Kotodama](/norito-snippets/call-transfer-asset.ko)

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