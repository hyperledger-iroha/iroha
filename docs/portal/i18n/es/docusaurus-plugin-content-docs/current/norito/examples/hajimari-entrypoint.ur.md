---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/examples/hajimari-entrypoint
título: Hajimari انٹری پوائنٹ اسکیلیٹن
descripción: ایک واحد عوامی انٹری پوائنٹ اور اسٹیٹ ہینڈل کے ساتھ کم سے کم Kotodama کنٹریکٹ ڈھانچہ۔
fuente: crates/ivm/docs/examples/01_hajimari.ko
---

ایک واحد عوامی انٹری پوائنٹ اور اسٹیٹ ہینڈل کے ساتھ کم سے کم Kotodama کنٹریکٹ ڈھانچہ۔

## لیجر واک تھرو

- کنٹریکٹ کو `koto_compile --abi 1` کے ساتھ کمپائل کریں جیسا کہ [Norito Getting Started](/norito/getting-started#1-compile-a-kotodama-contract) میں دکھایا گیا ہے یا `cargo test -p ivm developer_portal_norito_snippets_compile` کے ذریعے۔
- `ivm_run` / `developer_portal_norito_snippets_run` Prueba de humo کریں تاکہ `info!` لاگ اور ابتدائی syscall کی تصدیق ہو سکے، نوڈ کو چھونے سے پہلے۔
- `iroha_cli app contracts deploy` کے ذریعے آرٹیفیکٹ ڈیپلائے کریں اور [Norito Primeros pasos](/norito/getting-started#4-deploy-via-iroha_cli) کے مراحل سے مینی فیسٹ کی تصدیق کریں۔

## متعلقہ SDK گائیڈز

- [Inicio rápido del SDK de Rust](/sdks/rust)
- [Inicio rápido del SDK de Python](/sdks/python)
- [Inicio rápido del SDK de JavaScript](/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```