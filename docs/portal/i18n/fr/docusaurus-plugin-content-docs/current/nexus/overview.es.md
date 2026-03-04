---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu du lien
titre : Résumé de Sora Nexus
description : Résumé de haut niveau de l'architecture de Iroha 3 (Sora Nexus) avec enlaces les documents canoniques du mono-repo.
---

Nexus (Iroha 3) et Iroha 2 avec exécution multivoie, espaces de données pris en charge par le gouvernement et les outils partagés dans chaque SDK. Cette page reflète le nouveau résumé `docs/source/nexus_overview.md` du mono-repo pour que les lecteurs du portail puissent rapidement saisir les pièces de l'architecture.

## Lignes de lancement

- **Iroha 2** - despliegues autoalojados para consorcios o redes privadas.
- **Iroha 3 / Sora Nexus** - la rouge publique multivoie où les opérateurs enregistrent des espaces de données (DS) et voici les outils de gouvernance, de liquidation et d'observation.
- Des lignes supplémentaires ont été compilées à partir de cet espace de travail (IVM + chaîne d'outils Kotodama), car les corrections du SDK, les mises à jour d'ABI et les appareils Norito sont toujours portables. Les opérateurs téléchargent le paquet `iroha3-<version>-<os>.tar.zst` pour unirse a Nexus ; Consultez `docs/source/sora_nexus_operator_onboarding.md` pour la liste de vérification sur l'écran complet.

## Blocs de construction| Composants | CV | Ganchos du portail |
|-----------|---------|--------------|
| Espace de données (DS) | Dominio de ejecucion/almacenamiento défini par l'administration qui pose una o mas voies, déclarea conjuntos de validadores, clase de privacidad y politica de tarifsas + DA. | Consultez la [spécification Nexus] (./nexus-spec) pour l'esquema del manifeste. |
| Voie | Fragmento déterministe d'éjection ; émettre des compromis qui ordonnent l’anneau mondial NPoS. Les classes de voie incluent `default_public`, `public_custom`, `private_permissioned` et `hybrid_confidential`. | Le [modèle de voie](./nexus-lane-model) capture la géométrie, les paramètres de stockage et de rétention. |
| Plan de transition | Espace réservé aux identifiants, phases de recrutement et empaquetado de double profil siguen como los despliegues d'une voie solo évolutive hacia Nexus. | Les [notes de transition](./nexus-transition-notes) documentent chaque phase de migration. |
| Directeur des espaces | Contrato de registro que almacena manifiestos + versions de DS. Les opérateurs concilient les entrées du catalogue avec ce directeur avant l'université. | Le rastreador des différences de manifestes vit en `docs/source/project_tracker/nexus_config_deltas/`. || Catalogue des voies | Section `[nexus]` de configuration pour l'attribution des identifiants de voie à un alias, politiques d'inscription et cadres DA. `irohad --sora --config ... --trace-config` imprime le catalogue de résultats pour les auditoriums. | Utilisez `docs/source/sora_nexus_operator_onboarding.md` pour l'enregistrement de CLI. |
| Routeur de liquidation | L'Orquestateur de transferts XOR connecte les voies CBDC privées aux voies de liquidation publiques. | `docs/source/cbdc_lane_playbook.md` détaille les boutons de politique et les ordinateurs de télémétrie. |
| Télémétrie/SLO | Panneaux + alertes sous `dashboards/grafana/nexus_*.json` capturant l'altitude des voies, l'arriéré DA, la latence de liquidation et la profondeur de la tête de gouvernement. | Le [plan de remédiation de télémétrie](./nexus-telemetry-remediation) détaille les panneaux, les alertes et les preuves de l'auditoire. |

## Instantanea de dépliage| Phase | Enfique | Critères de sortie |
|-------|-------|--------------------|
| N0 - Bêta Cerrada | Registrar géré par le consejo (`.sora`), manuel d'incorporation des opérateurs, catalogue des voies statiques. | Manifiestos DS firmados + traspasos de gobernanza ensayados. |
| N1 - Lancement public | Anade sufijos `.nexus`, subastas, registrar de autoservicio, cableado de liquidación XOR. | Essais de synchronisation des résolveurs/passerelles, panels de réconciliation de facturation, simulacres de litiges. |
| N2 - Extension | Présentez `.dao`, les API de rapport, d'analyse, le portail de litiges, les tableaux de bord des stewards. | Artefacts de cumplimiento versionados, boîte à outils du jurado de politique en ligne, informe de la transparence du trésor. |
| Porte NX-12/13/14 | Le moteur d'assemblage, les panneaux de télémétrie et la documentation doivent être utilisés ensemble avant les pilotes avec les socios. | [Présentation Nexus](./nexus-overview) + [Opérations Nexus](./nexus-operations) publiés, panneaux connectés, moteur de politique fusionné. |

## Responsabilités de l'opérateur1. **Hygiène de configuration** - maintenir `config/config.toml` synchronisé avec le catalogue publié de voies et d'espaces de données ; archiver la sortie de `--trace-config` avec chaque ticket de sortie.
2. **Suite des manifestes** - conciliez les entrées du catalogue avec le paquet le plus récent du Space Directory avant d'unir ou d'actualiser des nœuds.
3. **Cobertura de telemetria** - expose les panneaux `nexus_lanes.json`, `nexus_settlement.json` et les tableaux de bord associés au SDK ; connectez-vous aux alertes de PagerDuty et exécutez des révisions trimestrielles en fonction du plan de remédiation de la télémétrie.
4. **Rapport d'incidents** - suivez la matrice de gravité dans [Nexus opérations](./nexus-operations) et présentez les RCA dentro de 5 dias habiles.
5. **Préparation de la mise en œuvre** - assistez aux votes du conseil Nexus qui affectent vos voies et suivez les instructions de restauration trimestrielle (suivi en `docs/source/project_tracker/nexus_config_deltas/`).

## Voir aussi

- CV canonique : `docs/source/nexus_overview.md`
- Spécification détaillée : [./nexus-spec](./nexus-spec)
- Géométrie des voies : [./nexus-lane-model](./nexus-lane-model)
- Plan de transition : [./nexus-transition-notes](./nexus-transition-notes)
- Plan de remédiation de télémétrie : [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook des opérations : [./nexus-operations](./nexus-operations)
- Guide d'intégration des opérateurs : `docs/source/sora_nexus_operator_onboarding.md`