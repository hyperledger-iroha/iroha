---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/norito/examples/hajimari-entrypoint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7eaf8a500a8d18d5061ee27bb71b90254bdfcd7e833b4da092c7cf416f8868fd
source_last_modified: "2025-11-14T04:43:20.715009+00:00"
translation_last_reviewed: 2026-01-30
---

هيكل عقد Kotodama بسيط بنقطة دخول عامة واحدة ومقبض حالة.

## جولة دفتر الأستاذ

- قم بتجميع العقد باستخدام `koto_compile --abi 1` كما هو موضح في [البدء مع Norito](/norito/getting-started#1-compile-a-kotodama-contract) أو عبر `cargo test -p ivm developer_portal_norito_snippets_compile`.
- أجر اختبار دخان للبايت كود محليا باستخدام `ivm_run` / `developer_portal_norito_snippets_run` للتحقق من سجل `info!` والنداء النظامي الأول قبل لمس عقدة.
- انشر الأثر عبر `iroha_cli app contracts deploy` وأكد البيان باستخدام الخطوات في [البدء مع Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## أدلة SDK ذات صلة

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
