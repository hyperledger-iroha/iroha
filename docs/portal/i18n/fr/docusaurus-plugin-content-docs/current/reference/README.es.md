---
lang: fr
direction: ltr
source: docs/portal/docs/reference/README.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Indice de référence
limace : /référence
---

Cette section réunit le matériel de "leelo comme spécification" pour Iroha. Ces pages sont maintenues stables également lorsque les guides et les tutoriels évoluent.

## Disponible aujourd'hui

- **Resumen del codec Norito** - `reference/norito-codec.md` affiché directement sur la spécification autorisée `norito.md` pendant que vous complétez la table du portail.
- **Torii OpenAPI** - `/reference/torii-openapi` rend la spécification REST plus récente de Torii en utilisant Redoc. Régénérez la spécification avec `npm run sync-openapi -- --version=current --latest` (ajoutez `--mirror=<label>` pour copier l'instantané dans des versions historiques supplémentaires).
- **Tableaux de configuration** - Le catalogue complet de paramètres est conservé sur `docs/source/references/configuration.md`. Jusqu'à ce que le portail public ait une importation automatique, consultez ce fichier Markdown pour les valeurs par défaut exact et les annonces d'entrée.
- **Versionado de docs** - La version disponible dans la barre de navigation expose les instantanés gelés créés avec `npm run docs:version -- <label>`, ce qui facilite la comparaison de la guide entre les versions.