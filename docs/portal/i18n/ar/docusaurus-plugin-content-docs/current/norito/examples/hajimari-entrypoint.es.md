---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/hajimari-entrypoint.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/hajimari-entrypoint
العنوان: Esqueleto del Entrypoint Hajimari
الوصف: الحد الأدنى من العقد Kotodama مع نقطة دخول عامة واحدة ومدير الحالة.
المصدر:crates/ivm/docs/examples/01_hajimari.ko
---

احصل على الحد الأدنى من العقد Kotodama مع نقطة دخول عامة واحدة ومدير الحالة.

## Recorrido del libro mayor

- قم بتجميع العقد مع `koto_compile --abi 1` كما يظهر في [بداية Norito](/norito/getting-started#1-compile-a-kotodama-contract) أو في منتصف `cargo test -p ivm developer_portal_norito_snippets_compile`.
- قم بإجراء اختبار سريع للرمز الثانوي المحلي باستخدام `ivm_run` / `developer_portal_norito_snippets_run` للتحقق من السجل `info!` واستدعاء النظام الأولي قبل النقر على عقدة.
- اعرض المنتج مع `iroha_cli app contracts deploy` وأكد البيان باستخدام الخطوات [Inicio de Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## أدلة SDK ذات الصلة

- [Quickstart del SDK de Rust](/sdks/rust)
- [Quickstart del SDK de Python](/sdks/python)
- [بدء التشغيل السريع لـ SDK لـ JavaScript](/sdks/javascript)

[تنزيل مصدر Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```