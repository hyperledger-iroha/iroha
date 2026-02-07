---
lang: fr
direction: ltr
source: docs/portal/docs/reference/README.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : ریفرنس انڈیکس
limace : /référence
---

Le modèle Iroha correspond à "spec کے طور پر پڑھیں" مواد اکٹھا کرتا ہے۔ یہ صفحات مستحکم رہتے ہیں چاہے گائیڈز اور ٹیوٹوریلز ترقی کریں۔

## آج دستیاب

- **Présentation du codec Norito** - `reference/norito-codec.md` pour la spécification `norito.md` et le tableau des spécifications ہو رہی ہے۔
- **Torii OpenAPI** - `/reference/torii-openapi` Redoc ici Torii pour le rendu de la spécification REST. spec کو `npm run sync-openapi -- --version=current --latest` سے دوبارہ بنائیں (اضافی تاریخی versions میں snapshot کاپی کرنے کے لئے `--mirror=<label>` شامل کریں)۔
- **Tableaux de configuration** - Catalogue de paramètres `docs/source/references/configuration.md` Il s'agit de l'importation automatique, des valeurs par défaut et des remplacements de l'environnement, ainsi que du Markdown.
- **Gestion des versions de Docs** - barre de navigation dans la liste déroulante des versions `npm run docs:version -- <label>` et des instantanés gelés pour les versions et les conseils d'utilisation des versions ultérieures. ہوتا ہے۔