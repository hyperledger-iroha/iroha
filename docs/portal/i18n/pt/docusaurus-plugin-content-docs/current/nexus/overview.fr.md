---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-overview
title: Apercu de Sora Nexus
description: Resume de haut niveau de l'architecture Iroha 3 (Sora Nexus) avec des pointeurs vers les docs canoniques du mono-repo.
---

Nexus (Iroha 3) etend Iroha 2 avec l'execution multi-lane, des espaces de donnees cadres par la gouvernance et des outils partages sur chaque SDK. Cette page reflete le nouveau brief `docs/source/nexus_overview.md` dans le mono-repo afin que les lecteurs du portail comprennent rapidement comment les pieces de l'architecture s'emboitent.

## Lignes de version

- **Iroha 2** - deploiements auto-heberges pour consortiums ou reseaux prives.
- **Iroha 3 / Sora Nexus** - reseau public multi-lane ou les operateurs enregistrent des espaces de donnees (DS) et heritent d'outils partages de gouvernance, reglement et observabilite.
- Les deux lignes compilent depuis le meme workspace (IVM + toolchain Kotodama), donc les correctifs SDK, les mises a jour ABI et les fixtures Norito restent portables. Les operateurs telechargent l'archive `iroha3-<version>-<os>.tar.zst` pour rejoindre Nexus ; reportez-vous a `docs/source/sora_nexus_operator_onboarding.md` pour la checklist plein ecran.

## Blocs de construction

| Composant | Resume | Points du portail |
|-----------|---------|--------------|
| Espace de donnees (DS) | Domaine d'execution/stockage defini par la gouvernance qui possede une ou plusieurs lanes, declare des ensembles de validateurs, la classe de confidentialite et la politique de frais + DA. | Voir [Nexus spec](./nexus-spec) pour le schema du manifeste. |
| Lane | Shard deterministe d'execution ; emet des engagements que l'anneau NPoS global ordonne. Les classes de lane incluent `default_public`, `public_custom`, `private_permissioned` et `hybrid_confidential`. | Le [modele de lane](./nexus-lane-model) capture la geometrie, les prefixes de stockage et la retention. |
| Plan de transition | Des identifiants de placeholder, des phases de routage et un packaging double profil tracent comment les deploiements mono-lane evoluent vers Nexus. | Les [notes de transition](./nexus-transition-notes) documentent chaque phase de migration. |
| Space Directory | Contrat registre qui stocke les manifestes + versions DS. Les operateurs concilient les entrees du catalogue avec ce repertoire avant de rejoindre. | Le suivi des diffs de manifeste vit sous `docs/source/project_tracker/nexus_config_deltas/`. |
| Catalogue de lanes | La section de configuration `[nexus]` mappe les IDs de lane vers des alias, politiques de routage et seuils DA. `irohad --sora --config ... --trace-config` imprime le catalogue resolu pour les audits. | Utilisez `docs/source/sora_nexus_operator_onboarding.md` pour le parcours CLI. |
| Routeur de reglement | Orchestrateur de transferts XOR qui connecte des lanes CBDC privees aux lanes de liquidite publiques. | `docs/source/cbdc_lane_playbook.md` detaille les reglages de politique et les garde-fous de telemetrie. |
| Telemetrie/SLOs | Tableaux de bord + alertes sous `dashboards/grafana/nexus_*.json` capturent la hauteur des lanes, le backlog DA, la latence de reglement et la profondeur de la file de gouvernance. | Le [plan de remediation de telemetrie](./nexus-telemetry-remediation) detaille les tableaux de bord, alertes et preuves d'audit. |

## Instantane de deploiement

| Phase | Focus | Criteres de sortie |
|-------|-------|---------------|
| N0 - Beta fermee | Registrar gere par le conseil (`.sora`), onboarding operateur manuel, catalogue de lanes statique. | Manifestes DS signes + passations de gouvernance repetees. |
| N1 - Lancement public | Ajoute les suffixes `.nexus`, les encheres, un registrar en libre-service, le cablage de reglement XOR. | Tests de synchronisation resolver/gateway, tableaux de reconciliation de facturation, exercices de litiges. |
| N2 - Expansion | Introduit `.dao`, APIs revendeurs, analytique, portail de litiges, scorecards de stewards. | Artefacts de conformite versionnes, toolkit jury de politique en ligne, rapports de transparence du tresor. |
| Porte NX-12/13/14 | Le moteur de conformite, les dashboards de telemetrie et la documentation doivent sortir ensemble avant les pilotes partenaires. | [Nexus overview](./nexus-overview) + [Nexus operations](./nexus-operations) publies, dashboards cables, moteur de politiques fusionne. |

## Responsabilites des operateurs

1. **Hygiene de configuration** - gardez `config/config.toml` synchronise avec le catalogue publie des lanes et dataspaces ; archivez la sortie `--trace-config` avec chaque ticket de release.
2. **Suivi des manifestes** - conciliez les entrees du catalogue avec le dernier bundle Space Directory avant de rejoindre ou de mettre a niveau les noeuds.
3. **Couverture telemetrie** - exposez les dashboards `nexus_lanes.json`, `nexus_settlement.json` et ceux lies aux SDK ; cablez les alertes a PagerDuty et realisez des revues trimestrielles selon le plan de remediation de telemetrie.
4. **Signalement d'incidents** - suivez la matrice de severite dans [Nexus operations](./nexus-operations) et deposez les RCAs sous cinq jours ouvrables.
5. **Preparation de gouvernance** - assistez aux votes du conseil Nexus impactant vos lanes et repetez les instructions de rollback chaque trimestre (suivi via `docs/source/project_tracker/nexus_config_deltas/`).

## Voir aussi

- Apercu canonique : `docs/source/nexus_overview.md`
- Specification detaillee : [./nexus-spec](./nexus-spec)
- Geometrie des lanes : [./nexus-lane-model](./nexus-lane-model)
- Plan de transition : [./nexus-transition-notes](./nexus-transition-notes)
- Plan de remediation de telemetrie : [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook operations : [./nexus-operations](./nexus-operations)
- Guide d'onboarding operateur : `docs/source/sora_nexus_operator_onboarding.md`
