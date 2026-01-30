---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-routed-trace-audit-2026q1
title: Rapport d'audit routed-trace 2026 Q1 (B1)
description: Miroir de `docs/source/nexus_routed_trace_audit_report_2026q1.md`, couvrant les resultats trimestriels des repetitions de telemetrie.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note Source canonique
Cette page reflete `docs/source/nexus_routed_trace_audit_report_2026q1.md`. Gardez les deux copies alignees jusqu'a ce que les traductions restantes arrivent.
:::

# Rapport d'audit Routed-Trace 2026 Q1 (B1)

L'item de roadmap **B1 - Routed-Trace Audits & Telemetry Baseline** exige une revue trimestrielle du programme routed-trace Nexus. Ce rapport documente la fenetre d'audit Q1 2026 (janvier-mars) afin que le conseil de gouvernance puisse valider la posture de telemetrie avant les repetitions de lancement Q2.

## Portee et calendrier

| Trace ID | Fenetre (UTC) | Objectif |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Verifier les histogrammes d'admission des lanes, le gossip des files et le flux d'alertes avant l'activation multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Valider le replay OTLP, la parite du diff bot et l'ingestion de telemetrie SDK avant les jalons AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Confirmer les deltas `iroha_config` approuves par la gouvernance et la preparation au rollback avant le cut RC1. |

Chaque repetition a tourne sur une topologie proche production avec l'instrumentation routed-trace activee (telemetrie `nexus.audit.outcome` + compteurs Prometheus), les regles Alertmanager chargees et des preuves exportees dans `docs/examples/`.

## Methodologie

1. **Collecte de telemetrie.** Tous les noeuds ont emis l'evenement structure `nexus.audit.outcome` et les metriques associees (`nexus_audit_outcome_total*`). Le helper `scripts/telemetry/check_nexus_audit_outcome.py` a suivi le log JSON, valide le statut de l'evenement et archive le payload sous `docs/examples/nexus_audit_outcomes/`. [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **Validation des alertes.** `dashboards/alerts/nexus_audit_rules.yml` et son harness de test ont assure que les seuils de bruit et le templating des payloads restent coherents. CI execute `dashboards/alerts/tests/nexus_audit_rules.test.yml` a chaque modification; les memes regles ont ete exercees manuellement pendant chaque fenetre.
3. **Capture des dashboards.** Les operateurs ont exporte les panneaux routed-trace depuis `dashboards/grafana/soranet_sn16_handshake.json` (sante handshake) et les dashboards de telemetrie globale pour correler la sante des files avec les resultats d'audit.
4. **Notes des relecteurs.** La secretaire de gouvernance a consigne les initiales, la decision et les tickets de mitigation dans [Nexus transition notes](./nexus-transition-notes) et dans le tracker de deltas de config (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Constatations

| Trace ID | Resultat | Preuves | Notes |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | Captures alerte fire/recover (lien interne) + replay `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diffs de telemetrie enregistres dans [Nexus transition notes](./nexus-transition-notes#quarterly-routed-trace-audit-schedule). | Le P95 d'admission de file est reste a 612 ms (cible <=750 ms). Aucun suivi requis. |
| `TRACE-TELEMETRY-BRIDGE` | Pass | Payload archive `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` plus hash de replay OTLP enregistre dans `status.md`. | Les sels de redaction SDK correspondaient a la base Rust; le diff bot a signale zero delta. |
| `TRACE-CONFIG-DELTA` | Pass (mitigation closed) | Entree du tracker de gouvernance (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifest profil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifest du pack telemetrie (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | La rerun Q2 a hashe le profil TLS approuve et confirme zero retardataires; le manifest telemetrie enregistre la plage de slots 912-936 et le workload seed `NEXUS-REH-2026Q2`. |

Tous les traces ont produit au moins un evenement `nexus.audit.outcome` dans leurs fenetres, satisfaisant les guardrails Alertmanager (`NexusAuditOutcomeFailure` est reste vert sur le trimestre).

## Suivis

- L'appendice routed-trace a ete mis a jour avec le hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; la mitigation `NEXUS-421` est fermee dans les transition notes.
- Continuer a joindre les replays OTLP bruts et les artefacts de diff Torii a l'archive pour renforcer la preuve de parite pour les revues Android AND4/AND7.
- Confirmer que les prochaines rehearsals `TRACE-MULTILANE-CANARY` reutilisent le meme helper de telemetrie pour que la validation Q2 beneficie du workflow valide.

## Index des artefacts

| Asset | Emplacement |
|-------|----------|
| Validateur de telemetrie | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Regles et tests d'alerte | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Exemple de payload outcome | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker des deltas de config | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Planning et notes routed-trace | [Nexus transition notes](./nexus-transition-notes) |

Ce rapport, les artefacts ci-dessus et les exports d'alertes/telemetrie doivent etre attaches au journal de decision de gouvernance pour cloturer B1 du trimestre.
