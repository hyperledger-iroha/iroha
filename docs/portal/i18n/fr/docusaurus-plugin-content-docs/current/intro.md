<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/intro.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Bienvenue sur le portail développeur SORA Nexus

Le portail développeur SORA Nexus regroupe de la documentation interactive, des tutoriels SDK et des références d'API pour les opérateurs Nexus et les contributeurs Hyperledger Iroha. Il complète le site principal de docs en mettant en avant des guides pratiques et des spécifications générées directement depuis ce dépôt. La page d'accueil propose désormais des points d'entrée thématiques Norito/SoraFS, des snapshots OpenAPI signés et une référence dédiée à Norito Streaming afin que les contributeurs puissent trouver le contrat du plan de contrôle du streaming sans fouiller la spécification racine.

## Ce que vous pouvez faire ici

- **Apprendre Norito** - commencez par l'aperçu et le quickstart pour comprendre le modèle de sérialisation et les outils de bytecode.
- **Démarrer les SDKs** - suivez les quickstarts JavaScript et Rust dès aujourd'hui ; les guides Python, Swift et Android les rejoindront au fur et à mesure de la migration des recettes.
- **Parcourir les références API** - la page OpenAPI de Torii rend la dernière spécification REST, et les tableaux de configuration renvoient aux sources Markdown canoniques.
- **Préparer les déploiements** - les runbooks opérationnels (telemetry, settlement, Nexus overlays) sont en cours de portage depuis `docs/source/` et arriveront ici à mesure que la migration avance.

## Statut actuel

- ✅ Landing Docusaurus v3 thématique avec typographie rafraîchie, hero/cards guidés par des dégradés et tuiles de ressources incluant le résumé Norito Streaming.
- ✅ Plugin OpenAPI Torii câblé sur `npm run sync-openapi`, avec vérifications de snapshots signés et protections CSP appliquées par `buildSecurityHeaders`.
- ✅ La couverture preview et probe s'exécute en CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), et bloque désormais la doc streaming, les quickstarts SoraFS et les checklists de référence avant publication des artefacts.
- ✅ Les quickstarts Norito, SoraFS et SDK ainsi que les sections de référence sont dans la barre latérale ; les nouvelles importations depuis `docs/source/` (streaming, orchestration, runbooks) arrivent ici au fil de leur rédaction.

## Participer

- Voir `docs/portal/README.md` pour les commandes de développement local (`npm install`, `npm run start`, `npm run build`).
- Les tâches de migration de contenu sont suivies avec les items de roadmap `DOCS-*`. Les contributions sont bienvenues : portez des sections depuis `docs/source/` et ajoutez la page à la barre latérale.
- Si vous ajoutez un artefact généré (specs, tableaux de configuration), documentez la commande de build pour que les futurs contributeurs puissent le régénérer facilement.
