---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слизень: /norito/examples/hajimari-entrypoint
Название: هيكل نقطة دخول Hajimari
описание: هيكل عقد Kotodama بسيط بنقطة دخول عامة واحدة ومقبض حالة.
источник: crates/ivm/docs/examples/01_hajimari.ko
---

В качестве примера Kotodama был установлен новый модуль.

## جولة دفتر الأستاذ

- قم بتجميع العقد باستخدام `koto_compile --abi 1` كما هو موضح في [البدء مع Norito](/norito/getting-started#1-compile-a-kotodama-contract) или `cargo test -p ivm developer_portal_norito_snippets_compile`.
- أجر اختبار دخان للبايت محليا باستخدام `ivm_run` / `developer_portal_norito_snippets_run` للتحقق من سجل. `info!` находится в центре внимания.
- انشر الأثر عبر `iroha_cli app contracts deploy` وأكد البيان باستخدام الخطوات في [البدء مع] Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## Использование SDK

- [Загрузка в Rust SDK](/sdks/rust)
- [Просмотр Python SDK](/sdks/python)
- [Загрузка JavaScript SDK](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```