---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/hajimari-entrypoint.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/حاجیمری-انٹرپوائنٹ
عنوان: ہجیماری انٹری پوائنٹ کا کنکال
تفصیل: ایک ہی عوامی انٹری پوائنٹ اور ایک ریاستی مینیجر کے ساتھ کم سے کم معاہدہ کا ڈھانچہ Kotodama۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/01_hajimari.ko
---

کم سے کم معاہدہ کا ڈھانچہ Kotodama ایک ہی عوامی انٹری پوائنٹ اور ایک ریاستی مینیجر کے ساتھ۔

## رجسٹری براؤزنگ

- `koto_compile --abi 1` کے ساتھ معاہدہ مرتب کریں جیسا کہ [Norito شروع کرنا] (/norito/getting-started#1-compile-a-kotodama-contract) میں یا `cargo test -p ivm developer_portal_norito_snippets_compile` کے ذریعے دکھایا گیا ہے۔
- نوڈ کو مارنے سے پہلے لاگ `info!` اور ابتدائی سیسکل کو چیک کرنے کے لئے `ivm_run` / `developer_portal_norito_snippets_run` کے ساتھ مقامی طور پر بائیک کوڈ کا دھواں ٹیسٹ انجام دیں۔
- `iroha_cli app contracts deploy` کے توسط سے آرٹیکٹیکٹ کو تعینات کریں اور [Norito کو شروع کرتے ہوئے] (/norito/getting-started#4-deploy-via-iroha_cli) کے اقدامات پر عمل کرکے مینی فیسٹ کی تصدیق کریں۔

## متعلقہ SDK گائیڈز

- [کوئیک اسٹارٹ ایس ڈی کے زنگ] (/sdks/rust)
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