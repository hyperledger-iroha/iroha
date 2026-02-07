---
lang: fr
direction: ltr
source: docs/portal/docs/intro.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Bienvenue sur le portail développeur SORA Nexus

Le portail développeur SORA Nexus regroupe de la documentation interactive, des tutoriels SDK et des références d'API pour les opérateurs Nexus et les contributeurs Hyperledger Iroha. Il complète le site principal de docs en mettant en avant des guides pratiques et des spécifications générales directement depuis ce dépôt. La page d'accueil propose désormais des points d'entrée thématiques Norito/SoraFS, des instantanés OpenAPI signes et une référence dédiée à Norito Streaming afin que les contributeurs puissent trouver le contrat du plan de contrôle du streaming sans chercher la spécification racine.

## Ce que vous pouvez faire ici- **Apprendre Norito** - commencez par l'apercu et le quickstart pour comprendre le modèle de sérialisation et les outils de bytecode.
- **Démarrer les SDKs** - suivez les quickstarts JavaScript et Rust des aujourd'hui ; les guides Python, Swift et Android les rejoignent au fur et à mesure de la migration des recettes.
- **Parcourir les références API** - la page OpenAPI de Torii rend la dernière spécification REST, et les tableaux de configuration renvoient aux sources Markdown canoniques.
- **Préparer les déploiements** - les runbooks opérationnels (télémétrie, règlement, superpositions Nexus) sont en cours de portage depuis `docs/source/` et arriveront ici à mesure que la migration avance.

## Statut actuel- Landing Docusaurus v3 thématique avec typographie rafraichie, hero/cards guides par des dégradés et tuiles de ressources incluant le CV Norito Streaming.
- Plugin OpenAPI Torii câble sur `npm run sync-openapi`, avec vérifications de snapshots signes et protections CSP appliquées par `buildSecurityHeaders`.
- La couverture aperçu et sonde s'exécute en CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), et bloque désormais la doc streaming, les quickstarts SoraFS et les checklists de référence avant publication des artefacts.
- Les quickstarts Norito, SoraFS et SDK ainsi que les sections de référence sont dans la barre latérale ; les nouvelles importations depuis `docs/source/` (streaming, orchestration, runbooks) arrivent ici au fil de leur rédaction.

## Participant

- Voir `docs/portal/README.md` pour les commandes de développement local (`npm install`, `npm run start`, `npm run build`).
- Les tâches de migration de contenu sont suivies avec les éléments de roadmap `DOCS-*`. Les contributions sont bienvenues : portez des sections depuis `docs/source/` et ajoutez la page à la barre latérale.
- Si vous ajoutez un artefact générique (specs, tableaux de configuration), documentez la commande de build pour que les futurs contributeurs puissent le régénérer facilement.