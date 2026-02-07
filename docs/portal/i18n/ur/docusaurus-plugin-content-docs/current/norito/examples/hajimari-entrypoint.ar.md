---
lang: ur
direction: rtl
source: docs/portal/docs/norito/examples/hajimari-entrypoint.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
سلگ:/نوریٹو/مثالوں/حاجیمری-انٹرپوائنٹ
عنوان: حاجیماری انٹری پوائنٹ ڈھانچہ
تفصیل: Kotodama معاہدہ کا ڈھانچہ ایک ہی عوامی انٹری پوائنٹ اور اسٹیٹس ہینڈل کے ساتھ آسان ہے۔
ماخذ: کریٹس/IVM/دستاویزات/مثالوں/01_hajimari.ko
---

Kotodama معاہدہ کا ڈھانچہ ایک واحد عالمی انٹری پوائنٹ اور اسٹیٹس ہینڈل کے ساتھ آسان ہے۔

## لیجر ٹور

- `koto_compile --abi 1` کا استعمال کرتے ہوئے نوڈس کو جمع کریں جیسا کہ [Norito کے ساتھ شروعات کرنا] (/norito/getting-started#1-compile-a-kotodama-contract) یا `cargo test -p ivm developer_portal_norito_snippets_compile` کے ذریعے بیان کیا گیا ہے۔
- نوڈ کو چھونے سے پہلے /norito/getting-started#1-compile-a-kotodama-contract رجسٹر اور پہلا سسٹم کال چیک کرنے کے لئے `ivm_run` / `developer_portal_norito_snippets_run` کا استعمال کرتے ہوئے مقامی طور پر ایک بائیکوڈ دھواں ٹیسٹ انجام دیں۔
- `iroha_cli app contracts deploy` کے توسط سے ٹریس کو شائع کریں اور [Norito کے ساتھ شروعات کرنا] (/norito/getting-started#4-deploy-via-iroha_cli) کے اقدامات کا استعمال کرتے ہوئے بیان کی تصدیق کریں۔

## متعلقہ SDK گائیڈز

- [مورچا SDK کوئیک اسٹارٹ] (/sdks/rust)
- [ازگر SDK کوئیک اسٹارٹ] (/sdks/python)
- [جاوا اسکرپٹ SDK کوئیک اسٹارٹ] (/sdks/javascript)

[ماخذ Kotodama ڈاؤن لوڈ کریں] (/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```