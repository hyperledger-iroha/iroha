---
lang: he
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-telemetry-remediation
כותרת: Plan de remediation de telemetrie Nexus (B2)
תיאור: Miroir de `docs/source/nexus_telemetry_remediation_plan.md`, מתעד La matrice des ecarts de telemetrie et le flux operationnel.
---

# Vue d'ensemble

L'element de roadmap **B2 - בעלות des ecarts de telemetrie** exige un plan publie reliant chaque ecart de telemetrie restant de Nexus a un signal, un signal, un guardian d'alerte, un proprietaire, une date limite et un artefact de verification de avant le d'20 pages. reflete `docs/source/nexus_telemetry_remediation_plan.md` אפינ que שחרור הנדסה, טלמטריה פעולות ו-les בעלי SDK puissent confirmer la couverture avant les repetitions routed-trace et `TRACE-TELEMETRY-BRIDGE`.

# Matrice des ecarts

| ID d'ecart | אות ומעקה בטיחות | קניין / הסלמה | Echeance (UTC) | Preuves et Verification |
|--------|------------------------|------------------------|-----------|------------------------|
| `GAP-TELEM-001` | היסטוגרמה `torii_lane_admission_latency_seconds{lane_id,endpoint}` avec l'alerte **`SoranetLaneAdmissionLatencyDegraded`** declenchee quand `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` תליון 5 דקות (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (אות) + `@telemetry-ops` (תראה); הסלמה דרך l'on-call routed-trace Nexus. | 23-02-2026 | בדיקות d'alerte sous `dashboards/alerts/tests/soranet_lane_rules.test.yml` פלוס la capture de la repetition `TRACE-LANE-ROUTING` montrant l'alerte declenchee/retablie et le scrape Torii `/metrics` archive dans [00100NT transition הערות](./nexus-transition-notes). |
| `GAP-TELEM-002` | Compteur `nexus_config_diff_total{knob,profile}` avec מעקה בטיחות `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` bloquant les deploiements (`docs/source/telemetry.md`). | `@nexus-core` (מכשור) -> `@telemetry-ops` (תראה); l'officier de garde governance est page lorsque le compteur augmente de maniere inattendue. | 2026-02-26 | Sorties de dry-run de governance stockees a cote de `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; רשימת השחרור כוללת ללכוד את הבקשה Prometheus פלוס תוספת יומני פרובנט que `StateTelemetry::record_nexus_config_diff` a emis le diff. |
| `GAP-TELEM-003` | Evenement `TelemetryEvent::AuditOutcome` (metrique `nexus.audit.outcome`) avec l'alerte **`NexusAuditOutcomeFailure`** lorsque des echecs ou des outcomes manquants persistent>30 דקות (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (צינור) avec escalade לעומת `@sec-observability`. | 2026-02-27 | La gate CI `scripts/telemetry/check_nexus_audit_outcome.py` archive des payloads NDJSON et echoue lorsqu'une fenetre TRACE ne contient pas d'evenement de success; לוכדת d'alertes jointes au rapport מנותב-עקבות. |
| `GAP-TELEM-004` | מד `nexus_lane_configured_total` avec מעקה בטיחות `nexus_lane_configured_total != EXPECTED_LANE_COUNT` alimentant la checklist SRE בכוננות. | `@telemetry-ops` (מד/ייצוא) avec escalade versus `@nexus-core` lorsque les noeuds signalent des tails de catalog incoherentes. | 2026-02-28 | Le test de telemetrie du scheduler `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` prouve l'emission; les operators joignent un diff Prometheus + extrait de log `StateTelemetry::set_nexus_catalogs` au package de repetition TRACE. |

# תפעול שטף1. **Triage hebdomadaire.** Les owners rapportent l'avancement lors de l'appel de readiness Nexus; les blocages et artefacts de tests d'alerte sont consignes dans `status.md`.
2. **מבחנים ד'התראה.** Chaque regle d'alerte est livree avec une entree `dashboards/alerts/tests/*.test.yml` afin que CI execute `promtool test rules` quand le מעקה בטיחות evolution.
3. **Preuves d'audit.** תליון חזרות `TRACE-LANE-ROUTING` et `TRACE-TELEMETRY-BRIDGE`, לכידת תוצאות בהזמנה לתוצאות בקשות Prometheus, היסטוריית התראות וסיורים של סקריפטים (I02NI00,02NI `scripts/telemetry/check_redaction_status.py` pour les signaux correles) et les stocke avec les artefacts routed-trace.
4. **הסלמה.** זו מעקה בטיחות חוזר ונשנה, ציוד פרטי או כרטיס אירוע Nexus בהתייחס לתוכנית ce, הכוללת תמונת מצב של מטריקה ו-les etapes de mitigation les avant de rep.

Avec cette matrice publiee - et referencee depuis `roadmap.md` et `status.md` - L'item de roadmap **B2** satisfait maintenant aux criteres d'acceptation "אחריות, echeance, alerte, אימות".