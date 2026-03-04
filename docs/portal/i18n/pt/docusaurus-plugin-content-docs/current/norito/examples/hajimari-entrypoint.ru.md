---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/hajimari-entrypoint
título: Каркас входной точки Hajimari
descrição: Минимальный каркас контракта Kotodama с одной публичной точкой входа и хендлом состояния.
fonte: crates/ivm/docs/examples/01_hajimari.ko
---

O contrato mínimo de carcaça Kotodama está disponível para venda e envio.

## Пошаговый обход реестра

- Скомпилируйте контракт с `koto_compile --abi 1` как показано в [Norito Getting Started](/norito/getting-started#1-compile-a-kotodama-contract) ou через `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Прогоните smoke-test байткода локально с `ivm_run` / `developer_portal_norito_snippets_run`, чтобы проверить лог `info!` e начальный syscall é o tempo que está sendo executado.
- Разверните артефакт через `iroha_cli app contracts deploy` e подтвердите манифест, используя шаги из [Norito Getting Iniciado](/norito/getting-started#4-deploy-via-iroha_cli).

## Como usar o SDK

- [Início rápido Rust SDK](/sdks/rust)
- [Início rápido do SDK do Python](/sdks/python)
- [SDK JavaScript de início rápido](/sdks/javascript)

[Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```