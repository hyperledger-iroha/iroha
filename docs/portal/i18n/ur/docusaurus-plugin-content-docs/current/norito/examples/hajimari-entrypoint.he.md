---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/norito/examples/hajimari-entrypoint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd083bdf6ff74fc3dc52d6d19fd184c8283f9fe736950558b596f758bd1cdec4
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/hajimari-entrypoint.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/hajimari-entrypoint
title: Hajimari انٹری پوائنٹ اسکیلیٹن
description: ایک واحد عوامی انٹری پوائنٹ اور اسٹیٹ ہینڈل کے ساتھ کم سے کم Kotodama کنٹریکٹ ڈھانچہ۔
source: crates/ivm/docs/examples/01_hajimari.ko
---

ایک واحد عوامی انٹری پوائنٹ اور اسٹیٹ ہینڈل کے ساتھ کم سے کم Kotodama کنٹریکٹ ڈھانچہ۔

## لیجر واک تھرو

- کنٹریکٹ کو `koto_compile --abi 1` کے ساتھ کمپائل کریں جیسا کہ [Norito Getting Started](/norito/getting-started#1-compile-a-kotodama-contract) میں دکھایا گیا ہے یا `cargo test -p ivm developer_portal_norito_snippets_compile` کے ذریعے۔
- `ivm_run` / `developer_portal_norito_snippets_run` کے ساتھ لوکل طور پر بائٹ کوڈ کا smoke-test کریں تاکہ `info!` لاگ اور ابتدائی syscall کی تصدیق ہو سکے، نوڈ کو چھونے سے پہلے۔
- `iroha_cli app contracts deploy` کے ذریعے آرٹیفیکٹ ڈیپلائے کریں اور [Norito Getting Started](/norito/getting-started#4-deploy-via-iroha_cli) کے مراحل سے مینی فیسٹ کی تصدیق کریں۔

## متعلقہ SDK گائیڈز

- [Rust SDK quickstart](/sdks/rust)
- [Python SDK quickstart](/sdks/python)
- [JavaScript SDK quickstart](/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```
