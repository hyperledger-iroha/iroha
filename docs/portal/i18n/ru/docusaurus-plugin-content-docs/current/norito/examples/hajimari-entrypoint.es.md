---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слизень: /norito/examples/hajimari-entrypoint
Название: Esqueleto del inputpoint Hajimari
описание: Минимальный договор Kotodama с единственной публичной точкой входа и управляющим государством.
источник: crates/ivm/docs/examples/01_hajimari.ko
---

Минимальный договор Kotodama с единственной публичной точкой входа и управляющим государством.

## Запись мэра библиотеки

- Составьте договор с `koto_compile --abi 1` как нужно в [Inicio de Norito](/norito/getting-started#1-compile-a-kotodama-contract) или в середине `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Была проведена быстрая проверка локального байт-кода с `ivm_run` / `developer_portal_norito_snippets_run` для проверки журнала `info!` и начального системного вызова до открытия узла.
- Отобразите артефакт с `iroha_cli app contracts deploy` и подтвердите, что манифест используется в [Inicio de Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## Руководство по настройке SDK

- [Краткий запуск SDK de Rust](/sdks/rust)
- [Краткий запуск SDK Python](/sdks/python)
- [Краткий запуск SDK JavaScript](/sdks/javascript)

[Удалить ссылку Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```