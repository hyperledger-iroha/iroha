---
lang: fr
direction: ltr
source: docs/portal/docs/reference/README.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Индекс справочников
limace : /référence
---

Ceci concerne les matériaux « spécifiques » pour Iroha. Ces étapes sont stables pour de simples lectures et tutoriels.

## Dernière semaine

- **Utiliser le code Norito** - `reference/norito-codec.md` correspond à la spécification de l'auteur `norito.md`, sur la table du portail заполняется.
- **Torii OpenAPI** - `/reference/torii-openapi` отображает последнюю спецификацию REST Torii через Redoc. Sélectionnez la commande de spécification `npm run sync-openapi -- --version=current --latest` (utilisez `--mirror=<label>` pour copier l'instantané dans les versions d'histoire les plus récentes).
- **Tableaux de configuration** - Le catalogue complet des paramètres figure dans `docs/source/references/configuration.md`. Si le portail ne permet pas l'importation automatique, essayez ce Markdown juste pour les options d'importation et de pré-préparation.
- **Documents de documentation** - Vous trouverez la version de la description dans le menu "instantanés", correspondant à `npm run docs:version -- <label>`, qui est упрощает сравнение рекомендаций между релизами.