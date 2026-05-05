---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/hajimari-entrypoint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 93b3329a32d69ca67bd267256a57c40d6619d07e1e39b2ec912fba3621cec123
source_last_modified: "2026-04-08T09:19:38.793123+00:00"
translation_last_reviewed: 2026-04-08
---

---
slug: /norito/examples/hajimari-entrypoint
title: هيكل نقطة دخول Hajimari
description: هيكل عقد Kotodama بسيط بنقطة دخول عامة واحدة ومقبض حالة.
source: crates/ivm/docs/examples/01_hajimari.ko
---

هيكل عقد Kotodama بسيط بنقطة دخول عامة واحدة ومقبض حالة.

## جولة دفتر الأستاذ

- قم بتجميع العقد باستخدام `koto_compile --abi 1` كما هو موضح في [البدء مع Norito](/norito/getting-started#1-compile-a-kotodama-contract) أو عبر `cargo test -p ivm developer_portal_norito_snippets_compile`.
- أجر اختبار دخان للبايت كود محليا باستخدام `ivm_run` / `developer_portal_norito_snippets_run` للتحقق من سجل `info!` والنداء النظامي الأول قبل لمس عقدة.
- انشر الأثر عبر `iroha app contracts deploy` وأكد البيان باستخدام الخطوات في [البدء مع Norito](/norito/getting-started#4-deploy-via-iroha).

## أدلة SDK ذات صلة

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```
