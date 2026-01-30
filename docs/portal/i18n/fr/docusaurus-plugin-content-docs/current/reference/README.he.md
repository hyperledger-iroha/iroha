---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b715cfbd3b6a415a3f5a459a8f355cb3f9e5c2fd2003b4c867004a88ea410303
source_last_modified: "2025-11-14T04:43:20.931716+00:00"
translation_last_reviewed: 2026-01-30
---

Cette section regroupe le materiel  a lire comme une specification  pour Iroha. Ces pages restent stables meme lorsque les guides et tutoriels evoluent.

## Disponible aujourd'hui

- **Apercu du codec Norito** - `reference/norito-codec.md` renvoie directement a la specification autoritative `norito.md` pendant que la table du portail est en cours de remplissage.
- **Torii OpenAPI** - `/reference/torii-openapi` rend la derniere specification REST de Torii avec Redoc. Regenerez la spec via `npm run sync-openapi -- --version=current --latest` (ajoutez `--mirror=<label>` pour copier le snapshot dans des versions historiques supplementaires).
- **Tables de configuration** - Le catalogue complet des parametres se trouve dans `docs/source/references/configuration.md`. Tant que le portail ne propose pas d'auto-import, referez-vous a ce fichier Markdown pour les valeurs par defaut exactes et les surcharges d'environnement.
- **Versionnement des docs** - Le menu de version dans la barre de navigation expose des snapshots figes crees avec `npm run docs:version -- <label>`, ce qui facilite la comparaison des recommandations entre releases.
