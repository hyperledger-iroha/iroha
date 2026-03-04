---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/hajimari-entrypoint.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/حاجیمری-انٹرپوائنٹ
عنوان: حاجیماری انٹری پوائنٹ کنکال
تفصیل: ایک ہی عوامی انٹری پوائنٹ اور اسٹیٹ ہینڈلر کے ساتھ کم سے کم معاہدہ سہاروں Kotodama۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/01_hajimari.ko
---

کم سے کم معاہدہ سہاروں Kotodama ایک ہی عوامی انٹری پوائنٹ اور اسٹیٹ ہینڈلر کے ساتھ۔

## لیجر ٹور

- `koto_compile --abi 1` کے ساتھ معاہدہ مرتب کریں جیسا کہ [Norito کے آغاز] (/norito/getting-started#1-compile-a-kotodama-contract) یا `cargo test -p ivm developer_portal_norito_snippets_compile` کا استعمال کرتے ہوئے دکھایا گیا ہے۔
- نوڈ کو چھونے سے پہلے `info!` لاگ اور ابتدائی سیسکل کو چیک کرنے کے لئے `ivm_run` / `developer_portal_norito_snippets_run` کے ساتھ مقامی طور پر ایک فوری بائیک کوڈ ٹیسٹ کریں۔
- `iroha_cli app contracts deploy` کے ساتھ آرٹیکٹیکٹ کو تعینات کریں اور [Norito کو شروع کرتے ہوئے] (/norito/getting-started#4-deploy-via-iroha_cli) کے اقدامات کا استعمال کرتے ہوئے ظاہر کریں۔

## متعلقہ SDK گائیڈز

- [مورچا SDK کوئیک اسٹارٹ] (/sdks/rust)
- [ازگر ایس ڈی کے کوئیک اسٹارٹ] (/sdks/python)
- [جاوا اسکرپٹ SDK کوئیک اسٹارٹ] (/sdks/javascript)

[Kotodama کا ماخذ ڈاؤن لوڈ کریں] (/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```