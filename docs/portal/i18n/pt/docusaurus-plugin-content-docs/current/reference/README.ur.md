---
lang: pt
direction: ltr
source: docs/portal/docs/reference/README.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: ریفرنس انڈیکس
slug: /referência
---

یہ سیکشن Iroha کے لئے "spec کے طور پر پڑھیں" مواد اکٹھا کرتا ہے۔ یہ صفحات مستحکم رہتے ہیں چاہے گائیڈز اور ٹیوٹوریلز ترقی کریں۔

## آج دستیاب

- **Visão geral do codec Norito** - `reference/norito-codec.md` براہ راست مستند Especificação `norito.md` سے لنک کرتا ہے جب تک پورٹل کی tabela تیار ہو رہی ہے۔
- **Torii OpenAPI** - `/reference/torii-openapi` Redoc کے ذریعے Torii کی تازہ ترین Especificação REST render کرتا ہے۔ spec کو `npm run sync-openapi -- --version=current --latest` سے دوبارہ بنائیں (اضافی تاریخی versões میں snapshot کاپی کرنے کے لئے `--mirror=<label>` شامل کریں)۔
- **Tabelas de configuração** - Catálogo de parâmetros de referência `docs/source/references/configuration.md` میں رکھا جاتا ہے۔ جب تک پورٹل importação automática نہ دے, درست padrões e substituições de ambiente کے لئے اسی Markdown فائل کو دیکھیں۔
- **Versão de documentos** - barra de navegação کا menu suspenso de versão `npm run docs:version -- <label>` سے بنے ہوئے instantâneos congelados دکھاتا ہے, جس سے lançamentos کے درمیان orientação کا موازنہ آسان ہوتا ہے۔