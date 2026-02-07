---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/hajimari-entrypoint
título: Esqueleto do ponto de entrada Hajimari
description: Estrutura mínima de contrato Kotodama com um único ponto de entrada público e um identificador de estado.
fonte: crates/ivm/docs/examples/01_hajimari.ko
---

Estrutura mínima de contrato Kotodama com um único ponto de entrada público e um identificador de estado.

## Roteiro do livro razão

- Compile o contrato com `koto_compile --abi 1` conforme mostrado em [Norito Getting Started](/norito/getting-started#1-compile-a-kotodama-contract) ou via `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Faça um smoke-test do bytecode localmente com `ivm_run` / `developer_portal_norito_snippets_run` para verificar o log `info!` e um syscall inicial antes de tocar em um nodo.
- Implante os artistas via `iroha_cli app contracts deploy` e confirme o manifesto usando os passos em [Norito Getting Started](/norito/getting-started#4-deploy-via-iroha_cli).

## Guias de SDK relacionados

- [Início rápido do SDK Rust](/sdks/rust)
- [Início rápido do SDK Python](/sdks/python)
- [Início rápido do SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```