<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ur
direction: rtl
source: docs/source/nexus_routed_trace_audit_report_2026q1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9b77d8021c6e09ba132ba080f183b532b35f8f6293a13497646566d13e932306
source_last_modified: "2025-11-22T12:03:01.494516+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/nexus_routed_trace_audit_report_2026q1.md -->

# 2026 Q1 Routed-Trace آڈٹ رپورٹ (B1)

روڈ میپ آئٹم **B1 — Routed-Trace Audits & Telemetry Baseline** Nexus کے routed-trace پروگرام کی
سہ ماہی جائزہ مانگتا ہے۔ یہ رپورٹ Q1 2026 آڈٹ ونڈو (جنوری-مارچ) کو دستاویز بناتی ہے تاکہ
گورننس کونسل Q2 لانچ ریہرسل سے پہلے ٹیلیمیٹری پوزیشن کی منظوری دے سکے۔

## اسکوپ اور ٹائم لائن

| Trace ID | ونڈو (UTC) | مقصد |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | ملٹی-لین enablement سے پہلے lane-admission histograms، queue gossip اور alert flow کی تصدیق۔ |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | AND4/AND7 milestones سے پہلے OTLP replay، diff bot parity اور SDK telemetry ingestion کی توثیق۔ |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | RC1 cut سے پہلے governance-approved `iroha_config` deltas اور rollback readiness کی تصدیق۔ |

ہر rehearsal پروڈکشن جیسی topography پر چلایا گیا جس میں routed-trace instrumentation فعال تھا
(`nexus.audit.outcome` telemetry + Prometheus counters)، Alertmanager rules لوڈ تھیں، اور
evidence `docs/examples/` میں export کیا گیا تھا۔

## Methodology

1. **Telemetry collection.** تمام nodes نے structured `nexus.audit.outcome` event اور متعلقہ
   metrics (`nexus_audit_outcome_total*`) emit کیں۔ helper
   `scripts/telemetry/check_nexus_audit_outcome.py` نے JSON log کو tail کیا، event status validate
   کیا، اور payload کو `docs/examples/nexus_audit_outcomes/` میں archive کیا
   (`scripts/telemetry/check_nexus_audit_outcome.py:1`).
2. **Alert validation.** `dashboards/alerts/nexus_audit_rules.yml` اور اس کا test harness اس بات کو
   یقینی بناتے ہیں کہ alert noise thresholds اور payload templating consistent رہیں۔ CI ہر تبدیلی
   پر `dashboards/alerts/tests/nexus_audit_rules.test.yml` چلاتا ہے؛ انہی rules کو ہر window میں
   دستی طور پر بھی چلایا گیا۔
3. **Dashboard capture.** Operators نے routed-trace panels کو
   `dashboards/grafana/soranet_sn16_handshake.json` (handshake health) سے export کیا اور telemetry
   overview dashboards کے ساتھ queue health اور audit outcomes کو correlate کیا۔
4. **Reviewer notes.** Governance secretary نے reviewer initials، decision اور mitigation tickets
   کو `docs/source/nexus_transition_notes.md` اور config delta tracker
   (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) میں log کیا۔

## Findings

| Trace ID | Outcome | Evidence | Notes |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | Alert fire/recover screenshots (internal link) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` replay; telemetry diffs `docs/source/nexus_transition_notes.md#quarterly-routed-trace-audit-schedule` میں درج۔ | Queue-admission P95 612 ms رہا (target <=750 ms). Follow-up درکار نہیں۔ |
| `TRACE-TELEMETRY-BRIDGE` | Pass | Archived outcome payload `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` اور OTLP replay hash جو `status.md` میں درج ہے۔ | SDK redaction salts Rust baseline سے match ہوئے؛ diff bot نے zero deltas رپورٹ کیے۔ |
| `TRACE-CONFIG-DELTA` | Pass (mitigation closed) | Governance tracker entry (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS profile manifest (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + telemetry pack manifest (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Q2 rerun نے approved TLS profile hash کیا اور zero stragglers کی تصدیق کی؛ telemetry manifest slot range 912–936 اور workload seed `NEXUS-REH-2026Q2` ریکارڈ کرتا ہے۔ |

تمام traces نے اپنی windows میں کم از کم ایک `nexus.audit.outcome` event پیدا کیا،
جس سے Alertmanager guardrails پورے ہوئے (`NexusAuditOutcomeFailure` پورے quarter میں سبز رہا)۔

## Follow-ups

- Routed-trace appendix کو TLS hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`
  کے ساتھ اپ ڈیٹ کیا گیا (دیکھیں `nexus_transition_notes.md`); mitigation `NEXUS-421` بند۔
- Raw OTLP replays اور Torii diff artefacts کو archive کے ساتھ attach کرنا جاری رکھیں تاکہ
  AND4/AND7 reviews کے لئے parity evidence مضبوط ہو۔
- تصدیق کریں کہ آنے والی `TRACE-MULTILANE-CANARY` rehearsals وہی telemetry helper دوبارہ استعمال
  کریں تاکہ Q2 sign-off validated workflow سے فائدہ اٹھائے۔

## Artefact Index

| Asset | Location |
|-------|----------|
| Telemetry validator | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Alert rules & tests | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Sample outcome payload | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Config delta tracker | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Routed-trace schedule & notes | `docs/source/nexus_transition_notes.md` |

یہ رپورٹ، اوپر درج artefacts، اور alert/telemetry exports کو گورننس decision log کے ساتھ
منسلک کیا جانا چاہئے تاکہ quarter کے لئے B1 بند ہو سکے۔

</div>
