---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu du lien
titre : Apercu de Sora Nexus
description : Resume de haut niveau de l'architecture Iroha 3 (Sora Nexus) avec des pointeurs vers les docs canoniques du mono-repo.
---

Nexus (Iroha 3) etend Iroha 2 avec l'exécution multi-voie, des espaces de données cadres par la gouvernance et des outils partages sur chaque SDK. Cette page reflète le nouveau brief `docs/source/nexus_overview.md` dans le mono-repo afin que les lecteurs du portail comprennent rapidement comment les pièces de l'architecture s'emboitent.

## Lignes de version

- **Iroha 2** - déploiements auto-hébergements pour consortiums ou réseaux privés.
- **Iroha 3 / Sora Nexus** - réseau public multi-voies ou les opérateurs enregistrent des espaces de données (DS) et héritent d'outils partages de gouvernance, régulation et observabilité.
- Les deux lignes compilent depuis le meme workspace (IVM + toolchain Kotodama), donc les correctifs SDK, les mises à jour ABI et les luminaires Norito restent portables. Les opérateurs téléchargent l'archive `iroha3-<version>-<os>.tar.zst` pour rejoindre Nexus ; reportez-vous a `docs/source/sora_nexus_operator_onboarding.md` pour la checklist plein écran.

## Blocs de construction| Composant | CV | Points du portail |
|-----------|---------|--------------|
| Espace de données (DS) | Domaine d'exécution/stockage défini par la gouvernance qui possède une ou plusieurs voies, déclare des ensembles de validateurs, la classe de confidentialité et la politique de frais + DA. | Voir [Nexus spec](./nexus-spec) pour le schéma du manifeste. |
| Voie | Shard déterministe d'exécution ; emet des engagements que l'anneau NPoS global ordonne. Les classes de voie incluent `default_public`, `public_custom`, `private_permissioned` et `hybrid_confidential`. | Le [modele de lane](./nexus-lane-model) capture la géométrie, les préfixes de stockage et la rétention. |
| Plan de transition | Des identifiants de placeholder, des phases de routage et un packaging double profil tracent comment les déploiements mono-lane évoluant vers Nexus. | Les [notes de transition](./nexus-transition-notes) documentent chaque phase de migration. |
| Répertoire spatial | Contrat registre qui stocke les manifestes + versions DS. Les opérateurs concilient les entrées du catalogue avec ce répertoire avant de rejoindre. | Le suivi des différences de manifeste vit sous `docs/source/project_tracker/nexus_config_deltas/`. || Catalogue des voies | La section de configuration `[nexus]` mappe les ID de voie vers les alias, les politiques de routage et les seuils DA. `irohad --sora --config ... --trace-config` imprimer le catalogue résolu pour les audits. | Utilisez `docs/source/sora_nexus_operator_onboarding.md` pour le parcours CLI. |
| Routeur de réglementation | Orchestrateur de transferts XOR qui connecte des voies CBDC privées aux voies de liquidité publiques. | `docs/source/cbdc_lane_playbook.md` détaille les règlements de politique et les garde-fous de télémétrie. |
| Télémétrie/SLO | Tableaux de bord + alertes sous `dashboards/grafana/nexus_*.json` capturent la hauteur des voies, le backlog DA, la latence de réglementation et la profondeur du fichier de gouvernance. | Le [plan de remédiation de télémétrie](./nexus-telemetry-remediation) détaille les tableaux de bord, alertes et preuves d'audit. |

## Instantané de déploiement| Phases | Mise au point | Critères de sortie |
|-------|-------|--------------------|
| N0 - Bêta fermée | Registrar gere par le conseil (`.sora`), manuel de l'opérateur d'embarquement, catalogue de voies statiques. | Manifestes DS signes + passations de gouvernance répétées. |
| N1 - Lancement public | Ajoute les suffixes `.nexus`, les enchères, un registraire en libre-service, le câblage de règlement XOR. | Tests de synchronisation résolveur/passerelle, tableaux de réconciliation de facturation, exercices de litiges. |
| N2 - Extension | Introduit `.dao`, APIs revendeurs, analytique, portail de litiges, scorecards de stewards. | Artefacts de versions conformes, boîte à outils jury de politique en ligne, rapports de transparence du trésor. |
| Porte NX-12/13/14 | Le moteur de conformité, les tableaux de bord de télémétrie et la documentation doivent sortir ensemble avant les pilotes partenaires. | [Présentation Nexus](./nexus-overview) + [Opérations Nexus](./nexus-operations) publies, câbles tableaux de bord, moteur de politiques fusionne. |

## Responsabilités des opérateurs1. **Hygiene de configuration** - gardez `config/config.toml` synchroniser avec le catalogue public des voies et espaces de données ; archivez la sortie `--trace-config` avec chaque ticket de sortie.
2. **Suivi des manifestes** - conciliez les entrées du catalogue avec le dernier bundle Space Directory avant de rejoindre ou de mettre à niveau les nœuds.
3. **Couverture télémétrie** - exposez les tableaux de bord `nexus_lanes.json`, `nexus_settlement.json` et ceux qui se trouvent aux SDK ; Câblez les alertes à PagerDuty et réalisez des revues trimestrielles selon le plan de remédiation de télémétrie.
4. **Signalement d'incidents** - suivez la matrice de gravité dans [Nexus opérations](./nexus-operations) et déposez les RCA sous cinq jours ouvrables.
5. **Préparation de gouvernance** - assistez aux votes du conseil Nexus impactant vos voies et répétez les instructions de rollback chaque trimestre (suivi via `docs/source/project_tracker/nexus_config_deltas/`).

## Voir aussi

- Ouverture canonique : `docs/source/nexus_overview.md`
- Spécification détaillée : [./nexus-spec](./nexus-spec)
- Géométrie des voies : [./nexus-lane-model](./nexus-lane-model)
- Plan de transition : [./nexus-transition-notes](./nexus-transition-notes)
- Plan de remédiation de télémétrie : [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Opérations Runbook : [./nexus-operations](./nexus-operations)
- Guide d'onboarding opérateur : `docs/source/sora_nexus_operator_onboarding.md`