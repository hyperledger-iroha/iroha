---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/hajimari-entrypoint.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/hajimari-entrypoint
العنوان: هاجيماري إنتري پوينٹ سكيلیٹن
الوصف: هناك واحدة من نقاط الجذب الرئيسية والموقع على الإنترنت مثل Kotodama.
المصدر:crates/ivm/docs/examples/01_hajimari.ko
---

هناك أيضًا نقطة واحدة ونقطة عبر الإنترنت متصلة بشبكة Kotodama.

## ليجر واک تھرو

- الإنترنت الذي `koto_compile --abi 1` ينضم إلى مركز التجارة العالمي [Norito البدء](/norito/getting-started#1-compile-a-kotodama-contract) هذا هو السبب في ذلك `cargo test -p ivm developer_portal_norito_snippets_compile` ذریعے.
- `ivm_run` / `developer_portal_norito_snippets_run` الذي يعمل على إيقاف تشغيل اختبار الدخان لفترة `info!` والتشغيل المستمر للتشغيل، جديد تمامًا چھونے سے پہلے.
- `iroha_cli app contracts deploy` التي تم إطلاقها في البداية و[Norito البدء](/norito/getting-started#4-deploy-via-iroha_cli) هي مراحل التصنيع الخاصة بي.

## مواضيع ذات صلة SDK

- [البدء السريع لـ Rust SDK](/sdks/rust)
- [البدء السريع لـ Python SDK](/sdks/python)
- [البدء السريع لـ JavaScript SDK](/sdks/javascript)

[Kotodama تنزيل التنزيل](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```