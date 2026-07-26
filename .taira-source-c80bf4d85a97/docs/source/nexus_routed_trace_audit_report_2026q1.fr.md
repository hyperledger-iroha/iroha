<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: fr
direction: ltr
source: docs/source/nexus_routed_trace_audit_report_2026q1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9b77d8021c6e09ba132ba080f183b532b35f8f6293a13497646566d13e932306
source_last_modified: "2025-11-22T12:03:01.494516+00:00"
translation_last_reviewed: 2026-01-01
---

# Rapport d'audit Routed-Trace 2026 Q1 (B1)

Le point de roadmap **B1 - Routed-Trace Audits & Telemetry Baseline** exige une
revue trimestrielle du programme routed-trace Nexus. Ce rapport documente la
fenetre d'audit Q1 2026 (janvier-mars) afin que le conseil de gouvernance valide
la posture de telemetrie avant les rehearsals de lancement Q2.

## Portee et calendrier

| Trace ID | Fenetre (UTC) | Objectif |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Verifier les histogrammes d'admission de lanes, le gossip des files et le flux d'alertes avant l'activation multi-lane. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Valider le replay OTLP, la parite du bot de diff et l'ingestion de telemetrie SDK avant les jalons AND4/AND7. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Confirmer les deltas `iroha_config` approuves par gouvernance et la preparation du rollback avant le cut RC1. |

Chaque rehearsal a tourne sur une topologie proche production avec
l'instrumentation routed-trace activee (telemetrie `nexus.audit.outcome` +
compteurs Prometheus), les regles Alertmanager chargees, et les preuves
exportees vers `docs/examples/`.

## Methodologie

1. **Collecte de telemetrie.** Tous les noeuds ont emis l'evenement structure
   `nexus.audit.outcome` et les metriques associees (`nexus_audit_outcome_total*`). Le helper
   `scripts/telemetry/check_nexus_audit_outcome.py` a suivi le log JSON, valide le statut
   d'evenement, et archive la charge utile sous `docs/examples/nexus_audit_outcomes/`
   (`scripts/telemetry/check_nexus_audit_outcome.py:1`).
2. **Validation des alertes.** `dashboards/alerts/nexus_audit_rules.yml` et son harness de tests
   ont garanti que les seuils de bruit et le templating de payload restaient coherents.
   CI execute `dashboards/alerts/tests/nexus_audit_rules.test.yml` a chaque changement; les memes
   regles ont ete exercees manuellement pendant chaque fenetre.
3. **Capture de dashboards.** Les operateurs ont exporte les panneaux routed-trace depuis
   `dashboards/grafana/soranet_sn16_handshake.json` (sante handshake) et les dashboards de
   telemetrie pour correler la sante des files avec les outcomes d'audit.
4. **Notes des reviewers.** La secretaire de gouvernance a consigne les initiales, la decision et
   les tickets de mitigation dans `docs/source/nexus_transition_notes.md` et le tracker de config
   delta (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`).

## Constatations

| Trace ID | Resultat | Evidence | Notes |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | Captures alertes fire/recover (lien interne) + replay de `dashboards/alerts/tests/soranet_lane_rules.test.yml`; diffs de telemetrie consignes dans `docs/source/nexus_transition_notes.md#quarterly-routed-trace-audit-schedule`. | P95 de queue-admission reste a 612 ms (cible <=750 ms). Aucun suivi requis. |
| `TRACE-TELEMETRY-BRIDGE` | Pass | Payload archive `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` plus hash de replay OTLP enregistre dans `status.md`. | Les sels de redaction SDK ont matche la baseline Rust; le bot de diff a rapporte zero delta. |
| `TRACE-CONFIG-DELTA` | Pass (mitigation close) | Entree du tracker gouvernance (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + manifest profil TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + manifest du pack telemetrie (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Le rerun Q2 a hashe le profil TLS approuve et confirme zero stragglers; le manifest telemetrie enregistre la plage de slots 912-936 et la seed workload `NEXUS-REH-2026Q2`. |

Tous les traces ont produit au moins un evenement `nexus.audit.outcome` pendant
leurs fenetres, satisfaisant les guardrails Alertmanager (`NexusAuditOutcomeFailure`
est reste au vert pour le trimestre).

## Suivis

- L'appendice routed-trace a ete mis a jour avec le hash TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`
  (voir `nexus_transition_notes.md`); mitigation `NEXUS-421` close.
- Continuer a attacher les replays OTLP bruts et les artefacts de diff Torii a l'archive pour
  renforcer les preuves de parite pour les revues AND4/AND7.
- Confirmer que les prochains rehearsals `TRACE-MULTILANE-CANARY` reutilisent le meme helper
  telemetrie afin que le sign-off Q2 beneficie du workflow valide.

## Index des artefacts

| Actif | Emplacement |
|-------|----------|
| Validateur de telemetrie | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Regles et tests d'alerte | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Payload outcome exemple | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Tracker de config delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Calendrier et notes routed-trace | `docs/source/nexus_transition_notes.md` |

Ce rapport, les artefacts ci-dessus, et les exports alerte/telemetrie doivent
etre joints au log de decision de gouvernance pour clore B1 sur le trimestre.
