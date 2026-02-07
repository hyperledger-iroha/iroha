---
lang: fr
direction: ltr
source: docs/portal/docs/intro.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Je suis venu vers le portail des utilisateurs SORA Nexus

Le portail des développeurs SORA Nexus regroupe une documentation interactive, des tutoriels de SDK et des références d'API pour les opérateurs Nexus et les contributeurs Hyperledger Iroha. Il complète le site principal de documentation pour l'exportation de guides pratiques et de spécifications gérés directement à partir du référentiel. Une page d'accueil vient de passer par les points d'entrée thématiques de Norito/SoraFS, les instantanés OpenAPI assassinés et une référence dédiée au streaming Norito pour contribuer à rencontrer le contrat de contrôle du plan de streaming en fonction des spécifications. Raiz.

## O que votre voix peut faire ici

- **Aprender Norito** - voici la présentation et le démarrage rapide pour comprendre le modèle de sérialisation et les outils de bytecode.
- **Inicializar SDKs** - suivez les démarrages rapides de JavaScript et Rust aujourd'hui ; Les guides Python, Swift et Android seront des adicionados conformes aux recettes des migrations.
- **Explorer les références de l'API** - la page OpenAPI de Torii rend les spécifications REST les plus récentes, ainsi que les tableaux de configuration disponibles pour les polices canoniques dans Markdown.
- **Préparer les déploiements** - les runbooks opérationnels (télémétrie, règlement, superpositions Nexus) sont envoyés aux ports `docs/source/` et téléchargés sur ce site conformément à une avancée de migration.

## Statut actuel- OK Landing Docusaurus v3 tematizada avec une typographie rénovée, des héros/cartes guidés par dégradé et des tuiles de recursos qui incluent le résumé du streaming Norito.
- OK Plugin OpenAPI fait Torii lié à `npm run sync-openapi`, avec des vérifications des instantanés assassinés et des gardes CSP appliqués par `buildSecurityHeaders`.
- OK Preview et la couverture de la sonde ne sont pas CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), depuis le portail ou le document de streaming, les démarrages rapides de SoraFS et les listes de contrôle de référence avant la publication des articles.
- OK Quickstarts de Norito, SoraFS et SDK mais les secondes de référence sont dans la barre latérale ; Les nouvelles importations de `docs/source/` (streaming, orchestration, runbooks) sont ici conformes à celles écrites.

## Côme participant

- Voir `docs/portal/README.md` pour les commandes de développement local (`npm install`, `npm run start`, `npm run build`).
- En tant que tarifs de migration du contenu sao accompagnant les éléments de la feuille de route `DOCS-*`. Contribue à cette page - porte secoes de `docs/source/` et ajout à la page de la barre latérale.
- Si vous ajoutez un artéfact créé (spécifications, tables de configuration), documentez ou commande de build pour que les futurs contributeurs puissent les actualiser facilement.