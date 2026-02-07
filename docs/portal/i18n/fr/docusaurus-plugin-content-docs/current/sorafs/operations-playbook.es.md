---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : manuel d'opérations
titre : Playbook des opérations de SoraFS
sidebar_label : Playbook des opérations
description : Guides de réponse aux incidents et procédures d'exercices de caos pour les opérateurs de SoraFS.
---

:::note Source canonique
Cette page reflète le runbook géré par `docs/source/sorafs_ops_playbook.md`. Assurez-vous que les copies sont synchronisées jusqu'à ce que le ensemble de documents Sphinx soit migré complètement.
:::

## Références clés

- Actifs d'observabilité : consultez les tableaux de bord de Grafana et `dashboards/grafana/` et les règles d'alerte de Prometheus et `dashboards/alerts/`.
- Catalogue de mesures : `docs/source/sorafs_observability_plan.md`.
- Superficies de télémétrie de l'orchestre : `docs/source/sorafs_orchestrator_plan.md`.

## Matrice d'escalade| Priorité | Exemples de disparo | Directeur de garde | Sauvegarde | Notes |
|----------|-----------|------------------|--------|-------|
| P1 | Caída global del gateway, tasa de fallos PoR > 5% (15 min), backlog de réplicación duplicándose chaque 10 min | Stockage SRE | Observabilité TL | Involucra al consejo de gobernanza si el impacto supera 30 min. |
| P2 | Incumplimiento de SLO de latencia Regional del Gateway, pico de reintentos de l'orquestador sin impacto de SLA | Observabilité TL | Stockage SRE | Continuer le déploiement mais bloquer de nouveaux manifestes. |
| P3 | Alertes sans critiques (obsolescence des manifestes, capacité 80–90%) | Triage d'admission | Guilde des opérations | Résolvez le jour suivant. |

## Caída del gateway / disponibilité dégradée

**Détection**

- Alertes : `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Tableau de bord : `dashboards/grafana/sorafs_gateway_overview.json`.

**Acciones immédiates**

1. Confirmez l'emplacement (fournisseur unique contre flot) via le panneau de commande.
2. Changez l'inscription de Torii à des fournisseurs sains (s'ils sont multi-fournisseurs) en activant `sorafs_gateway_route_weights` dans la configuration des opérations (`docs/source/sorafs_gateway_self_cert.md`).
3. Si tous les fournisseurs sont concernés, activez le repli de « récupération directe » pour les clients CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**Triage**- Révisez l'utilisation du jeton de flux avant vers `sorafs_gateway_stream_token_limit`.
- Inspection des journaux de la passerelle pour les erreurs de TLS ou d'admission.
- Exécutez `scripts/telemetry/run_schema_diff.sh` pour garantir que le texte exporté par la passerelle coïncide avec la version attendue.

**Options de remédiation**

- Réinitialiser seul le processus de passerelle affecté ; Evita reciclar todo el cluster salvo que falled divers provenedores.
- Augmente la limite de jetons de flux de 10 à 15 % de forme temporelle si la saturation est confirmée.
- Rejecuta el self-cert (`scripts/sorafs_gateway_self_cert.sh`) après la stabilisation.

**Post-incident**

- Enregistrer un post-mortem P1 en utilisant `docs/source/sorafs/postmortem_template.md`.
- Programmez un exercice de caos de suivi si la remédiation nécessite des interventions manuelles.

## Pico de fallos de pruebas (PoR / PoTR)

**Détection**

- Alertes : `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Tableau de bord : `dashboards/grafana/sorafs_proof_integrity.json`.
- Télémétrie : `torii_sorafs_proof_stream_events_total` et événements `sorafs.fetch.error` avec `provider_reason=corrupt_proof`.

**Acciones immédiates**

1. Congela nuevas admissiones de manifests marcando el registro de manifests (`docs/source/sorafs/manifest_pipeline.md`).
2. Notifier la gouvernance pour suspendre les incitations des fournisseurs concernés.

**Triage**

- Réviser la profondeur de la cola de desafíos PoR frente a `sorafs_node_replication_backlog_total`.
- Validez le pipeline de vérification des essais (`crates/sorafs_node/src/potr.rs`) pour les applications récentes.
- Comparez les versions du firmware des fournisseurs avec le registre des opérateurs.**Options de remédiation**

- Dispara replays de PoR en utilisant `sorafs_cli proof stream` avec le manifeste le plus récent.
- Si les essais échouent systématiquement, éliminez le fournisseur du programme actif en actualisant le registre des gouverneurs et en forçant la fresque des tableaux de bord de l'orchestre.

**Post-incident**

- Exécutez le scénario de forage de caos PoR avant le prochain déploiement d'une production.
- Capturez les leçons apprises sur la plante post-mortem et actualisez la liste de contrôle de qualification des fournisseurs.

## Retraso de réplicación / crecimiento del backlog

**Détection**

- Alertes : `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. Importation
  `dashboards/alerts/sorafs_capacity_rules.yml` et exécuté
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  avant la promotion pour qu'Alertmanager reflète les parapluies documentés.
- Tableau de bord : `dashboards/grafana/sorafs_capacity_health.json`.
- Métriques : `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Acciones immédiates**

1. Vérifiez l'ampleur du retard (fournisseur unique ou flottant) et faites des pauses dans les zones de réplication non essentielles.
2. Si le retard est réglé, réaffectez temporairement de nouvelles commandes aux fournisseurs alternés au milieu du planificateur de réplication.

**Triage**

- Inspecciona la télémétrie de l'orquestador por ráfagas de reintentos qui puedan aggraver l'arriéré.
- Confirmez que les cibles de stockage ont une marge suffisante (`sorafs_node_capacity_utilisation_percent`).
- Révision des modifications récentes de configuration (actualisation des profils de bloc, cadence de preuves).**Options de remédiation**

- Exécutez `sorafs_cli` avec l'option `--rebalance` pour redistribuer le contenu.
- Escala horizontalement les travailleurs de réplication pour le fournisseur concerné.
- Afficher un rafraîchissement des manifestes pour réalister les fenêtres TTL.

**Post-incident**

- Programmez un exercice de capacité optimisé en cas de saturation des fournisseurs.
- Actualiser la documentation de SLA de réplication en `docs/source/sorafs_node_client_protocol.md`.

## Cadence des exercices de caos

- **Trimestral** : simulation combinée de caída de gateway + tourmenta de reintentos del orquestador.
- **Semestral** : injection de pertes PoR/PoTR chez deux fournisseurs avec récupération.
- **Chèque mensuel** : scénario de retour de réplication à l'aide des manifestes de mise en scène.
- Enregistrez les exercices dans le journal partagé (`ops/drill-log.md`) via :

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- Valider le journal avant les commits avec :

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- Utilisez `--status scheduled` pour les forets futurs, `pass`/`fail` pour les exécutions complètes, et `follow-up` lorsque vous effectuez des opérations ouvertes.
- Notez le destin avec `--log` pour les essais à sec ou la vérification automatisée ; mais ce script est actualisé `ops/drill-log.md`.

## Plante post-mortemUsa `docs/source/sorafs/postmortem_template.md` pour chaque incident P1/P2 et pour les rétrospectives des exercices de caos. La plante comprend une chronologie, une évaluation de l'impact, des facteurs contribuant, des actions correctives et des tâches de vérification de la suite.