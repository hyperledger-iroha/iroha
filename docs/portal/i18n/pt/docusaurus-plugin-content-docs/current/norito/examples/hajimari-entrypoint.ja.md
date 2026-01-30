---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/norito/examples/hajimari-entrypoint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b8b803529f877a5acc92b74a3355eee1c4f76a0abfe20bff3d15a382a2054696
source_last_modified: "2025-11-14T04:43:20.712946+00:00"
translation_last_reviewed: 2026-01-30
---

Estrutura minima de contrato Kotodama com um unico entrypoint publico e um handle de estado.

## Roteiro do livro razao

- Compile o contrato com `koto_compile --abi 1` conforme mostrado em [Norito Getting Started](/norito/getting-started#1-compile-a-kotodama-contract) ou via `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Faca um smoke-test do bytecode localmente com `ivm_run` / `developer_portal_norito_snippets_run` para verificar o log `info!` e a syscall inicial antes de tocar em um nodo.
- Implante o artefato via `iroha_cli app contracts deploy` e confirme o manifesto usando os passos em [Norito Getting Started](/norito/getting-started#4-deploy-via-iroha_cli).

## Guias de SDK relacionados

- [Quickstart do SDK Rust](/sdks/rust)
- [Quickstart do SDK Python](/sdks/python)
- [Quickstart do SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```
