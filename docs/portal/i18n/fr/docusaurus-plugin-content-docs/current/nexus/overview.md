<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-overview
title: Aperçu de Sora Nexus
description: Résumé de haut niveau de l'architecture Iroha 3 (Sora Nexus) avec des pointeurs vers les docs canoniques du mono-repo.
---

Nexus (Iroha 3) étend Iroha 2 avec l'exécution multi-lane, des espaces de données cadrés par la gouvernance et des outils partagés sur chaque SDK. Cette page reflète le nouveau brief `docs/source/nexus_overview.md` dans le mono-repo afin que les lecteurs du portail comprennent rapidement comment les pièces de l'architecture s'emboîtent.

## Lignes de version

- **Iroha 2** - déploiements auto-hébergés pour consortiums ou réseaux privés.
- **Iroha 3 / Sora Nexus** - réseau public multi-lane où les opérateurs enregistrent des espaces de données (DS) et héritent d'outils partagés de gouvernance, règlement et observabilité.
- Les deux lignes compilent depuis le même workspace (IVM + toolchain Kotodama), donc les correctifs SDK, les mises à jour ABI et les fixtures Norito restent portables. Les opérateurs téléchargent l'archive `iroha3-<version>-<os>.tar.zst` pour rejoindre Nexus ; reportez-vous à `docs/source/sora_nexus_operator_onboarding.md` pour la checklist plein écran.

## Blocs de construction

| Composant | Résumé | Points du portail |
|-----------|---------|--------------|
| Espace de données (DS) | Domaine d'exécution/stockage défini par la gouvernance qui possède une ou plusieurs lanes, déclare des ensembles de validateurs, la classe de confidentialité et la politique de frais + DA. | Voir [Nexus spec](./nexus-spec) pour le schéma du manifeste. |
| Lane | Shard déterministe d'exécution ; émet des engagements que l'anneau NPoS global ordonne. Les classes de lane incluent `default_public`, `public_custom`, `private_permissioned` et `hybrid_confidential`. | Le [modèle de lane](./nexus-lane-model) capture la géométrie, les préfixes de stockage et la rétention. |
| Plan de transition | Des identifiants de placeholder, des phases de routage et un packaging double profil tracent comment les déploiements mono-lane évoluent vers Nexus. | Les [notes de transition](./nexus-transition-notes) documentent chaque phase de migration. |
| Space Directory | Contrat registre qui stocke les manifestes + versions DS. Les opérateurs concilient les entrées du catalogue avec ce répertoire avant de rejoindre. | Le suivi des diffs de manifeste vit sous `docs/source/project_tracker/nexus_config_deltas/`. |
| Catalogue de lanes | La section de configuration `[nexus]` mappe les IDs de lane vers des alias, politiques de routage et seuils DA. `irohad --sora --config … --trace-config` imprime le catalogue résolu pour les audits. | Utilisez `docs/source/sora_nexus_operator_onboarding.md` pour le parcours CLI. |
| Routeur de règlement | Orchestrateur de transferts XOR qui connecte des lanes CBDC privées aux lanes de liquidité publiques. | `docs/source/cbdc_lane_playbook.md` détaille les réglages de politique et les garde-fous de télémétrie. |
| Télémétrie/SLOs | Tableaux de bord + alertes sous `dashboards/grafana/nexus_*.json` capturent la hauteur des lanes, le backlog DA, la latence de règlement et la profondeur de la file de gouvernance. | Le [plan de remédiation de télémétrie](./nexus-telemetry-remediation) détaille les tableaux de bord, alertes et preuves d'audit. |

## Instantané de déploiement

| Phase | Focus | Critères de sortie |
|-------|-------|---------------|
| N0 - Bêta fermée | Registrar géré par le conseil (`.sora`), onboarding opérateur manuel, catalogue de lanes statique. | Manifestes DS signés + passations de gouvernance répétées. |
| N1 - Lancement public | Ajoute les suffixes `.nexus`, les enchères, un registrar en libre-service, le câblage de règlement XOR. | Tests de synchronisation resolver/gateway, tableaux de réconciliation de facturation, exercices de litiges. |
| N2 - Expansion | Introduit `.dao`, APIs revendeurs, analytique, portail de litiges, scorecards de stewards. | Artefacts de conformité versionnés, toolkit jury de politique en ligne, rapports de transparence du trésor. |
| Porte NX-12/13/14 | Le moteur de conformité, les dashboards de télémétrie et la documentation doivent sortir ensemble avant les pilotes partenaires. | [Nexus overview](./nexus-overview) + [Nexus operations](./nexus-operations) publiés, dashboards câblés, moteur de politiques fusionné. |

## Responsabilités des opérateurs

1. **Hygiène de configuration** - gardez `config/config.toml` synchronisé avec le catalogue publié des lanes et dataspaces ; archivez la sortie `--trace-config` avec chaque ticket de release.
2. **Suivi des manifestes** - conciliez les entrées du catalogue avec le dernier bundle Space Directory avant de rejoindre ou de mettre à niveau les noeuds.
3. **Couverture télémétrie** - exposez les dashboards `nexus_lanes.json`, `nexus_settlement.json` et ceux liés aux SDK ; câblez les alertes à PagerDuty et réalisez des revues trimestrielles selon le plan de remédiation de télémétrie.
4. **Signalement d'incidents** - suivez la matrice de sévérité dans [Nexus operations](./nexus-operations) et déposez les RCAs sous cinq jours ouvrables.
5. **Préparation de gouvernance** - assistez aux votes du conseil Nexus impactant vos lanes et répétez les instructions de rollback chaque trimestre (suivi via `docs/source/project_tracker/nexus_config_deltas/`).

## Voir aussi

- Aperçu canonique : `docs/source/nexus_overview.md`
- Spécification détaillée : [./nexus-spec](./nexus-spec)
- Géométrie des lanes : [./nexus-lane-model](./nexus-lane-model)
- Plan de transition : [./nexus-transition-notes](./nexus-transition-notes)
- Plan de remédiation de télémétrie : [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook opérations : [./nexus-operations](./nexus-operations)
- Guide d'onboarding opérateur : `docs/source/sora_nexus_operator_onboarding.md`
