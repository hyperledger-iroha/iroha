---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : manuel d'opérations
titre : Плейбук операций SoraFS
sidebar_label : Nom d'utilisateur
description : Dispositif de réinstallation pour les incidents et procédures pour les opérateurs SoraFS.
---

:::note Канонический источник
Cette page vient du réseau, qui se trouve dans `docs/source/sorafs_ops_playbook.md`. Vous pouvez obtenir des copies de synchronisation pour que la documentation de Sphinx ne soit pas transférée.
:::

## Clés à molette

- Activités disponibles : utilisez les ports Grafana dans `dashboards/grafana/` et activez les alertes Prometheus dans `dashboards/alerts/`.
- Mesure du catalogue : `docs/source/sorafs_observability_plan.md`.
- opérateur télémétrique public : `docs/source/sorafs_orchestrator_plan.md`.

## Матрица эскалации

| Priorité | Exemples de déclenchements | Service de garde | Réserves | Première |
|-----------|---------|-----------------|--------|------------|
| P1 | Глобальная остановка gateway, уровень отказов PoR > 5% (15 min), arriéré de réplication удваивается каждые 10 min | Stockage SRE | Observabilité TL | Mettez-vous au courant de la gouvernance, si ce n'est avant 30 minutes. |
| P2 | Mise en place d'un SLO régional avec une passerelle de latence, toutes les tentatives de l'opérateur sans accord avec le SLA | Observabilité TL | Stockage SRE | Commencez le déploiement et ne bloquez pas les nouveaux manifestes. |
| P3 | Alertes nécrotiques (l'obsolescence se manifeste, 80 à 90%) | Triage d'admission | Guilde des opérations | Исправить в следующий рабочий день. |## Ouvrir la passerelle / Dégrader les installations

**Обнаружение**

- Alertes : `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Tableau de bord : `dashboards/grafana/sorafs_gateway_overview.json`.

**Description spéciale**

1. Mettez à jour le volume (de votre fournisseur ou de votre flotte) sur le taux de demande du panneau.
2. Sélectionnez la configuration Torii pour les fournisseurs suivants (ou multi-fournisseurs), puis `sorafs_gateway_route_weights` dans le configuration des opérations. (`docs/source/sorafs_gateway_self_cert.md`).
3. Si vous recherchez votre fournisseur, activez la fonction de secours « récupération directe » pour les clients CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**Triage**

- Vérifiez que vous utilisez le jeton de flux `sorafs_gateway_stream_token_limit`.
- Activez la passerelle de connexion TLS ou les options d'admission.
- Sélectionnez `scripts/telemetry/run_schema_diff.sh` pour déterminer la version de la passerelle d'exportation la plus appropriée.

**Варианты ремедиации**

- Перезапускайте только затронутый процесс gateway; N'hésitez pas à utiliser votre clavier, si ce n'est pas un fournisseur.
- Il est possible de limiter le jeton de flux à 10-15 % avant de pouvoir le faire.
- Повторно выполните auto-cert (`scripts/sorafs_gateway_self_cert.sh`) après la stabilisation.

**Après l'incident**

- Formez le poste P1 avec `docs/source/sorafs/postmortem_template.md`.
- Planifiez le travail suivant, si les mesures correctives sont prises dans le cadre de votre projet.

## Всплеск отказов preuve (PoR / PoTR)

**Обнаружение**

- Alertes : `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Tableau de bord : `dashboards/grafana/sorafs_proof_integrity.json`.
- Télémétrie : `torii_sorafs_proof_stream_events_total` et connexion `sorafs.fetch.error` à `provider_reason=corrupt_proof`.**Description spéciale**

1. Заморозьте прием новых manifestes, пометив реестр manifestes (`docs/source/sorafs/manifest_pipeline.md`).
2. Уведомите Governance о приостановке стимулов для затронутых провайдеров.

**Triage**

- Vérifiez le défi PoR de manière automatique `sorafs_node_replication_backlog_total`.
- Validez la preuve de vérification du pipeline (`crates/sorafs_node/src/potr.rs`) pour les déploiements ultérieurs.
- Vérifiez les versions du firmware fournies par les opérateurs de restauration.

**Варианты ремедиации**

- Téléchargez les replays PoR à partir de `sorafs_cli proof stream` avec le manifeste suivant.
- Si les preuves sont stables, incluez le fournisseur actif qui s'occupe de la gouvernance des restaurants et de l'organisateur des tableaux de bord.

**Après l'incident**

- Запустите сценарий хаос-дрилла PoR до следующего продакшен-деплоя.
- Inscrivez-vous dans le courrier postal et consultez la liste de contrôle des qualifications des fournisseurs.

## Задержка репликации / рост backlog

**Обнаружение**

- Alertes : `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. Importer
  `dashboards/alerts/sorafs_capacity_rules.yml` et validez
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  Lors de la promotion, Alertmanager a fourni des informations sur les documents.
- Tableau de bord : `dashboards/grafana/sorafs_capacity_health.json`.
- Paramètres : `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Description spéciale**

1. Réduire le retard accumulé (pour le fournisseur ou le flottement) et préparer les réplications nécessaires.
2. Si l'arriéré est local, effectuez régulièrement de nouvelles commandes auprès d'autres fournisseurs pour les réplications du planificateur.**Triage**

- Vérifiez l'opérateur télémétrique lors de chaque tentative, afin de réduire l'arriéré.
- Réglez la marge disponible (`sorafs_node_capacity_utilisation_percent`).
- Vérifiez les configurations de configuration ultérieures (profil de chunk, preuves de cadence).

**Варианты ремедиации**

- Connectez-vous au `sorafs_cli` avec l'option `--rebalance` pour la configuration du contenu.
- Масштабируйте les travailleurs de réplication горизонтально для затронутого провайдера.
- Lancez l'actualisation du manifeste pour la mise à jour du TTL.

**Après l'incident**

- Planifiez la perceuse de capacité, en vous concentrant sur les fournisseurs concernés.
- Documentez les réplications SLA dans `docs/source/sorafs_node_client_protocol.md`.

## Периодичность хаос-дриллов

- **Ежеквартально** : connexion simultanée de la passerelle + nouvelle tentative de l'opérateur Storm.
- **Два раза в год**: инъекция сбоев PoR/PoTR на 2 провайдерах с восстановлением.
- **Ежемесячная spot-proverka** : сценарий задержки репликации с использованием staging manifests.
- Ajoutez les exercices dans le journal du runbook suivant (`ops/drill-log.md`) ici :

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

- Vérifiez le journal avant les engagements :

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```- Utilisez `--status scheduled` pour les perceuses à main, `pass`/`fail` pour les perceuses à main et `follow-up`, etc. остались открытые действия.
- Sélectionnez la commande `--log` pour le fonctionnement à sec ou les vérifications automatiques ; Sans aucun script, le produit met à jour `ops/drill-log.md`.

## Postmortel de Shablon

Utilisez `docs/source/sorafs/postmortem_template.md` pour les incidents P1/P2 et pour la rétrospective des incidents. Shablon fournit des produits, des colis, des usines, des correctifs et des validations ultérieures.