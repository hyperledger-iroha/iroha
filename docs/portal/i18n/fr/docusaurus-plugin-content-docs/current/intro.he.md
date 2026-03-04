---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/intro.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ddc7ff0c9dc3bbedcaa089acfc2c7ccb04b53086f763b1d59a49935af4f834ef
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/intro.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Bienvenue sur le portail developpeur SORA Nexus

Le portail developpeur SORA Nexus regroupe de la documentation interactive, des tutoriels SDK et des references d'API pour les operateurs Nexus et les contributeurs Hyperledger Iroha. Il complete le site principal de docs en mettant en avant des guides pratiques et des specifications generees directement depuis ce depot. La page d'accueil propose desormais des points d'entree thematiques Norito/SoraFS, des snapshots OpenAPI signes et une reference dediee a Norito Streaming afin que les contributeurs puissent trouver le contrat du plan de controle du streaming sans fouiller la specification racine.

## Ce que vous pouvez faire ici

- **Apprendre Norito** - commencez par l'apercu et le quickstart pour comprendre le modele de serialisation et les outils de bytecode.
- **Demarrer les SDKs** - suivez les quickstarts JavaScript et Rust des aujourd'hui ; les guides Python, Swift et Android les rejoindront au fur et a mesure de la migration des recettes.
- **Parcourir les references API** - la page OpenAPI de Torii rend la derniere specification REST, et les tableaux de configuration renvoient aux sources Markdown canoniques.
- **Preparer les deploiements** - les runbooks operationnels (telemetry, settlement, Nexus overlays) sont en cours de portage depuis `docs/source/` et arriveront ici a mesure que la migration avance.

## Statut actuel

-  Landing Docusaurus v3 thematique avec typographie rafraichie, hero/cards guides par des degrades et tuiles de ressources incluant le resume Norito Streaming.
-  Plugin OpenAPI Torii cable sur `npm run sync-openapi`, avec verifications de snapshots signes et protections CSP appliquees par `buildSecurityHeaders`.
-  La couverture preview et probe s'execute en CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), et bloque desormais la doc streaming, les quickstarts SoraFS et les checklists de reference avant publication des artefacts.
-  Les quickstarts Norito, SoraFS et SDK ainsi que les sections de reference sont dans la barre laterale ; les nouvelles importations depuis `docs/source/` (streaming, orchestration, runbooks) arrivent ici au fil de leur redaction.

## Participer

- Voir `docs/portal/README.md` pour les commandes de developpement local (`npm install`, `npm run start`, `npm run build`).
- Les taches de migration de contenu sont suivies avec les items de roadmap `DOCS-*`. Les contributions sont bienvenues : portez des sections depuis `docs/source/` et ajoutez la page a la barre laterale.
- Si vous ajoutez un artefact genere (specs, tableaux de configuration), documentez la commande de build pour que les futurs contributeurs puissent le regenerer facilement.
