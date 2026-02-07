---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/examples/hajimari-entrypoint
título: هيكل نقطة دخول Hajimari
descripción: هيكل عقد Kotodama بسيط بنقطة دخول عامة واحدة ومقبض حالة.
fuente: crates/ivm/docs/examples/01_hajimari.ko
---

Asegúrese de que Kotodama esté encendido y apagado.

## جولة دفتر الأستاذ

- قم بتجميع العقد باستخدام `koto_compile --abi 1` كما هو موضح في [البدء مع Norito](/norito/getting-started#1-compile-a-kotodama-contract) أو عبر `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Para el hogar o para el hogar `ivm_run` / `developer_portal_norito_snippets_run` para el hogar `info!` والنداء النظامي الأول قبل لمس عقدة.
- انشر الأثر عبر `iroha_cli app contracts deploy` y أكد البيان باستخدام الخطوات في [البدء مع Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## أدلة SDK ذات صلة

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [Aplicación del SDK de Python](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[Actualización Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```