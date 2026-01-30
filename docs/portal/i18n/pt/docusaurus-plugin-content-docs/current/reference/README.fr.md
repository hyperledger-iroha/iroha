---
lang: pt
direction: ltr
source: docs/portal/docs/reference/README.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: Index de reference
slug: /reference
---

Cette section regroupe le materiel  a lire comme une specification  pour Iroha. Ces pages restent stables meme lorsque les guides et tutoriels evoluent.

## Disponible aujourd'hui

- **Apercu du codec Norito** - `reference/norito-codec.md` renvoie directement a la specification autoritative `norito.md` pendant que la table du portail est en cours de remplissage.
- **Torii OpenAPI** - `/reference/torii-openapi` rend la derniere specification REST de Torii avec Redoc. Regenerez la spec via `npm run sync-openapi -- --version=current --latest` (ajoutez `--mirror=<label>` pour copier le snapshot dans des versions historiques supplementaires).
- **Tables de configuration** - Le catalogue complet des parametres se trouve dans `docs/source/references/configuration.md`. Tant que le portail ne propose pas d'auto-import, referez-vous a ce fichier Markdown pour les valeurs par defaut exactes et les surcharges d'environnement.
- **Versionnement des docs** - Le menu de version dans la barre de navigation expose des snapshots figes crees avec `npm run docs:version -- <label>`, ce qui facilite la comparaison des recommandations entre releases.
