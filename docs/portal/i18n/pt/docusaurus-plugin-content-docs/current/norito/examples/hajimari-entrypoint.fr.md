---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/hajimari-entrypoint
título: Squelette du point d'entrée Hajimari
descrição: Estrutura mínima de contrato Kotodama com um único ponto de entrada público e um gerente de estado.
fonte: crates/ivm/docs/examples/01_hajimari.ko
---

Estrutura mínima de contrato Kotodama com um único ponto de entrada público e um gerente de estado.

## Parcours du registre

- Compile o contrato com `koto_compile --abi 1` conforme indicado em [Démarrage de Norito](/norito/getting-started#1-compile-a-kotodama-contract) ou via `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Efetue um teste de fumaça do bytecode local com `ivm_run` / `developer_portal_norito_snippets_run` para verificar o log `info!` e o syscall inicial antes de tocar um nó.
- Implante o artefato via `iroha_cli app contracts deploy` e confirme o manifesto seguindo as etapas de [Desarmar de Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## Guias SDK associados

- [Início rápido do SDK Rust](/sdks/rust)
- [Início rápido do SDK Python](/sdks/python)
- [JavaScript do SDK de início rápido](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```