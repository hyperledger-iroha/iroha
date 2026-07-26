---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-ٹیلی میٹری-ریمیڈیشن
عنوان: ٹیلی میٹری ریمیڈیشن پلان Nexus (B2)
تفصیل: ٹیلی میٹری انحراف میٹرکس اور آپریشنل بہاؤ کی دستاویز کرتے ہوئے ، `docs/source/nexus_telemetry_remediation_plan.md` کا آئینہ۔
---

# جائزہ

روڈ میپ عنصر ** B2 - ٹیلی میٹری گیپ کی ملکیت ** کے لئے ایک شائع شدہ منصوبے کی ضرورت ہے جس میں Nexus کے باقی ہر ٹیلی میٹری گیپ کو سگنل ، الرٹ گارڈریل ، مالک ، ڈیڈ لائن اور تصدیقی نمونے سے منسلک کیا گیا ہے۔ یہ صفحہ `docs/source/nexus_telemetry_remediation_plan.md` کی عکاسی کرتا ہے تاکہ انجینئرنگ کی رہائی ، ٹیلی میٹری او پی ایس اور ایس ڈی کے مالکان روٹ ٹریس اور `TRACE-TELEMETRY-BRIDGE` سے پہلے کوریج کی تصدیق کرسکیں۔

# انحراف میٹرکس

| گیپ آئی ڈی | انتباہی سگنل اور محافظ | مالک / چڑھنے | ڈیڈ لائن (UTC) | ثبوت اور توثیق |
| -------- | ------------------ | ---------------------- | ------------ | ---------------------------- |
| `GAP-TELEM-001` | ہسٹگرام `torii_lane_admission_latency_seconds{lane_id,endpoint}` انتباہ کے ساتھ ** `SoranetLaneAdmissionLatencyDegraded` ** جب `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` 5 منٹ (`dashboards/alerts/soranet_lane_rules.yml`) کے لئے متحرک ہوا۔ | `@torii-sdk` (سگنل) + `@telemetry-ops` (الرٹ) ؛ آن کال روٹ ٹریس Nexus کے ذریعے اضافہ۔ | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` کے تحت الرٹ ٹیسٹ پلس کیپچر `TRACE-LANE-ROUTING` کو دکھایا گیا انتباہ کو متحرک/دوبارہ قائم کیا گیا ہے اور کھرچنا Nexus میں [Nexus ٹرانزیشن نوٹس] (./nexus-transition-notes) میں محفوظ ہے۔ |
| `GAP-TELEM-002` | کاؤنٹر `nexus_config_diff_total{knob,profile}` کے ساتھ گارڈرییل `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` بلاک کرنے والی تعیناتی (`docs/source/telemetry.md`)۔ | `@nexus-core` (انسٹرومینٹیشن) -> `@telemetry-ops` (الرٹ) ؛ جب کاؤنٹر غیر متوقع طور پر بڑھتا ہے تو گورننس گارڈ آفیسر کو پیج کیا جاتا ہے۔ | 2026-02-26 | گورننس ڈرائی رن آؤٹ پٹس `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` کے ساتھ ہی محفوظ ہیں۔ ریلیز چیک لسٹ میں درخواست Prometheus کے علاوہ لاگ ان نچوڑ میں شامل ہے جس سے یہ ثابت ہوتا ہے کہ `StateTelemetry::record_nexus_config_diff` نے فرق جاری کیا ہے۔ |
| `GAP-TELEM-003` | واقعہ `TelemetryEvent::AuditOutcome` (میٹرک `nexus.audit.outcome`) انتباہ کے ساتھ ** `NexusAuditOutcomeFailure` ** جب ناکامی یا گمشدہ نتائج برقرار ہیں> 30 منٹ (`dashboards/alerts/nexus_audit_rules.yml`)۔ | `@telemetry-ops` (پائپ لائن) `@sec-observability` میں اضافے کے ساتھ۔ | 2026-02-27 | CI گیٹ `scripts/telemetry/check_nexus_audit_outcome.py` آرکائیوز NDJSON پے لوڈ اور ناکام ہوجاتے ہیں جب ٹریس ونڈو میں کامیابی کا واقعہ نہیں ہوتا ہے۔ روٹ ٹریس رپورٹ سے منسلک الرٹ کیپچر۔ |
| `GAP-TELEM-004` | گیج `nexus_lane_configured_total` کے ساتھ گارڈریل `nexus_lane_configured_total != EXPECTED_LANE_COUNT` SRE آن کال چیک لسٹ کو کھانا کھلا رہا ہے۔ | `@telemetry-ops` (گیج/برآمد) `@nexus-core` میں اضافے کے ساتھ جب نوڈس متضاد کیٹلاگ سائز کی اطلاع دیتے ہیں۔ | 2026-02-28 | `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` شیڈیولر کا ٹیلی میٹری ٹیسٹ ٹرانسمیشن کو ثابت کرتا ہے۔ آپریٹرز ٹریس ریپیٹیشن پیکیج کے ساتھ ایک مختلف Prometheus + لاگ انکٹر `StateTelemetry::set_nexus_catalogs` منسلک کرتے ہیں۔ |

# آپریشنل فلو1. ** ہفتہ وار ٹریج۔ ** مالکان تیاری کے دوران پیشرفت کی اطلاع دیتے ہیں Nexus ؛ بلاکس اور الرٹ ٹیسٹ نمونے `status.md` میں لاگ ان ہیں۔
2. ** الرٹ ٹیسٹ۔
3. ** آڈٹ ثبوت۔ ** `TRACE-LANE-ROUTING` اور `TRACE-TELEMETRY-BRIDGE` تکرار کے دوران ، آن کال پر قبضہ Prometheus کے نتائج ، انتباہ کی تاریخ اور متعلقہ اسکرپٹ آؤٹ پٹ (`scripts/telemetry/check_nexus_audit_outcome.py` کے ساتھ `scripts/telemetry/check_redaction_status.py`) اور اسٹورز کے ساتھ اسٹورز۔
4. ** اضافے۔

اس میٹرکس کے ساتھ شائع ہوا - اور `roadmap.md` اور `status.md` سے حوالہ دیا گیا ہے - روڈ میپ آئٹم ** B2 ** اب قبولیت کے معیار پر پورا اترتا ہے "ذمہ داری ، ڈیڈ لائن ، انتباہ ، توثیق"۔