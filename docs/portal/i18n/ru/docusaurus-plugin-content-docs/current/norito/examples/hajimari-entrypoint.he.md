---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/norito/examples/hajimari-entrypoint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 88e945f9f0517d4d17386b24ccab966b92cb3e5b789d3ceb3b5a5fadf79d343e
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/hajimari-entrypoint
title: Каркас входной точки Hajimari
description: Минимальный каркас контракта Kotodama с одной публичной точкой входа и хендлом состояния.
source: crates/ivm/docs/examples/01_hajimari.ko
---

Минимальный каркас контракта Kotodama с одной публичной точкой входа и хендлом состояния.

## Пошаговый обход реестра

- Скомпилируйте контракт с `koto_compile --abi 1` как показано в [Norito Getting Started](/norito/getting-started#1-compile-a-kotodama-contract) или через `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Прогоните smoke-test байткода локально с `ivm_run` / `developer_portal_norito_snippets_run`, чтобы проверить лог `info!` и начальный syscall перед тем, как трогать узел.
- Разверните артефакт через `iroha_cli app contracts deploy` и подтвердите манифест, используя шаги из [Norito Getting Started](/norito/getting-started#4-deploy-via-iroha_cli).

## Связанные руководства SDK

- [Quickstart Rust SDK](/sdks/rust)
- [Quickstart Python SDK](/sdks/python)
- [Quickstart JavaScript SDK](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```
