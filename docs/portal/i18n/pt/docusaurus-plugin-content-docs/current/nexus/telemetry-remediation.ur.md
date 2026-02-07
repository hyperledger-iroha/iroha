---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-telemetria-remediação
título: Nexus ٹیلیمیٹری ریمیڈی ایشن پلان (B2)
description: `docs/source/nexus_telemetry_remediation_plan.md` ہے۔
---

# جائزہ

روڈ میپ آئٹم **B2 - ٹیلیمیٹری گیپس کی ملکیت** ایک شائع شدہ پلان کا تقاضا کرتا ہے جو Nexus کی ہر زیر التواء ٹیلیمیٹری گیپ کو سگنل, الرٹ گارڈ ریل, اونر, ڈیڈ لائن اور ویریفیکیشن آرٹیفیکٹ سے جوڑے, تاکہ Q1 2026 کی آڈٹ ونڈوز شروع ہونے سے پہلے سب کچھ واضح ہو۔ O `docs/source/nexus_telemetry_remediation_plan.md` é um software de engenharia de liberação, operações de telemetria, SDK, rastreamento roteado e `TRACE-TELEMETRY-BRIDGE`. ریہرسل سے پہلے کوریج کی تصدیق کر سکیں۔

# گیپ میٹرکس

| گیپ ID | سگنل اور الرٹ گارڈ ریل | اونر / اسکیلشن | Horário de funcionamento (UTC) | ثبوت اور ویریفیکیشن |
|----|-------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | ہسٹوگرام `torii_lane_admission_latency_seconds{lane_id,endpoint}` اور الرٹ **`SoranetLaneAdmissionLatencyDegraded`** جو اس وقت فائر ہوتا ہے جب `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` پانچ منٹ Verifique o valor (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (سگنل) + `@telemetry-ops` (الرٹ)؛ Nexus routed-trace on-call کے ذریعے اسکیلشن۔ | 23/02/2026 | O cartão `dashboards/alerts/tests/soranet_lane_rules.test.yml` é um cartão de crédito `TRACE-LANE-ROUTING`. فائر/ریکور دکھایا گیا ہو اور Torii `/metrics` اسکریپ [Nexus notas de transição](./nexus-transition-notes) میں محفوظ ہو۔ |
| `GAP-TELEM-002` | O cartão `nexus_config_diff_total{knob,profile}` é o cartão `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` ou o cartão de crédito (`docs/source/telemetry.md`). | `@nexus-core` (انسٹرومنٹیشن) -> `@telemetry-ops` (الرٹ)؛ غیر متوقع بڑھوتری پر گورننس ڈیوٹی آفیسر پیج ہوتا ہے۔ | 26/02/2026 | گورننس simulação de teste `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` کے ساتھ محفوظ؛ ریلیز چیک لسٹ میں Prometheus کوئری اسکرین شاٹ اور `StateTelemetry::record_nexus_config_diff` کے diff emit کرنے کا لاگ اقتباس شامل ہو۔ |
| `GAP-TELEM-003` | ایونٹ `TelemetryEvent::AuditOutcome` (میٹرک `nexus.audit.outcome`) اور الرٹ **`NexusAuditOutcomeFailure`** جب falhas یا resultados ausentes 30 meses atrás رہیں (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) e `@sec-observability` تک۔ | 27/02/2026 | CI گیٹ `scripts/telemetry/check_nexus_audit_outcome.py` NDJSON payloads محفوظ کرتا ہے اور اگر TRACE ونڈو میں کامیابی کا ایونٹ نہ ہو تو فیل ہو جاتا ہے؛ O código de rastreamento roteado é um recurso de rastreamento roteado |
| `GAP-TELEM-004` | گیج `nexus_lane_configured_total` اور گارڈ ریل `nexus_lane_configured_total != EXPECTED_LANE_COUNT` ou SRE de plantão چیک لسٹ کو فیڈ کرتا ہے۔ | `@telemetry-ops` (manômetro/exportação) اور اسکیلشن `@nexus-core` کی طرف جب نوڈز کیٹلاگ سائز میں عدم مطابقت رپورٹ کریں۔ | 28/02/2026 | agendador ٹیلیمیٹری ٹیسٹ `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` ایمیشن ثابت کرتا ہے؛ آپریٹرز Prometheus diff + `StateTelemetry::set_nexus_catalogs` لاگ اقتباس TRACE ریہرسل پیکیج کے ساتھ جوڑتے ہیں۔ |

# آپریشنل ورک فلو

1. **ہفتہ وار ٹریاج۔** اونرز Nexus prontidão کال میں پیش رفت رپورٹ کرتے ہیں؛ bloqueadores اور الرٹ ٹیسٹ آرٹیفیکٹس `status.md` میں درج ہوتے ہیں۔
2. **الرٹ dry-run۔** ہر الرٹ رول کے ساتھ `dashboards/alerts/tests/*.test.yml` انٹری دی جاتی ہے تاکہ guardrail بدلنے پر CI `promtool test rules` چلائے۔
3. **آڈٹ ایویڈنس۔** `TRACE-LANE-ROUTING` e `TRACE-TELEMETRY-BRIDGE` ریہرسل کے دوران آن کال Prometheus کوئریز کے نتائج, الرٹ ہسٹری, اور متعلقہ اسکرپٹس کی آؤٹ پٹس (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` برائے sinais correlacionados) Como usar o routed-trace آرٹیفیکٹس کے ساتھ محفوظ کرتا ہے۔
4. **اسکیلشن۔** اگر کوئی guardrail ریہرسل ونڈو کے باہر فائر ہو تو متعلقہ ٹیم اس پلان کے حوالہ O incidente Nexus é definido como um procedimento de mitigação que pode ser feito com etapas de mitigação. شروع کرتی ہے۔

اس میٹرکس کی اشاعت - اور `roadmap.md` e `status.md` میں حوالہ دینے کے ساتھ - روڈ میپ آئٹم **B2** اب "ذمہ داری، ڈیڈ لائن، الرٹ، ویریفیکیشن" کی قبولیت معیار پر پورا اترتا ہے۔