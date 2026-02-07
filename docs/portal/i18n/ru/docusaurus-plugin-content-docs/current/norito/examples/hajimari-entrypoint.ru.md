---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слизень: /norito/examples/hajimari-entrypoint
Название: Каркас входной точки Хаджимари
описание: Минимальный каркасный контракт Kotodama с публичной одной точкой входа и хендлом состояния.
источник: crates/ivm/docs/examples/01_hajimari.ko
---

Минимальный каркасный контракт Kotodama одна с публичной точкой входа и хендлом состояния.

## Пошаговый обход реестра

- Скомпилируйте контракт с `koto_compile --abi 1`, как показано в [Norito Начало работы](/norito/getting-started#1-compile-a-kotodama-contract) или через `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Прогоните локальный байткод дымового теста с `ivm_run` / `developer_portal_norito_snippets_run`, чтобы проверить регистрацию `info!` и начальный системный вызов перед тем, как трогать узел.
- Разверните документ через `iroha_cli app contracts deploy` и подтвердите манифест, следуя шагам из [Norito Начало работы](/norito/getting-started#4-deploy-via-iroha_cli).

## Связанные управления SDK

- [Быстрый запуск Rust SDK](/sdks/rust)
- [Быстрый запуск Python SDK] (/sdks/python)
- [Быстрый запуск JavaScript SDK](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```