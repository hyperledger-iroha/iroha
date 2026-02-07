---
lang: fr
direction: ltr
source: docs/portal/docs/reference/README.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Indice de référence
limace : /référence
---

Esta secao agrega o materials "leia como especialacao" para Iroha. Ces pages sont permanentes lorsque les guides et tutoriels évoluent.

## Disponivel hoje

- **Visa geral do codec Norito** - `reference/norito-codec.md` s'adresse directement à l'autorité compétente `norito.md` pour le tableau du portail esta sendo preenchida.
- **Torii OpenAPI** - `/reference/torii-openapi` rend les spécifications REST plus récentes de Torii en utilisant Redoc. Régénérez une spécification avec `npm run sync-openapi -- --version=current --latest` (ajoutez `--mirror=<label>` pour copier un instantané pour des versions historiques supplémentaires).
- **Tableaux de configuration** - Le catalogue complet de paramètres figure dans `docs/source/references/configuration.md`. Lorsque le portail propose l'importation automatique, consultez cet archive Markdown pour les paramètres par défaut et les remplacements de l'environnement.
- **Version de la documentation** - La liste déroulante vers la barre de navigation expose les instantanés gelés créés avec `npm run docs:version -- <label>`, facilitant la comparaison des orientations entre les versions.