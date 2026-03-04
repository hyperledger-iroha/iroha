---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/hajimari-entrypoint.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سبيكة: /norito/examples/hajimari-entrypoint
العنوان: Esqueleto do نقطة الدخول Hajimari
الوصف: الحد الأدنى من إعدادات العقد Kotodama مع نقطة دخول عامة فريدة ومقبض حالة.
المصدر:crates/ivm/docs/examples/01_hajimari.ko
---

تم إنشاء الحد الأدنى من العقد Kotodama كنقطة دخول عامة فريدة ومقبض للحالة.

## Roteiro do livro razao

- قم بتجميع العقد مع `koto_compile --abi 1` المتوافق مع [Norito Getting Started](/norito/getting-started#1-compile-a-kotodama-contract) أو عبر `cargo test -p ivm developer_portal_norito_snippets_compile`.
- قم بإجراء اختبار دخان للرمز الثانوي المحلي باستخدام `ivm_run` / `developer_portal_norito_snippets_run` للتحقق من السجل `info!` والاتصال الأولي مسبقًا بالعقدة.
- قم بالزرع أو التصنيع عبر `iroha_cli app contracts deploy` وقم بتأكيد البيان باستخدام المرور في [Norito Getting Started](/norito/getting-started#4-deploy-via-iroha_cli).

## أدلة SDK ذات الصلة

- [بدء التشغيل السريع لـ SDK Rust](/sdks/rust)
- [البدء السريع لـ SDK Python](/sdks/python)
- [بدء التشغيل السريع لـ SDK JavaScript](/sdks/javascript)

[اضغط على الخط Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```