---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/hajimari-entrypoint.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/hajimari-entrypoint
العنوان: Squelette du point d'entrée Hajimari
الوصف: هيكل الحد الأدنى من العقد Kotodama مع نقطة دخول عامة واحدة ومدير حالة.
المصدر:crates/ivm/docs/examples/01_hajimari.ko
---

هيكل الحد الأدنى من العقد Kotodama مع نقطة دخول عامة واحدة ومدير حالة.

## باركور دو ريجيستري

- قم بتجميع العقد مع `koto_compile --abi 1` كما هو موضح في [Démarrage de Norito](/norito/getting-started#1-compile-a-kotodama-contract) أو عبر `cargo test -p ivm developer_portal_norito_snippets_compile`.
- قم بإجراء اختبار دخان للرمز الثانوي محليًا باستخدام `ivm_run` / `developer_portal_norito_snippets_run` للتحقق من السجل `info!` واستدعاء النظام الأولي قبل لمس مرة واحدة.
- قم بنشر المنتج عبر `iroha_cli app contracts deploy` وقم بتأكيد البيان بعد خطوات [Démarrage de Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## أدلة شركاء SDK

- [Quickstart SDK Rust](/sdks/rust)
- [Quickstart SDK Python](/sdks/python)
- [Quickstart SDK JavaScript](/sdks/javascript)

[تحميل المصدر Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```