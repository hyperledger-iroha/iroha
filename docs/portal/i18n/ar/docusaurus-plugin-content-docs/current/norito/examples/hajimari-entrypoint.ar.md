---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/hajimari-entrypoint.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/hajimari-entrypoint
العنوان: هيكل نقطة دخول الحجيماري
الوصف: هيكل عقد Kotodama بسيط بنقطة عامة دخول واحد ومقبض حالة.
المصدر:crates/ivm/docs/examples/01_hajimari.ko
---

هيكل عقد Kotodama بسيط بنقطة واحدة عامة دخول ومقبض الحالة.

## جولة أستاذ الأستاذ

- قم بتجميع العقد باستخدام `koto_compile --abi 1` كما هو موضح في [البدء مع Norito](/norito/getting-started#1-compile-a-kotodama-contract) أو عبر `cargo test -p ivm developer_portal_norito_snippets_compile`.
- أجر اختبار دخان للبايت كود المحلي باستخدام `ivm_run` / `developer_portal_norito_snippets_run`.
- نشر الأثر عبر `iroha_cli app contracts deploy` التحرك باستخدام الخطوات في [البدء مع Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## دليل SDK ذات صلة

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```