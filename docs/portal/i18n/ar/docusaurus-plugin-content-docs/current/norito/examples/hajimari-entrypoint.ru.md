---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/hajimari-entrypoint.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/hajimari-entrypoint
العنوان: karcas водной точки هاجيماري
الوصف: الحد الأدنى من عقد السيارة Kotodama مع مياه عامة واحدة وحالة جيدة.
المصدر:crates/ivm/docs/examples/01_hajimari.ko
---

الحد الأدنى من عقد الشحن Kotodama مع مياه عامة جيدة واحدة وحالة جيدة.

## Почаговый обдод еестра

- قم بتجميع العقد باستخدام `koto_compile --abi 1` كما تم عرضه في [Norito البدء](/norito/getting-started#1-compile-a-kotodama-contract) أو عبر `cargo test -p ivm developer_portal_norito_snippets_compile`.
- قم بترقية الرمز البنكي لاختبار الدخان محليًا باستخدام `ivm_run` / `developer_portal_norito_snippets_run` للتحقق من السجل `info!` وبدء استدعاء النظام قبل ذلك، كما يمكنك شراءها.
- تم التحقق من المنتج من خلال `iroha_cli app contracts deploy` وبيان التحقق، باستخدام أشياء من [Norito الشروع في العمل](/norito/getting-started#4-deploy-via-iroha_cli).

## تطوير شامل SDK

- [Quickstart Rust SDK](/sdks/rust)
- [Quickstart Python SDK](/sdks/python)
- [Quickstart JavaScript SDK](/sdks/javascript)

[تحميل المصدر Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```