---
lang: es
direction: ltr
source: docs/portal/docs/reference/README.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: ریفرنس انڈیکس
babosa: /referencia
---

یہ سیکشن Iroha کے لئے "spec کے طور پر پڑھیں" مواد اکٹھا کرتا ہے۔ یہ صفحات مستحکم رہتے ہیں چاہے گائیڈز اور ٹیوٹوریلز ترقی کریں۔

## آج دستیاب

- **Descripción general del códec Norito** - `reference/norito-codec.md` براہ راست مستند `norito.md` especificación سے لنک کرتا ہے جب تک پورٹل کی table تیار ہو رہی ہے۔
- **Torii OpenAPI** - `/reference/torii-openapi` Redoc کے ذریعے Torii کی تازہ ترین Especificación REST render کرتا ہے۔ especificación `npm run sync-openapi -- --version=current --latest` سے دوبارہ بنائیں (اضافی تاریخی versiones میں instantánea کاپی کرنے کے لئے `--mirror=<label>` شامل کریں)۔
- **Tablas de configuración** - Catálogo de parámetros del módulo `docs/source/references/configuration.md` میں رکھا جاتا ہے۔ Incluye importación automática, valores predeterminados y anulaciones de entorno, Markdown y otras opciones
- **Control de versiones de documentos** - menú desplegable de versiones de la barra de navegación `npm run docs:version -- <label>` سے بنے ہوئے instantáneas congeladas دکھاتا ہے، جس سے versiones کے درمیان guía کا موازنہ آسان ہوتا ہے۔