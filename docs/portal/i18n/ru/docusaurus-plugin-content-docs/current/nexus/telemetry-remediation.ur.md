---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-telemetry-remediation
title: Nexus ٹیلیمیٹری ریمیڈی ایشن پلان (B2)
description: `docs/source/nexus_telemetry_remediation_plan.md` کا آئینہ، جو ٹیلیمیٹری گیپ میٹرکس اور آپریشنل ورک فلو دستاویز کرتا ہے۔
---

# جائزہ

روڈ میپ آئٹم **B2 - ٹیلیمیٹری گیپس کی ملکیت** ایک شائع شدہ پلان کا تقاضا کرتا ہے جو Nexus کی ہر زیر التواء ٹیلیمیٹری گیپ کو سگنل، الرٹ گارڈ ریل، اونر، ڈیڈ لائن اور ویریفیکیشن آرٹیفیکٹ سے جوڑے، تاکہ Q1 2026 کی آڈٹ ونڈوز شروع ہونے سے پہلے سب کچھ واضح ہو۔ یہ صفحہ `docs/source/nexus_telemetry_remediation_plan.md` کی عکاسی کرتا ہے تاکہ release engineering، telemetry ops اور SDK اونرز routed-trace اور `TRACE-TELEMETRY-BRIDGE` کی ریہرسل سے پہلے کوریج کی تصدیق کر سکیں۔

# گیپ میٹرکس

| گیپ ID | سگنل اور الرٹ گارڈ ریل | اونر / اسکیلشن | ڈیڈ لائن (UTC) | ثبوت اور ویریفیکیشن |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | ہسٹوگرام `torii_lane_admission_latency_seconds{lane_id,endpoint}` اور الرٹ **`SoranetLaneAdmissionLatencyDegraded`** جو اس وقت فائر ہوتا ہے جب `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` پانچ منٹ تک برقرار رہے (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (سگنل) + `@telemetry-ops` (الرٹ)؛ Nexus routed-trace on-call کے ذریعے اسکیلشن۔ | 2026-02-23 | الرٹ ٹیسٹس `dashboards/alerts/tests/soranet_lane_rules.test.yml` کے تحت، ساتھ `TRACE-LANE-ROUTING` ریہرسل کی کیپچر جس میں الرٹ فائر/ریکور دکھایا گیا ہو اور Torii `/metrics` اسکریپ [Nexus transition notes](./nexus-transition-notes) میں محفوظ ہو۔ |
| `GAP-TELEM-002` | کاؤنٹر `nexus_config_diff_total{knob,profile}` اور گارڈ ریل `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` جو ڈپلائز کو گیٹ کرتا ہے (`docs/source/telemetry.md`). | `@nexus-core` (انسٹرومنٹیشن) -> `@telemetry-ops` (الرٹ)؛ غیر متوقع بڑھوتری پر گورننس ڈیوٹی آفیسر پیج ہوتا ہے۔ | 2026-02-26 | گورننس dry-run آؤٹ پٹس `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` کے ساتھ محفوظ؛ ریلیز چیک لسٹ میں Prometheus کوئری اسکرین شاٹ اور `StateTelemetry::record_nexus_config_diff` کے diff emit کرنے کا لاگ اقتباس شامل ہو۔ |
| `GAP-TELEM-003` | ایونٹ `TelemetryEvent::AuditOutcome` (میٹرک `nexus.audit.outcome`) اور الرٹ **`NexusAuditOutcomeFailure`** جب failures یا missing outcomes 30 منٹ سے زیادہ برقرار رہیں (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) اور اسکیلشن `@sec-observability` تک۔ | 2026-02-27 | CI گیٹ `scripts/telemetry/check_nexus_audit_outcome.py` NDJSON payloads محفوظ کرتا ہے اور اگر TRACE ونڈو میں کامیابی کا ایونٹ نہ ہو تو فیل ہو جاتا ہے؛ الرٹ اسکرین شاٹس routed-trace رپورٹ کے ساتھ منسلک ہوں۔ |
| `GAP-TELEM-004` | گیج `nexus_lane_configured_total` اور گارڈ ریل `nexus_lane_configured_total != EXPECTED_LANE_COUNT` جو SRE on-call چیک لسٹ کو فیڈ کرتا ہے۔ | `@telemetry-ops` (gauge/export) اور اسکیلشن `@nexus-core` کی طرف جب نوڈز کیٹلاگ سائز میں عدم مطابقت رپورٹ کریں۔ | 2026-02-28 | scheduler ٹیلیمیٹری ٹیسٹ `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` ایمیشن ثابت کرتا ہے؛ آپریٹرز Prometheus diff + `StateTelemetry::set_nexus_catalogs` لاگ اقتباس TRACE ریہرسل پیکیج کے ساتھ جوڑتے ہیں۔ |

# آپریشنل ورک فلو

1. **ہفتہ وار ٹریاج۔** اونرز Nexus readiness کال میں پیش رفت رپورٹ کرتے ہیں؛ blockers اور الرٹ ٹیسٹ آرٹیفیکٹس `status.md` میں درج ہوتے ہیں۔
2. **الرٹ dry-run۔** ہر الرٹ رول کے ساتھ `dashboards/alerts/tests/*.test.yml` انٹری دی جاتی ہے تاکہ guardrail بدلنے پر CI `promtool test rules` چلائے۔
3. **آڈٹ ایویڈنس۔** `TRACE-LANE-ROUTING` اور `TRACE-TELEMETRY-BRIDGE` ریہرسل کے دوران آن کال Prometheus کوئریز کے نتائج، الرٹ ہسٹری، اور متعلقہ اسکرپٹس کی آؤٹ پٹس (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` برائے correlated signals) جمع کرتا ہے اور routed-trace آرٹیفیکٹس کے ساتھ محفوظ کرتا ہے۔
4. **اسکیلشن۔** اگر کوئی guardrail ریہرسل ونڈو کے باہر فائر ہو تو متعلقہ ٹیم اس پلان کے حوالہ سے Nexus incident ٹکٹ فائل کرتی ہے، میٹرک اسنیپ شاٹ اور mitigation steps شامل کر کے آڈٹس دوبارہ شروع کرتی ہے۔

اس میٹرکس کی اشاعت - اور `roadmap.md` و `status.md` میں حوالہ دینے کے ساتھ - روڈ میپ آئٹم **B2** اب "ذمہ داری، ڈیڈ لائن، الرٹ، ویریفیکیشن" کی قبولیت معیار پر پورا اترتا ہے۔
