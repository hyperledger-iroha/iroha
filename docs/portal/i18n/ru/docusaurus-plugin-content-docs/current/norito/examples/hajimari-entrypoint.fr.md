---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слизень: /norito/examples/hajimari-entrypoint
Название: Squelette du point d'entrée Hajimari
описание: Минимальная структура контракта Kotodama с единственной точкой доступа для общественности и государственным управлением.
источник: crates/ivm/docs/examples/01_hajimari.ko
---

Минимальная структура контракта Kotodama с единственной точкой доступа для общественности и государственным управлением.

## Парк регистрации

- Скомпилируйте договор с `koto_compile --abi 1` как отдельный в [Démarrage de Norito](/norito/getting-started#1-compile-a-kotodama-contract) или через `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Выполните дымовое тестирование байт-кода на локальном компьютере с `ivm_run` / `developer_portal_norito_snippets_run` для проверки журнала `info!` и начального системного вызова перед нажатием кнопки.
- Разверните артефакт через `iroha_cli app contracts deploy` и подтвердите манифест на последующих этапах [Démarrage de Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## Руководства для партнеров SDK

- [Быстрый запуск SDK Rust](/sdks/rust)
- [Быстрый запуск SDK Python] (/sdks/python)
- [Быстрый запуск SDK JavaScript](/sdks/javascript)

[Зарядите источник Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```