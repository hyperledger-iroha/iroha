---
lang: ur
direction: rtl
source: docs/portal/docs/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b3b2becfdbab1446f8f230ace905de306e1e89147f5a5e578d784be97445d74d
source_last_modified: "2025-11-08T06:08:33.073497+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: ریفرنس انڈیکس
slug: /reference
---

یہ سیکشن Iroha کے لئے "spec کے طور پر پڑھیں" مواد اکٹھا کرتا ہے۔ یہ صفحات مستحکم رہتے ہیں چاہے گائیڈز اور ٹیوٹوریلز ترقی کریں۔

## آج دستیاب

- **Norito codec overview** - `reference/norito-codec.md` براہ راست مستند `norito.md` specification سے لنک کرتا ہے جب تک پورٹل کی table تیار ہو رہی ہے۔
- **Torii OpenAPI** - `/reference/torii-openapi` Redoc کے ذریعے Torii کی تازہ ترین REST specification render کرتا ہے۔ spec کو `npm run sync-openapi -- --version=current --latest` سے دوبارہ بنائیں (اضافی تاریخی versions میں snapshot کاپی کرنے کے لئے `--mirror=<label>` شامل کریں)۔
- **Configuration tables** - مکمل parameter catalog `docs/source/references/configuration.md` میں رکھا جاتا ہے۔ جب تک پورٹل auto-import نہ دے، درست defaults اور environment overrides کے لئے اسی Markdown فائل کو دیکھیں۔
- **Docs versioning** - navbar کا version dropdown `npm run docs:version -- <label>` سے بنے ہوئے frozen snapshots دکھاتا ہے، جس سے releases کے درمیان guidance کا موازنہ آسان ہوتا ہے۔
