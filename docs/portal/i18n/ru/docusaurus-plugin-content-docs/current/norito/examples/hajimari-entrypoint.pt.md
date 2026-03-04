---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слизень: /norito/examples/hajimari-entrypoint
Название: Esqueleto do inputpoint Hajimari
описание: Минимальное соглашение Kotodama с единой публичной точкой входа и дескриптором состояния.
источник: crates/ivm/docs/examples/01_hajimari.ko
---

Минимальный контракт Kotodama с единой публичной точкой входа и дескриптором состояния.

## Ротейру до Ливро Разау

- Скомпилируйте контракт с `koto_compile --abi 1` в соответствии с [Norito Начало работы](/norito/getting-started#1-compile-a-kotodama-contract) или через `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Вы можете выполнить дымовой тест с локальным байт-кодом с помощью `ivm_run` / `developer_portal_norito_snippets_run` для проверки журнала `info!` и начального системного вызова перед тем, как сделать это в Nodo.
- Вставьте артефато через `iroha_cli app contracts deploy` и подтвердите манифест, используя проходы в [Norito Начало работы](/norito/getting-started#4-deploy-via-iroha_cli).

## Рекомендации по использованию SDK

- [Краткий старт работы с SDK Rust](/sdks/rust)
- [Краткий старт работы с SDK Python](/sdks/python)
- [Быстрый запуск SDK JavaScript](/sdks/javascript)

[Вставьте шрифт Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```