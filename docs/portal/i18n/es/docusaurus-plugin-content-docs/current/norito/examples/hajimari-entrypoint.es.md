---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/examples/hajimari-entrypoint
título: Esqueleto del punto de entrada Hajimari
descripción: Andamiaje mínimo de contrato Kotodama con un único punto de entrada público y un manejador de estado.
fuente: crates/ivm/docs/examples/01_hajimari.ko
---

Andamiaje mínimo de contrato Kotodama con un único punto de entrada público y un manejador de estado.

## Recorrido del libro mayor

- Compila el contrato con `koto_compile --abi 1` como se muestra en [Inicio de Norito](/norito/getting-started#1-compile-a-kotodama-contract) o mediante `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Haga una prueba rápida del código de bytes localmente con `ivm_run` / `developer_portal_norito_snippets_run` para verificar el registro `info!` y el syscall inicial antes de tocar un nodo.
- Despliega el artefacto con `iroha_cli app contracts deploy` y confirma el manifiesto usando los pasos de [Inicio de Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## Guías de SDK relacionadas

- [Inicio rápido del SDK de Rust](/sdks/rust)
- [Inicio rápido del SDK de Python](/sdks/python)
- [Inicio rápido del SDK de JavaScript](/sdks/javascript)

[Descarga la fuente de Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```