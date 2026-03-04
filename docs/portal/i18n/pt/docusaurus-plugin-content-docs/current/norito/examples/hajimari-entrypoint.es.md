---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/hajimari-entrypoint
título: Esqueleto do ponto de entrada Hajimari
description: Defina o mínimo de contrato Kotodama com um único ponto de entrada público e um gerenciador de estado.
fonte: crates/ivm/docs/examples/01_hajimari.ko
---

Defina o mínimo de contrato Kotodama com um único ponto de entrada público e um gerenciador de estado.

## Recorrido do livro prefeito

- Compilar o contrato com `koto_compile --abi 1` como mostrado em [Início de Norito](/norito/getting-started#1-compile-a-kotodama-contract) ou por meio de `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Faça uma tentativa rápida de bytecode localmente com `ivm_run` / `developer_portal_norito_snippets_run` para verificar o log `info!` e o syscall inicial antes de tocar um nodo.
- Despligue o artefato com `iroha_cli app contracts deploy` e confirme a manifestação usando os passos de [Inicio de Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## Guias do SDK relacionados

- [Início rápido do SDK de Rust](/sdks/rust)
- [Início rápido do SDK de Python](/sdks/python)
- [Início rápido do SDK de JavaScript](/sdks/javascript)

[Descarregue a fonte de Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```