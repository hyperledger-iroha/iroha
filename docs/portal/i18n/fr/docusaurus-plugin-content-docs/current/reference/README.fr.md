---
lang: fr
direction: ltr
source: docs/portal/docs/reference/README.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Index de référence
limace : /référence
---

Cette section regroupe le matériel à lire comme une spécification pour Iroha. Ces pages restent stables même lorsque les guides et tutoriels évoluent.

## Disponible aujourd'hui

- **Apercu du codec Norito** - `reference/norito-codec.md` renvoie directement à la spécification faisant autorité `norito.md` pendant que la table du portail est en cours de remplissage.
- **Torii OpenAPI** - `/reference/torii-openapi` rend la dernière spécification REST de Torii avec Redoc. Régénérez la spécification via `npm run sync-openapi -- --version=current --latest` (ajoutez `--mirror=<label>` pour copier le snapshot dans des versions historiques supplémentaires).
- **Tables de configuration** - Le catalogue complet des paramètres se trouve dans `docs/source/references/configuration.md`. Tant que le portail ne propose pas d'importation automatique, référez-vous à ce fichier Markdown pour les valeurs par défaut exactes et les surcharges d'environnement.
- **Versionnement des docs** - Le menu de version dans la barre de navigation expose des instantanés figés avec `npm run docs:version -- <label>`, ce qui facilite la comparaison des recommandations entre releases.