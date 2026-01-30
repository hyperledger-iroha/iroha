---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/norito/examples/hajimari-entrypoint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2cd9808832e54d861ae4d7cf2e5131807db9d59f34f7474be93f28361959325
source_last_modified: "2025-11-14T04:43:20.705560+00:00"
translation_last_reviewed: 2026-01-30
---

Andamiaje mínimo de contrato Kotodama con un único entrypoint público y un manejador de estado.

## Recorrido del libro mayor

- Compila el contrato con `koto_compile --abi 1` como se muestra en [Inicio de Norito](/norito/getting-started#1-compile-a-kotodama-contract) o mediante `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Haz una prueba rápida del bytecode localmente con `ivm_run` / `developer_portal_norito_snippets_run` para verificar el log `info!` y el syscall inicial antes de tocar un nodo.
- Despliega el artefacto con `iroha_cli app contracts deploy` y confirma el manifiesto usando los pasos de [Inicio de Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## Guías de SDK relacionadas

- [Quickstart del SDK de Rust](/sdks/rust)
- [Quickstart del SDK de Python](/sdks/python)
- [Quickstart del SDK de JavaScript](/sdks/javascript)

[Descarga la fuente de Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```
