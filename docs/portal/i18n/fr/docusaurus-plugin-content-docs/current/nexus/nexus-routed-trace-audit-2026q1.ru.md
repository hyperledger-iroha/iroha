---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-routed-trace-audit-2026q1
titre : Отчет аудита routed-trace pour le premier trimestre 2026 (B1)
description : Zerkalo `docs/source/nexus_routed_trace_audit_report_2026q1.md`, охватывающее итоги квартальных репетиций телеметрии.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note Канонический источник
Cette page correspond à `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Il est possible de copier des copies de synchronisation, mais il n'y a aucun moyen de le faire avant.
:::

# Ouvrir l'audit Routed-Trace pour le premier trimestre 2026 (B1)

Feuille de route **B1 - Routed-Trace Audits & Telemetry Baseline** требует квартального обзора программы routed-trace Nexus. Ceci est prévu pour l'audit du premier trimestre 2026 (Yangwar-Mart), qui devrait permettre de mettre en place un système de télémétrie avant le début de l'année. répétition du Q2.

## Область и таймлайн

| Identifiant de trace | Okno (UTC) | Цель |
|--------------|--------------|---------------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Vérifiez l'historique des informations sur la voie, les potins et les alertes électriques avant l'ouverture de plusieurs voies. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Permet de valider la relecture OTLP, de partager le bot de comparaison et d'utiliser le SDK télémétrique pour tous les AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12h00-12h30 | Mettez à jour les deltas de gouvernance `iroha_config` et passez à la restauration avant l'adoption de RC1. |Cette méthode de répétition pour la topologie du produit avec un instrument de traçage acheminé complet (télémétrie `nexus.audit.outcome` + fiches) Prometheus), téléchargez Alertmanager et les documents d'exportation dans `docs/examples/`.

## Métologie

1. **Сбор телеметрии.** Vous utilisez des mesures structurelles émises par `nexus.audit.outcome` et des mesures pertinentes (`nexus_audit_outcome_total*`). L'assistant `scripts/telemetry/check_nexus_audit_outcome.py` a ajouté le journal JSON, le statut validé et la charge utile archivée dans `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **Alertes signalées.** `dashboards/alerts/nexus_audit_rules.yml` et ce harnais de test ont permis de stabiliser la charge utile des véhicules et des câbles. CI a mis `dashboards/alerts/tests/nexus_audit_rules.test.yml` pour toutes les modifications ; Vous serez programmé à chaque instant.
3. **Съемка дашбордов.** Les opérateurs exportateurs de panneaux routed-trace avec `dashboards/grafana/soranet_sn16_handshake.json` (poignée de main) et les téléphones de bord, ce qui est связать здоровье очередей с результатами аудита.
4. **Avis des rapports.** Le secrétaire pour la mise en œuvre des rapports, des résolutions et des tickets d'atténuation dans la [transition Nexus notes](./nexus-transition-notes) et la configuration du treker (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Résultats| Identifiant de trace | Cet article | Доказательства | Première |
|----------|---------|--------------|-------|
| `TRACE-LANE-ROUTING` | Passer | Скриншоты fire/recover алертов (внутренняя ссылка) + replay `dashboards/alerts/tests/soranet_lane_rules.test.yml` ; Les informations télémétriques sont fournies dans [Notes de transition Nexus] (./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | P95 приема очереди остался 612 ms (цель <=750 ms). Les deux projets ne sont pas difficiles. |
| `TRACE-TELEMETRY-BRIDGE` | Passer | Charge utile archivée `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` et hachage de relecture OTLP, téléchargée dans `status.md`. | Sels de rédaction du SDK comparés à la ligne de base de Rust ; diff bot сообщил ноль дельт. |
| `TRACE-CONFIG-DELTA` | Pass (atténuation fermée) | Téléchargez le module de gouvernance (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + le manifeste du profil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + le manifeste du pack de télémétrie (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | La réexécution du deuxième trimestre a permis de mettre à jour le profil TLS actuel et de mettre à jour les résultats suivants ; Le manifeste de télémétrie intègre les emplacements de dialogue 912-936 et la graine de charge de travail `NEXUS-REH-2026Q2`. |

Vous avez trouvé des traces en utilisant `nexus.audit.outcome` dans votre ordinateur, qui a utilisé les garde-corps Alertmanager (`NexusAuditOutcomeFailure` pour votre espace).

## Suivis- Ajout de routed-trace обновлено TLS хэшем `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` ; atténuation `NEXUS-421` закрыт в notes de transition.
- Si vous souhaitez utiliser les replays OTLP et les artéfacts Torii diff dans les archives, vous devez utiliser les documents réservés pour la vérification d'Android AND4/AND7.
- Pour que la répétition de pré-répétition `TRACE-MULTILANE-CANARY` utilise l'assistant télémétrique, la signature Q2 fonctionne dans le flux de travail éprouvé.

## Indices des artéfacts

| Actif | Localisation |
|-------|--------------|
| Validateurs télémétriques | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Alertes alertes et tests | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Exemple de charge utile de résultat | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Трекер конфиг-дельт | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Graphiques et paramètres routé-trace | [Notes de transition Nexus](./nexus-transition-notes) |

Il s'ensuit que les articles et les alertes/télémétries d'exportation doivent être utilisés dans le cadre du calendrier de réforme de la gouvernance, pour que le B1 soit mis en place pour le commerce.