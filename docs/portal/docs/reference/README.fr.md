<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/reference/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b3b2becfdbab1446f8f230ace905de306e1e89147f5a5e578d784be97445d74d
source_last_modified: "2025-11-08T06:08:33.073497+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: Index de référence
slug: /reference
---

Cette section regroupe le matériel « à lire comme une spécification » pour Iroha. Ces pages restent stables même lorsque les guides et tutoriels évoluent.

## Disponible aujourd'hui

- **Aperçu du codec Norito** - `reference/norito-codec.md` renvoie directement à la spécification autoritative `norito.md` pendant que la table du portail est en cours de remplissage.
- **Torii OpenAPI** - `/reference/torii-openapi` rend la dernière spécification REST de Torii avec Redoc. Régénérez la spec via `npm run sync-openapi -- --version=current --latest` (ajoutez `--mirror=<label>` pour copier le snapshot dans des versions historiques supplémentaires).
- **Tables de configuration** - Le catalogue complet des paramètres se trouve dans `docs/source/references/configuration.md`. Tant que le portail ne propose pas d'auto-import, référez-vous à ce fichier Markdown pour les valeurs par défaut exactes et les surcharges d'environnement.
- **Versionnement des docs** - Le menu de version dans la barre de navigation expose des snapshots figés créés avec `npm run docs:version -- <label>`, ce qui facilite la comparaison des recommandations entre releases.
