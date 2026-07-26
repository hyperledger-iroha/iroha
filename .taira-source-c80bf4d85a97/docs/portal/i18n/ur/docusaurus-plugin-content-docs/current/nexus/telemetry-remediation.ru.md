---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-ٹیلی میٹری-ریمیڈیشن
عنوان: ٹیلی میٹری گیپ فکسنگ پلان Nexus (B2)
تفصیل: آئینہ `docs/source/nexus_telemetry_remediation_plan.md` ٹیلی میٹری گیپ میٹرکس اور آپریشنل ورک فلو کی دستاویزی دستاویزات۔
---

# جائزہ

روڈ میپ آئٹم ** بی 2 - ٹیلی میٹری گیپس کی ملکیت ** کے لئے ایک شائع شدہ منصوبے کی ضرورت ہے جو باقی ہر ٹیلی میٹری گیپ Nexus کو سگنل ، الرٹ تھریشولڈ ، مالک ، ڈیڈ لائن ، اور آڈٹ آرٹیکٹیکٹ سے Q1 2026 آڈٹ ونڈوز کے آغاز سے قبل جوڑتا ہے۔ یہ صفحہ `docs/source/nexus_telemetry_remediation_plan.md` کی عکاسی کرتا ہے تاکہ جاری کردہ انجینئرنگ ، ٹیلی میٹری او پی ایس ، اور ایس ڈی کے مالکان Q1 2026 آڈٹ ونڈوز کے آغاز سے قبل کوریج کی تصدیق کرسکیں۔ روٹ ٹریس اور `TRACE-TELEMETRY-BRIDGE` کی مشقیں۔

# اسپیس میٹرکس

| گیپ آئی ڈی | سگنل اور حفاظتی انتباہ دہلیز | مالک/اضافے | تاریخ (UTC) | ثبوت اور توثیق |
| -------- | ------------ | ----------- | ----------- | ------------------------------- |
| `GAP-TELEM-001` | ہسٹگرام `torii_lane_admission_latency_seconds{lane_id,endpoint}` انتباہ کے ساتھ ** `SoranetLaneAdmissionLatencyDegraded` ** ، جب `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` 5 منٹ (`dashboards/alerts/soranet_lane_rules.yml`) کے اندر اندر متحرک ہوا۔ | `@torii-sdk` (سگنل) + `@telemetry-ops` (الرٹ) ؛ آن کال روٹ ٹریس Nexus کے ذریعے اضافہ۔ | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` میں الرٹ ٹیسٹ پلس ریہرسل ریکارڈنگ `TRACE-LANE-ROUTING` انتباہ اور بازیافت اور آرکائیوڈ کھرچو Nexus [Nexus منتقلی نوٹس] (./nexus-transition-notes) میں۔ |
| `GAP-TELEM-002` | کاؤنٹر `nexus_config_diff_total{knob,profile}` کے ساتھ گارڈریل `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` بلاکنگ تعیناتی (`docs/source/telemetry.md`)۔ | `@nexus-core` (انسٹرومینٹیشن) -> `@telemetry-ops` (الرٹ) ؛ جب کاؤنٹر غیر متوقع طور پر بڑھتا ہے تو گورننس آفیسر کو پیج کیا جاتا ہے۔ | 2026-02-26 | گورننس ڈرائی رن آؤٹ پٹس `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` کے ساتھ ہی محفوظ ہیں۔ ریلیز چیک لسٹ میں Prometheus درخواست کا ایک اسکرین شاٹ اور لاگ کا ایک اقتباس شامل ہے جس کی تصدیق کی گئی ہے کہ `StateTelemetry::record_nexus_config_diff` نے فرق پیدا کیا ہے۔ |
| `GAP-TELEM-003` | واقعہ `TelemetryEvent::AuditOutcome` (میٹرک `nexus.audit.outcome`) انتباہ کے ساتھ ** `NexusAuditOutcomeFailure` ** جب غلطیاں یا گمشدہ نتائج 30 منٹ سے زیادہ کے لئے محفوظ کیے جاتے ہیں (`dashboards/alerts/nexus_audit_rules.yml`)۔ | `@telemetry-ops` (پائپ لائن) `@sec-observability` میں اضافے کے ساتھ۔ | 2026-02-27 | CI گیٹ `scripts/telemetry/check_nexus_audit_outcome.py` آرکائیوز ndjson پے لوڈ اور کریش ہوتے ہیں جب ٹریس ونڈو میں کامیابی کا واقعہ نہیں ہوتا ہے۔ انتباہات کے اسکرین شاٹس روٹ ٹریس رپورٹ کے ساتھ منسلک ہیں۔ |
| `GAP-TELEM-004` | گیج `nexus_lane_configured_total` کے ساتھ گارڈرییل `nexus_lane_configured_total != EXPECTED_LANE_COUNT` ، جو آن کال چیک لسٹ SRE کو کھانا کھلاتا ہے۔ | `@telemetry-ops` (گیج/برآمد) Nexus میں بڑھ گیا جب نوڈس سے مطابقت پذیر ڈائریکٹری کے سائز کی اطلاع دی جائے۔ | 2026-02-28 | شیڈولر ٹیلی میٹری ٹیسٹ `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` اخراج کی تصدیق کرتا ہے۔ آپریٹرز ٹریس ریہرسل پیکیج میں Diff Prometheus + لاگ فریگمنٹ `StateTelemetry::set_nexus_catalogs` شامل کریں۔ |

# آپریشنل ورک فلو1. ** ہفتہ وار ٹریج۔ بلاکرز اور انتباہ ٹیسٹ کے نمونے `status.md` میں ریکارڈ کیے گئے ہیں۔
2. ** خشک رن الرٹس۔
3۔ ** آڈٹ کے ثبوت۔
4. ** اضافے۔

میٹرکس کے شائع کردہ - اور `roadmap.md` اور `status.md` - روڈ میپ آئٹم ** B2 ** کے حوالہ جات اب "ذمہ داری ، ڈیڈ لائن ، الرٹ ، جائزہ" قبولیت کے معیار پر پورا اترتا ہے۔