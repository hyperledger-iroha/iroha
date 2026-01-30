---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/hajimari-entrypoint.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
slug: /norito/examples/hajimari-entrypoint
title: Esqueleto del entrypoint Hajimari
description: Andamiaje mínimo de contrato Kotodama con un único entrypoint público y un manejador de estado.
source: crates/ivm/docs/examples/01_hajimari.ko
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
