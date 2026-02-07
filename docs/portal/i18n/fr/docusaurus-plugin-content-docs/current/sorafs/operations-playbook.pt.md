---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : manuel d'opérations
titre : Playbook des opéras du SoraFS
sidebar_label : Playbook des opéras
description : Guides de réponse aux incidents et procédures d'exercices de caos pour les opérateurs du SoraFS.
---

:::note Fonte canonica
Cette page apparaît ou le runbook se présente sous `docs/source/sorafs_ops_playbook.md`. Mantenha ambas as copias syncronizadas ate que o conjunto de documentacao Sphinx seja totalement migré.
:::

## Références chave

- Activités d'observation : consulter les tableaux de bord Grafana dans `dashboards/grafana/` et consulter les tableaux de bord Prometheus dans `dashboards/alerts/`.
- Catalogue de mesures : `docs/source/sorafs_observability_plan.md`.
- Superficies de télémétrie de l'orchestre : `docs/source/sorafs_orchestrator_plan.md`.

## Matrice d'escalade

| Priorité | Exemples de chat | Primaire de garde | Sauvegarde | Notes |
|---------------|------------|------------------|--------|-------|
| P1 | Queda global do gateway, taxa de falha PoR > 5% (15 min), backlog de replicacao dobrando a cada 10 min | Stockage SRE | Observabilité TL | Action du conseil de gouvernance pour un impact ultra-passé 30 min. |
| P2 | Violation de SLO de latence régionale par la passerelle, pic de tentatives de l'explorateur sans impact sur SLA | Observabilité TL | Stockage SRE | Continuez le déploiement, mais bloquez les nouveaux manifestes. |
| P3 | Alertes non critiques (obsolescence des manifestes, capacité 80-90%) | Triage d'admission | Guilde des opérations | Résolveur non utilisé à proximité. |## Queda do gateway / disponibilidade degradada

**Détection**

- Alertes : `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Tableau de bord : `dashboards/grafana/sorafs_gateway_overview.json`.

**Acoes immédiatas**

1. Confirmez l'escopo (fournisseur unique vs frota) via le tableau des taxons des exigences.
2. Appuyez sur le bouton Torii pour les fournisseurs saoudiens (ses multi-fournisseurs) en ajustant `sorafs_gateway_route_weights` pour les opérations de configuration (`docs/source/sorafs_gateway_self_cert.md`).
3. Si tous les résultats sont impactés, vous pouvez utiliser la solution de secours \"direct fetch\" pour les clients CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**Triage**

- Vérifiez l'utilisation des jetons de flux contre `sorafs_gateway_stream_token_limit`.
- Les journaux d'inspection font la passerelle en cas d'erreurs TLS ou d'admission.
- Exécutez `scripts/telemetry/run_schema_diff.sh` pour garantir que le schéma exporté de la passerelle correspond à la version attendue.

**Opérations de remédiation**

- Réinitialisation du processus de passerelle sécurisé ; évitez de recycler tout le cluster, à moins que plusieurs fournisseurs échouent.
- Augmente temporairement ou limite les jetons de flux à 10-15% comme saturation pour confirmation.
- Réexécutez l'auto-certification (`scripts/sorafs_gateway_self_cert.sh`) après stabilisation.

**Pos-incident**

- Enregistrez un post-mortem P1 en utilisant `docs/source/sorafs/postmortem_template.md`.
- L'ordre d'un exercice de caos d'accompagnement se fait en fonction des interventions manuelles.

## Pico de fausses preuves (PoR / PoTR)

**Détection**- Alertes : `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Tableau de bord : `dashboards/grafana/sorafs_proof_integrity.json`.
- Télémétrie : `torii_sorafs_proof_stream_events_total` et événements `sorafs.fetch.error` avec `provider_reason=corrupt_proof`.

**Acoes immédiatas**

1. Congele novas admissoes de manifests sinalizando o registro de manifests (`docs/source/sorafs/manifest_pipeline.md`).
2. Notifier la gouvernance pour suspendre les incitations aux fournisseurs soutenus.

**Triage**

- Vérifiez a profundidade da fila de desafios PoR contra `sorafs_node_replication_backlog_total`.
- Valider le pipeline de vérification des preuves (`crates/sorafs_node/src/potr.rs`) dans les déploiements récents.
- Comparez les versions du firmware des fournisseurs avec le registre des opérateurs.

**Opérations de remédiation**

- Dispare replays PoR usando `sorafs_cli proof stream` com o manifest plus recente.
- Si vous prouvez un changement de forme cohérente, supprimez le fournisseur en même temps que l'actualisation du registre de gouvernance et l'actualisation des tableaux de bord de l'orchestre.

**Pos-incident**

- Exécuter le scénario de forage de caos avant de déployer à proximité la production.
- Enregistrez les aprendizados no template de post-mortem et actualisez la liste de contrôle de qualification des fournisseurs.

## Attraso de réplicacao / crescimento do backlog

**Détection**

- Alertes : `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. Importer
  `dashboards/alerts/sorafs_capacity_rules.yml` et exécuter
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  avant la promotion pour que Alertmanager reflète les seuils documentés.
- Tableau de bord : `dashboards/grafana/sorafs_capacity_health.json`.
- Métriques : `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Acoes immédiatas**1. Vérifiez l'escopo do backlog (fournisseur unique ou frota) et mettez en pause les tarefas de réplication des éléments essentiels.
2. Récupérer le retard pour l'isolement, rétribuer temporairement les nouvelles étapes et les fournisseurs alternatifs via le planificateur de réplication.

**Triage**

- Inspecione a telemetria do orquestrador em busca de bursts of retrys that possam ampliar o backlog.
- Confirmez que les cibles d'armement ont une marge suffisante (`sorafs_node_capacity_utilisation_percent`).
- Réviser les mudancas recentes de configuracao (actualizacoes de chunk profile, cadencia de proofs).

**Opérations de remédiation**

- Exécutez `sorafs_cli` avec l'opération `--rebalance` pour redistribuer le contenu.
- Échelle horizontalement des travailleurs de réplication pour le fournisseur impacté.
- Supprimer l'actualisation du manifeste pour réaliser des tâches TTL.

**Pos-incident**

- Programmez un exercice de capacité axé sur la fausse saturation des fournisseurs.
- Actualiser une documentation de SLA de réplication sur `docs/source/sorafs_node_client_protocol.md`.

## Cadence des exercices de caos

- **Trimestral** : simulation combinée de la passerelle + tempête de tentatives de l'orchestre.
- **Semestral** : injection de faux PoR/PoTR dans deux fournisseurs de récupération.
- **Mensuel de contrôle ponctuel** : scénario d'atraso de réplication en utilisant les manifestes de mise en scène.
- Enregistrez les exercices sans journal partagé (`ops/drill-log.md`) via :

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

- Validez le journal avant les commits avec :

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```- Utilisez `--status scheduled` pour les exercices futurs, `pass`/`fail` pour les exécutions conclues, et `follow-up` lorsque vous arrivez à l'ouverture.
- Remplacer le destin par `--log` pour les essais à sec ou la vérification automatisée ; mais c'est le script continu mis à jour `ops/drill-log.md`.

## Modèle d'autopsie

Utilisez `docs/source/sorafs/postmortem_template.md` pour chaque incident P1/P2 et pour les rétrospectives d'exercices de simulation. Le modèle couvre la ligne du tempo, la quantification de l'impact, les facteurs contribuant, les aspects correctifs et les tarifications de vérification de l'accompagnement.