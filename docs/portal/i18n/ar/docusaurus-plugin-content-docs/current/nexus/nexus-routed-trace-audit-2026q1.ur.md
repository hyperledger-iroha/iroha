---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-routed-trace-audit-2026q1
title: 2026 Q1 routed-trace آڈٹ رپورٹ (B1)
description: `docs/source/nexus_routed_trace_audit_report_2026q1.md` کا آئینہ، جو سہ ماہی ٹیلیمیٹری ریہرسل کے نتائج کا احاطہ کرتا ہے۔
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_routed_trace_audit_report_2026q1.md` کی عکاسی کرتا ہے۔ باقی تراجم آنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# 2026 Q1 Routed-Trace آڈٹ رپورٹ (B1)

روڈ میپ آئٹم **B1 - Routed-Trace Audits & Telemetry Baseline** Nexus routed-trace پروگرام کی سہ ماہی جائزے کا تقاضا کرتا ہے۔ یہ رپورٹ Q1 2026 (جنوری-مارچ) کی آڈٹ ونڈو دستاویز کرتی ہے تاکہ گورننس کونسل Q2 لانچ ریہرسل سے پہلے ٹیلیمیٹری پوزیشن کی منظوری دے سکے۔

## دائرہ کار اور ٹائم لائن

| Trace ID | ونڈو (UTC) | مقصد |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | multi-lane فعال کرنے سے پہلے lane-admission histograms، queue gossip اور alert flow کی تصدیق۔ |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | AND4/AND7 milestones سے پہلے OTLP replay، diff bot parity اور SDK telemetry ingestion کی توثیق۔ |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | RC1 cut سے پہلے governance-approved `iroha_config` deltas اور rollback readiness کی تصدیق۔ |

ہر ریہرسل production-like topology پر routed-trace instrumentation کے ساتھ چلائی گئی (`nexus.audit.outcome` telemetry + Prometheus counters)، Alertmanager rules لوڈ تھے، اور evidence `docs/examples/` میں ایکسپورٹ ہوا۔

## طریقہ کار

1. **ٹیلیمیٹری کلیکشن۔** تمام نوڈز نے structured `nexus.audit.outcome` ایونٹ اور متعلقہ metrics (`nexus_audit_outcome_total*`) emit کیں۔ helper `scripts/telemetry/check_nexus_audit_outcome.py` نے JSON log tail کیا، ایونٹ اسٹیٹس ویلیڈیٹ کیا، اور payload کو `docs/examples/nexus_audit_outcomes/` میں آرکائیو کیا۔ [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **الرٹ ویلیڈیشن۔** `dashboards/alerts/nexus_audit_rules.yml` اور اس کا test harness یہ یقینی بناتے رہے کہ alert noise thresholds اور payload templating مسلسل رہیں۔ CI ہر تبدیلی پر `dashboards/alerts/tests/nexus_audit_rules.test.yml` چلاتا ہے؛ یہی رولز ہر ونڈو میں دستی طور پر بھی چلائے گئے۔
3. **ڈیش بورڈ کیپچر۔** آپریٹرز نے `dashboards/grafana/soranet_sn16_handshake.json` (handshake health) سے routed-trace panels اور telemetry overview dashboards ایکسپورٹ کیے تاکہ queue health کو audit outcomes کے ساتھ correlate کیا جا سکے۔
4. **ریویو نوٹس۔** گورننس سیکرٹری نے reviewer initials، فیصلہ اور mitigation tickets کو [Nexus transition notes](./nexus-transition-notes) اور config delta tracker (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) میں لاگ کیا۔

## نتائج

| Trace ID | نتیجہ | ثبوت | نوٹس |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | Alert fire/recover اسکرین شاٹس (اندرونی لنک) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` replay؛ telemetry diffs [Nexus transition notes](./nexus-transition-notes#quarterly-routed-trace-audit-schedule) میں ریکارڈ۔ | Queue-admission P95 612 ms پر رہا (ہدف <=750 ms)۔ فالو اپ درکار نہیں۔ |
| `TRACE-TELEMETRY-BRIDGE` | Pass | Archived outcome payload `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` اور OTLP replay hash `status.md` میں ریکارڈ۔ | SDK redaction salts Rust baseline سے match تھے؛ diff bot نے zero deltas رپورٹ کیے۔ |
| `TRACE-CONFIG-DELTA` | Pass (mitigation closed) | Governance tracker entry (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS profile manifest (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + telemetry pack manifest (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Q2 rerun نے منظور شدہ TLS profile hash کیا اور zero stragglers کی تصدیق کی؛ telemetry manifest نے slots 912-936 اور workload seed `NEXUS-REH-2026Q2` درج کیا۔ |

تمام traces نے اپنی ونڈوز کے اندر کم از کم ایک `nexus.audit.outcome` ایونٹ پیدا کیا، جس سے Alertmanager guardrails پورے ہوئے (`NexusAuditOutcomeFailure` پورے کوارٹر میں گرین رہا)۔

## Follow-ups

- Routed-trace appendix کو TLS hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` کے ساتھ اپ ڈیٹ کیا گیا؛ mitigation `NEXUS-421` transition notes میں بند کیا گیا۔
- Android AND4/AND7 reviews کے لئے parity evidence مضبوط کرنے کی خاطر raw OTLP replays اور Torii diff artifacts کو آرکائیو کے ساتھ منسلک کرتے رہیں۔
- تصدیق کریں کہ آنے والی `TRACE-MULTILANE-CANARY` rehearsals وہی telemetry helper دوبارہ استعمال کریں تاکہ Q2 sign-off validated workflow سے فائدہ اٹھائے۔

## Artefact انڈیکس

| Asset | مقام |
|-------|----------|
| Telemetry validator | `scripts/telemetry/check_nexus_audit_outcome.py` |
| Alert rules & tests | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| Sample outcome payload | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Config delta tracker | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Routed-trace schedule & notes | [Nexus transition notes](./nexus-transition-notes) |

یہ رپورٹ، اوپر دیے گئے artefacts اور alert/telemetry exports کو governance decision log کے ساتھ منسلک کیا جانا چاہئے تاکہ اس کوارٹر کے لئے B1 بند ہو جائے۔
