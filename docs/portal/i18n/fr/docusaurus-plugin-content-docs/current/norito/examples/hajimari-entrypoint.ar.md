---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
limace : /norito/examples/hajimari-entrypoint
titre : هيكل نقطة دخول Hajimari
description: هيكل عقد Kotodama بسيط بنقطة دخول عامة واحدة ومقبض حالة.
source : crates/ivm/docs/examples/01_hajimari.ko
---

هيكل عقد Kotodama بسيط بنقطة دخول عامة واحدة ومقبض حالة.

## جولة دفتر الأستاذ

- قم بتجميع العقد باستخدام `koto_compile --abi 1` كما هو موضح في [البدء مع Norito](/norito/getting-started#1-compile-a-kotodama-contract) أو عبر `cargo test -p ivm developer_portal_norito_snippets_compile`.
- أجر اختبار دخان للبايت كود محليا باستخدام `ivm_run` / `developer_portal_norito_snippets_run` للتحقق من سجل `info!` والنداء النظامي الأول قبل لمس عقدة.
- انشر الأثر عبر `iroha_cli app contracts deploy` وأكد البيان باستخدام الخطوات في [البدء مع Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## Le SDK est disponible

- [Détails du SDK Rust](/sdks/rust)
- [Détails du SDK Python](/sdks/python)
- [Détails du SDK JavaScript](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```