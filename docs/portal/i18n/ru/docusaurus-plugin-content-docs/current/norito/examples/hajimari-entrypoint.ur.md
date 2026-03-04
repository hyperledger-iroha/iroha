---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
слизень: /norito/examples/hajimari-entrypoint
титул: Хаджимари انٹری پوائنٹ اسکیلیٹن
описание: ایک واحد عوامی انٹری پوائنٹ اور اسٹیٹ ہینڈل کے ساتھ کم سے کم Kotodama کنٹریکٹ ڈھانچہ۔
источник: crates/ivm/docs/examples/01_hajimari.ko
---

Если вы хотите, чтобы это произошло, вы можете использовать Kotodama کنٹریکٹ ڈھانچہ۔

## لیجر واک تھرو

- Выберите `koto_compile --abi 1`, чтобы получить информацию о [Norito Начало работы](/norito/getting-started#1-compile-a-kotodama-contract) میں دکھایا گیا ہے یا `cargo test -p ivm developer_portal_norito_snippets_compile` کے ذریعے۔
- `ivm_run` / `developer_portal_norito_snippets_run` может быть использован для проверки дыма, например, для проверки на дым `info!`. Чтобы выполнить системный вызов, выберите нужный вариант.
- `iroha_cli app contracts deploy` کے ذریعے آرٹیفیکٹ ڈیپلائے کریں اور [Norito Начало работы](/norito/getting-started#4-deploy-via-iroha_cli) کے مراحل سے مینی فیسٹ کی تصدیق کریں۔

## Использование SDK

- [Краткий старт Rust SDK](/sdks/rust)
- [Краткий старт Python SDK](/sdks/python)
- [Краткое руководство по JavaScript SDK] (/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```