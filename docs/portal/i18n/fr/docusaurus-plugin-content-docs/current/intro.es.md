---
lang: fr
direction: ltr
source: docs/portal/docs/intro.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Bienvenido al portal de desarrolladores de SORA Nexus

Le portail des développeurs de SORA Nexus contient une documentation interactive, des tutoriels de SDK et des références d'API pour les opérateurs de Nexus et les contributeurs de Hyperledger Iroha. Complète le site principal de documents pour montrer les guides pratiques et les spécifications générées directement à partir de ce référentiel. La page d'accueil comprend désormais les points d'entrée thématiques de Norito/SoraFS, les instantanés OpenAPI établis et une référence dédiée au streaming Norito pour que les contributeurs découvrent le contrat du plan de contrôle de streaming sans tener que buscar en la especación raiz.

## Vous pouvez faire ici- **Aprender Norito** - commence avec la description générale et le guide de démarrage rapide pour comprendre le modèle de sérialisation et les outils de bytecode.
- **Poser en marche les SDK** - suivez les démarrages rapides de JavaScript et Rust aujourd'hui ; Les guides Python, Swift et Android sont toujours conformes aux recettes.
- **Explorer les références de l'API** - la page OpenAPI de Torii rend les spécifications REST plus récentes, et les tableaux de configuration affichés aux sources canoniques en Markdown.
- **Préparer les documents** - les runbooks opérationnels (télémétrie, règlement, superpositions Nexus) sont migrés à partir de `docs/source/` et sont placés sur ce site conforme avant la migration.

## État actuel- Atterrissage de Docusaurus v3 avec thème, type de photographie rénové, héros/tarjets avec dégradés et tuiles de recursos qui incluent le résumé du streaming Norito.
- Plugin OpenAPI de Torii connecté à `npm run sync-openapi`, avec vérification des instantanés fermes et gardes CSP appliqués par `buildSecurityHeaders`.
- La couverture de prévisualisation et de sonde corre en CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), et bloque désormais le document de streaming, les démarrages rapides de SoraFS et les listes de contrôle de référence avant de publier des artefacts.
- Les démarrages rapides de Norito, SoraFS et SDK, accompagnés de sections de référence, sont sur la barre latérale ; De nouvelles importations depuis `docs/source/` (streaming, orchestration, runbooks) sont ici conformes et rédigées.

## Côme participant

- Consultez `docs/portal/README.md` pour les commandes de développement local (`npm install`, `npm run start`, `npm run build`).
- Les zones de migration du contenu sont associées aux éléments `DOCS-*` de la feuille de route. Ajoutez des contributions : portez les sections depuis `docs/source/` et ajoutez la page à la barre latérale.
- Si vous ajoutez un artefact généré (spécifications, tableaux de configuration), documentez la commande de construction pour que les futurs contributeurs puissent le rafraîchir facilement.