---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 93b3329a32d69ca67bd267256a57c40d6619d07e1e39b2ec912fba3621cec123
source_last_modified: "2026-04-08T09:19:38.793123+00:00"
translation_last_reviewed: 2026-04-08
---

---
slug: /norito/examples/hajimari-entrypoint
title: Esqueleto do entrypoint Hajimari
description: Estrutura minima de contrato Kotodama com um unico entrypoint publico e um handle de estado.
source: crates/ivm/docs/examples/01_hajimari.ko
---

Estrutura minima de contrato Kotodama com um unico entrypoint publico e um handle de estado.

## Roteiro do livro razao

- Compile o contrato com `koto_compile --abi 1` conforme mostrado em [Norito Getting Started](/norito/getting-started#1-compile-a-kotodama-contract) ou via `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Faca um smoke-test do bytecode localmente com `ivm_run` / `developer_portal_norito_snippets_run` para verificar o log `info!` e a syscall inicial antes de tocar em um nodo.
- Implante o artefato via `iroha app contracts deploy` e confirme o manifesto usando os passos em [Norito Getting Started](/norito/getting-started#4-deploy-via-iroha).

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
