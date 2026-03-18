---
lang: ur
direction: rtl
source: docs/source/nexus_telemetry_remediation_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 19d46f99e2ba79c56cbc3af65b47f5fb6997fa66f8ee951806b21696418a1d7b
source_last_modified: "2025-11-27T14:13:33.645951+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/nexus_telemetry_remediation_plan.md -->

% Nexus ٹیلیمیٹری ریمیڈی ایشن پلان (Phase B2)

# جائزہ

روڈ میپ آئٹم **B2 - telemetry gap ownership** ایک شائع شدہ منصوبہ چاہتا ہے جو ہر باقی رہنے والی
Nexus ٹیلیمیٹری گیپ کو سگنل، الرٹ guardrail، ذمہ دار، آخری تاریخ، اور ویریفیکیشن آرٹیفیکٹ سے جوڑے
تاکہ Q1 2026 آڈٹ ونڈوز شروع ہونے سے پہلے سب کچھ واضح ہو۔ یہ دستاویز اس میٹرکس کو مرکزی بناتی ہے
تاکہ release engineering، telemetry ops اور SDK owners routed-trace اور `TRACE-TELEMETRY-BRIDGE`
ریہرسل سے پہلے کوریج کی تصدیق کر سکیں۔

# گیپ میٹرکس

| Gap ID | سگنل اور الرٹ guardrail | Owner / Escalation | ڈیڈ لائن (UTC) | ثبوت اور تصدیق |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | `torii_lane_admission_latency_seconds{lane_id,endpoint}` ہسٹوگرام اور الرٹ **`SoranetLaneAdmissionLatencyDegraded`** جو اس وقت فائر ہوتا ہے جب `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` پانچ منٹ تک برقرار رہے (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (سگنل) + `@telemetry-ops` (الرٹ) - Nexus routed-trace on-call کے ذریعے escalation. | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` میں الرٹ ٹیسٹس اور `TRACE-LANE-ROUTING` ریہرسل کی کیپچر جس میں الرٹ کا فائر/ریکور دکھایا گیا، ساتھ Torii `/metrics` scrape جو `docs/source/nexus_transition_notes.md` میں آرکائیو ہے۔ |
| `GAP-TELEM-002` | کاؤنٹر `nexus_config_diff_total{knob,profile}` اور guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` جو ڈیپلائیز کو گیٹ کرتا ہے (`docs/source/telemetry.md`). | `@nexus-core` (instrumentation) -> `@telemetry-ops` (الرٹ) - جب کاؤنٹر غیر متوقع طور پر بڑھے تو governance duty officer کو پیج کیا جاتا ہے۔ | 2026-02-26 | governance dry-run کی آؤٹ پٹس `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` کے ساتھ محفوظ ہیں؛ ریلیز چیک لسٹ میں Prometheus query اسکرین شاٹ اور لاگ اقتباس شامل ہے جو `StateTelemetry::record_nexus_config_diff` کے diff emit کرنے کو ثابت کرتا ہے۔ |
| `GAP-TELEM-003` | ایونٹ `TelemetryEvent::AuditOutcome` (میٹرک `nexus.audit.outcome`) اور الرٹ **`NexusAuditOutcomeFailure`** جب failures یا missing outcomes 30 منٹ سے زیادہ رہیں (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) کے ساتھ escalation to `@sec-observability`. | 2026-02-27 | CI gate `scripts/telemetry/check_nexus_audit_outcome.py` NDJSON payloads آرکائیو کرتا ہے اور جب TRACE ونڈو میں success ایونٹ نہ ہو تو fail ہوتا ہے؛ الرٹ اسکرین شاٹس routed-trace رپورٹ کے ساتھ منسلک ہیں۔ |
| `GAP-TELEM-004` | گیج `nexus_lane_configured_total` guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` کے ساتھ مانیٹر ہوتا ہے (دستاویز `docs/source/telemetry.md`) اور SRE on-call چیک لسٹ میں جاتا ہے۔ | `@telemetry-ops` (gauge/export) کے ساتھ escalation to `@nexus-core` جب نوڈز متضاد catalog sizes رپورٹ کریں۔ | 2026-02-28 | scheduler ٹیلیمیٹری ٹیسٹ `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` emission ثابت کرتا ہے؛ operators Prometheus diff اور `StateTelemetry::set_nexus_catalogs` لاگ اقتباس کو TRACE ریہرسل پیکج میں شامل کرتے ہیں۔ |

# Export بجٹ اور OTLP حدود

- **فیصلہ (2026-02-11):** OTLP exporters کو **5 MiB/min فی نوڈ** یا **25,000 spans/min** (جو کم ہو)
  تک محدود کرنا، 256 spans batch size اور 10 second export timeout کے ساتھ۔ حد کے 80% سے اوپر
  exports `dashboards/alerts/nexus_telemetry_rules.yml` میں `NexusOtelExporterSaturated` الرٹ
  ٹرگر کرتے ہیں اور audit logs کے لئے `telemetry_export_budget_saturation` ایونٹ emit کرتے ہیں۔
- **Enforcement:** Prometheus rules `iroha.telemetry.export.bytes_total` اور
  `iroha.telemetry.export.spans_total` counters پڑھتے ہیں؛ OTLP collector profile انہی limits کو
  default کے طور پر ship کرتا ہے، اور node configs انہیں governance waiver کے بغیر بڑھا نہیں سکتیں۔
- **Evidence:** alert test vectors اور منظور شدہ limits `docs/source/nexus_transition_notes.md` میں
  routed-trace audit artefacts کے ساتھ آرکائیو ہیں۔ B2 acceptance اب export budget کو بند مانتی ہے۔

# Operational Workflow

1. **Weekly triage.** Owners Nexus readiness call میں progress رپورٹ کرتے ہیں؛ blockers اور alert-test
   artefacts `status.md` میں لاگ ہوتے ہیں۔
2. **Alert dry-runs.** ہر alert rule کے ساتھ `dashboards/alerts/tests/*.test.yml` انٹری آتی ہے تاکہ
   guardrail بدلنے پر CI `promtool test rules` چلائے۔
3. **Audit evidence.** `TRACE-LANE-ROUTING` اور `TRACE-TELEMETRY-BRIDGE` ریہرسل کے دوران on-call
   Prometheus query results، alert history اور متعلقہ script outputs (`scripts/telemetry/check_nexus_audit_outcome.py`,
   `scripts/telemetry/check_redaction_status.py` correlated signals کے لئے) capture کرتا ہے اور
   routed-trace artefacts کے ساتھ محفوظ کرتا ہے۔
4. **Escalation.** اگر guardrail کسی rehearsed window کے باہر فائر ہو تو owning team اس پلان کا حوالہ
   دیتے ہوئے Nexus incident ticket کھولتی ہے، جس میں metric snapshot اور mitigation steps شامل
   ہوتے ہیں، پھر audits دوبارہ شروع ہوتے ہیں۔

اس میٹرکس کی اشاعت اور `roadmap.md` و `status.md` سے حوالہ دینے کے بعد، روڈ میپ آئٹم **B2** اب
"ذمہ داری، ڈیڈ لائن، الرٹ، ویریفیکیشن" کے acceptance criteria پر پورا اترتا ہے۔

</div>
