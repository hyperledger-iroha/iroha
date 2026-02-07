---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : manuel d'opérations
titre : SoraFS آپریشنز پلے بک
sidebar_label : آپریشنز پلے بک
description : SoraFS Service de réponse aux incidents et exercice de chaos
---

:::note مستند ماخذ
Le modèle `docs/source/sorafs_ops_playbook.md` est utilisé pour le runbook et le runbook. Le Sphinx est en vente libre et il est prêt à s'en servir.
:::

## کلیدی حوالہ جات

- Actifs d'observabilité : tableaux de bord `dashboards/grafana/` et Grafana et règles d'alerte `dashboards/alerts/` et Prometheus.
- Catalogue métrique : `docs/source/sorafs_observability_plan.md`.
- Surfaces de télémétrie Orchestrator : `docs/source/sorafs_orchestrator_plan.md`.

## Matrice d'escalade

| ترجیح | ٹرگر مثالیں | Service de garde | بیک اپ | نوٹس |
|--------|--------------|---------------|--------|------|
| P1 | Panne de passerelle, taux d'échec PoR > 5 % (15 mois) et retard de réplication d'environ 10 mois | Stockage SRE | Observabilité TL | Il y a 30 ans, il y a un conseil de gouvernance et un engagement |
| P2 | Violation du SLO de latence de la passerelle et pic de nouvelles tentatives de l'orchestrateur pour le SLA | Observabilité TL | Stockage SRE | déploiement جاری رکھیں مگر نئے manifeste la porte کریں۔ |
| P3 | غیر اہم alertes (obsolescence manifeste, capacité 80–90%) | Triage d'admission | Guilde des opérations | اگلے کاروباری دن میں نمٹا دیں۔ |

## Panne de la passerelle / disponibilité dégradée

**Détection**

- Alertes : `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Tableau de bord : `dashboards/grafana/sorafs_gateway_overview.json`.

**Actions immédiates**1. Panneau de taux de demande et portée de la demande (fournisseur unique vs flotte)
2. Configuration multi-fournisseurs et opérations (`docs/source/sorafs_gateway_self_cert.md`) avec `sorafs_gateway_route_weights` pour le routage Torii et les fournisseurs de services. کریں۔
3. Les fournisseurs de services utilisent des clients CLI/SDK comme une solution de repli « récupération directe » (`docs/source/sorafs_node_client_protocol.md`).

**Triage**

- `sorafs_gateway_stream_token_limit` pour l'utilisation du jeton de flux
- Les erreurs d'admission TLS et les journaux de passerelle inspectent le système
- `scripts/telemetry/run_schema_diff.sh` représente la passerelle, le schéma exporté et la version attendue et correspondent aux spécifications.

**Options de correction**

- صرف متاثرہ gateway process ری اسٹارٹ کریں؛ Il s'agit d'un cluster et d'un recyclage pour tous les fournisseurs de services.
- La saturation est maximale et la limite de jetons de flux est comprise entre 10 et 15 %.
- استحکام کے بعد auto-certification دوبارہ چلائیں (`scripts/sorafs_gateway_self_cert.sh`).

**Après l'incident**

- `docs/source/sorafs/postmortem_template.md` استعمال کرتے ہوئے P1 post-mortem فائل کریں۔
- L'assainissement et les interventions manuelles ainsi que les exercices de suivi du chaos et les interventions manuelles.

## Pic d'échec de preuve (PoR / PoTR)

**Détection**

- Alertes : `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Tableau de bord : `dashboards/grafana/sorafs_proof_integrity.json`.
- Télémétrie : événements `torii_sorafs_proof_stream_events_total` et `sorafs.fetch.error` et `provider_reason=corrupt_proof`.

**Actions immédiates**

1. Registre des manifestes et drapeau pour le gel des admissions manifestes (`docs/source/sorafs/manifest_pipeline.md`).
2. Gouvernance et informer les fournisseurs de services et les incitations en pause**Triage**

- Profondeur de la file d'attente de défi PoR (`sorafs_node_replication_backlog_total`)
- Les déploiements en ligne avec un pipeline de vérification de preuve (`crates/sorafs_node/src/potr.rs`) valident le pipeline
- versions du micrologiciel du fournisseur et registre des opérateurs et comparaison

**Options de correction**

- Le manifeste du manifeste `sorafs_cli proof stream` est utilisé pour déclencher les replays PoR.
- Les preuves d'échec et le registre de gouvernance ainsi que le fournisseur et l'ensemble actif ainsi que les tableaux de bord de l'orchestrateur et l'actualisation des tableaux de bord de l'orchestrateur.

**Après l'incident**

- Pour déployer un scénario d'exercice de chaos PoR et un scénario de forage de chaos PoR.
- modèle post-mortem pour capture de leçons et liste de contrôle de qualification des fournisseurs

## Retard de réplication/croissance du backlog

**Détection**

- Alertes : `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. `dashboards/alerts/sorafs_capacity_rules.yml` امپورٹ کریں اور
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  promotion et les seuils documentés par Alertmanager reflètent les problèmes
- Tableau de bord : `dashboards/grafana/sorafs_capacity_health.json`.
- Métriques : `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Actions immédiates**

1. backlog et portée du projet (fournisseur unique et flotte) et tâches de réplication
2. L'arriéré isolé et le planificateur de réplication ainsi que les commandes de réplication et les fournisseurs alternatifs et la réaffectation des commandes.

**Triage**- la télémétrie de l'orchestrateur et les nouvelles tentatives en rafale ainsi que le backlog des tâches
- les cibles de stockage ont une marge de sécurité maximale (`sorafs_node_capacity_utilisation_percent`).
- Modifications de la configuration (mises à jour du profil de fragments et cadence de preuve)

**Options de correction**

- `sorafs_cli` et `--rebalance` pour redistribuer le contenu
- Fournisseur de services de réplication pour les travailleurs de réplication à échelle horizontale
- Les fenêtres TTL s'alignent sur le déclencheur d'actualisation du manifeste.

**Après l'incident**

- échec de saturation du fournisseur en cas de forage de capacité
- documentation SLA de réplication selon le modèle `docs/source/sorafs_node_client_protocol.md`

## Cadence des exercices du chaos

- **Testimaire** : panne combinée de la passerelle + simulation de tempête de nouvelles tentatives de l'orchestrateur.
- **Biennal** : injection de défaillance PoR/PoTR et fournisseurs de récupération et de récupération.
- **Contrôle ponctuel mensuel** : les manifestes de préparation et le scénario de décalage de réplication.
- les exercices et le journal du runbook partagé (`ops/drill-log.md`) sont associés à la version suivante :

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

- commits et validation du journal par exemple :

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- Les exercices et les exercices `--status scheduled`, ainsi que les éléments d'action `pass`/`fail`, exécutent des actions. `follow-up` استعمال کریں۔
- essais à sec et vérification automatisée de la destination `--log` ou remplacement de la destination ورنہ اسکرپٹ `ops/drill-log.md` کو اپڈیٹ کرتا رہے گا۔

## Modèle post-mortemIncident P1/P2 et rétrospectives d'exercices de chaos par `docs/source/sorafs/postmortem_template.md`. یہ calendrier modèle, quantification de l'impact, facteurs contributifs, actions correctives, et tâches de vérification de suivi.