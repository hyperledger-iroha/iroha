---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29c6cd50c571800cdd9e9170db685880652681514b26bbd357dfa95561bd0580
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ur
direction: rtl
source: docs/portal/docs/reference/README.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: ریفرنس انڈیکس
slug: /reference
---

یہ سیکشن Iroha کے لئے "spec کے طور پر پڑھیں" مواد اکٹھا کرتا ہے۔ یہ صفحات مستحکم رہتے ہیں چاہے گائیڈز اور ٹیوٹوریلز ترقی کریں۔

## آج دستیاب

- **Norito codec overview** - `reference/norito-codec.md` براہ راست مستند `norito.md` specification سے لنک کرتا ہے جب تک پورٹل کی table تیار ہو رہی ہے۔
- **Torii OpenAPI** - `/reference/torii-openapi` Redoc کے ذریعے Torii کی تازہ ترین REST specification render کرتا ہے۔ spec کو `npm run sync-openapi -- --version=current --latest` سے دوبارہ بنائیں (اضافی تاریخی versions میں snapshot کاپی کرنے کے لئے `--mirror=<label>` شامل کریں)۔
- **Configuration tables** - مکمل parameter catalog `docs/source/references/configuration.md` میں رکھا جاتا ہے۔ جب تک پورٹل auto-import نہ دے، درست defaults اور environment overrides کے لئے اسی Markdown فائل کو دیکھیں۔
- **Docs versioning** - navbar کا version dropdown `npm run docs:version -- <label>` سے بنے ہوئے frozen snapshots دکھاتا ہے، جس سے releases کے درمیان guidance کا موازنہ آسان ہوتا ہے۔
