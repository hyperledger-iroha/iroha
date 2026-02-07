---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/hajimari-entrypoint.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/حاجیمری-انٹرپوائنٹ
عنوان: حاجیماری انٹری پوائنٹ فریم
تفصیل: ایک عوامی انٹری پوائنٹ اور اسٹیٹ ہینڈل کے ساتھ کم سے کم معاہدہ کا فریم ورک Kotodama ایکس۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/01_hajimari.ko
---

ایک عوامی انٹری پوائنٹ اور اسٹیٹ ہینڈل کے ساتھ کم سے کم معاہدہ کا فریم ورک Kotodama۔

## قدم بہ قدم رجسٹری ٹریورسل

- `koto_compile --abi 1` کے ساتھ معاہدہ مرتب کریں جیسا کہ [Norito شروع کرنا] (/norito/getting-started#1-compile-a-kotodama-contract) یا `cargo test -p ivm developer_portal_norito_snippets_compile` کے ذریعے دکھایا گیا ہے۔
- نوڈ کو چھونے سے پہلے `info!` لاگ اور ابتدائی سیسکل کو چیک کرنے کے لئے `ivm_run` / `developer_portal_norito_snippets_run` کے ساتھ مقامی طور پر بائیک کوڈ کا دھواں ٹیسٹ چلائیں۔
- `iroha_cli app contracts deploy` کے توسط سے آرٹیکٹیکٹ کو تعینات کریں اور [Norito شروع کرنے] (/norito/getting-started#4-deploy-via-iroha_cli) کے اقدامات کا استعمال کرتے ہوئے ظاہر کی توثیق کریں۔

## متعلقہ SDK سبق

- [کوئیک اسٹارٹ مورچا SDK] (/sdks/rust)
- [کوئک اسٹارٹ ازگر ایس ڈی کے] (/sdks/python)
- [کوئیک اسٹارٹ جاوا اسکرپٹ SDK] (/sdks/javascript)

[ماخذ Kotodama ڈاؤن لوڈ کریں] (/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```