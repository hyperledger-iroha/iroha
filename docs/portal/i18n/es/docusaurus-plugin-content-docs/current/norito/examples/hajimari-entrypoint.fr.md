---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/examples/hajimari-entrypoint
título: Squelette du point d'entrée Hajimari
descripción: Estructura mínima de contrato Kotodama con un solo punto de entrada pública y un gestor de estado.
fuente: crates/ivm/docs/examples/01_hajimari.ko
---

Estructura mínima de contrato Kotodama con un solo punto de entrada pública y un gestor de estado.

## Rutas del registro

- Compile el contrato con `koto_compile --abi 1` como se indica en [Démarrage de Norito](/norito/getting-started#1-compile-a-kotodama-contract) o mediante `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Realice una prueba de humo del código de bytes en local con `ivm_run` / `developer_portal_norito_snippets_run` para verificar el registro `info!` y la llamada al sistema inicial antes de tocar un nuevo.
- Implemente el artefacto a través de `iroha_cli app contracts deploy` y confirme el manifiesto en las siguientes etapas de [Démarrage de Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## Guías SDK asociadas

- [Inicio rápido SDK Rust](/sdks/rust)
- [Inicio rápido SDK Python](/sdks/python)
- [Inicio rápido SDK JavaScript](/sdks/javascript)

[Descargar la fuente Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```