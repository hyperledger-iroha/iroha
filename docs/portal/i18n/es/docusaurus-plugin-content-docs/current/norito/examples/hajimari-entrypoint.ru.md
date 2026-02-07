---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/examples/hajimari-entrypoint
título: Каркас входной точки Hajimari
descripción: Contacto de tarjeta mínima Kotodama con una conexión pública única y una configuración única.
fuente: crates/ivm/docs/examples/01_hajimari.ko
---

El contacto mínimo de la tarjeta Kotodama con la información pública habitual y la configuración actual.

## Пошаговый обход реестра

- Complete el contrato con `koto_compile --abi 1` y coloque en [Norito Getting Started](/norito/getting-started#1-compile-a-kotodama-contract) o en `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Haga una prueba de humo localmente con `ivm_run` / `developer_portal_norito_snippets_run`, compruebe el registro `info!` y syscall antes del tema, как трогать узел.
- Revise el artefacto con `iroha_cli app contracts deploy` y realice el manifiesto, implemente la configuración [Norito Getting Started](/norito/getting-started#4-deploy-via-iroha_cli).

## Связанные руководства SDK

- [SDK de inicio rápido de Rust](/sdks/rust)
- [SDK de Python de inicio rápido](/sdks/python)
- [SDK de JavaScript de inicio rápido](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```