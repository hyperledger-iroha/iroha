---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/examples/hajimari-entrypoint
título: Esqueleto del punto de entrada Hajimari
descripción: Estructura mínima de contrato Kotodama con un único punto de entrada público y un identificador de estado.
fuente: crates/ivm/docs/examples/01_hajimari.ko
---

Estructura mínima de contrato Kotodama con un único punto de entrada público y un identificador de estado.

## Roteiro do livro razao

- Compile el contrato con `koto_compile --abi 1` conforme se muestra en [Norito Getting Started](/norito/getting-started#1-compile-a-kotodama-contract) o vía `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Haga una prueba de humo con código de bytes localmente con `ivm_run` / `developer_portal_norito_snippets_run` para verificar el registro `info!` y una llamada al sistema inicial antes de tocar un nodo.
- Implante o artefato via `iroha_cli app contracts deploy` e confirme o manifesto usando os passos em [Norito Getting Started](/norito/getting-started#4-deploy-via-iroha_cli).

## Guías de SDK relacionadas

- [Inicio rápido de SDK Rust](/sdks/rust)
- [Inicio rápido del SDK Python](/sdks/python)
- [Inicio rápido del SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```