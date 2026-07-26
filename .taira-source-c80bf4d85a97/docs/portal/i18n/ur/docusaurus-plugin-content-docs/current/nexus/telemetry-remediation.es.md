---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-ٹیلی میٹری-ریمیڈیشن
عنوان: Nexus (B2) ٹیلی میٹری ریمیڈیشن پلان
تفصیل: `docs/source/nexus_telemetry_remediation_plan.md` کا آئینہ ، ٹیلی میٹری گیپ میٹرکس اور آپریشنل بہاؤ کو دستاویز کرتا ہے۔
---

# عمومی خلاصہ

روڈ میپ آئٹم ** بی 2 - ٹیلی میٹری گیپ کی ملکیت ** کے لئے ایک شائع شدہ منصوبے کی ضرورت ہے جو Nexus کے ہر زیر التوا ٹیلی میٹری گیپ کو سگنل ، ایک انتباہ گارڈرییل ، ​​ایک ذمہ دار فریق ، ایک ڈیڈ لائن اور تصدیقی نمونہ سے منسلک کرتا ہے۔ اس صفحے میں `docs/source/nexus_telemetry_remediation_plan.md` کی عکاسی ہوتی ہے تاکہ جاری کردہ انجینئرنگ ، ٹیلی میٹری او پی ایس اور ایس ڈی کے مینیجر روٹ ٹریس اور `TRACE-TELEMETRY-BRIDGE` ٹیسٹ سے پہلے کوریج کی تصدیق کریں۔

# گیپ میٹرکس

| گیپ آئی ڈی | انتباہی سگنل اور محافظ | ذمہ دار / اضافے | تاریخ (UTC) | ثبوت اور توثیق |
| -------- | --------- | --------- | -------- | ------- |
| `GAP-TELEM-001` | ہسٹگرام `torii_lane_admission_latency_seconds{lane_id,endpoint}` انتباہ کے ساتھ ** `SoranetLaneAdmissionLatencyDegraded` ** جو 5 منٹ (`dashboards/alerts/soranet_lane_rules.yml`) کے لئے `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` کو متحرک کرتا ہے۔ | `@torii-sdk` (سگنل) + `@telemetry-ops` (الرٹ) ؛ Nexus کے کال پر روٹ ٹریس کے ذریعے بڑھتی ہے۔ | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` میں الرٹ ٹیسٹ کے علاوہ `TRACE-LANE-ROUTING` ٹیسٹ کی گرفتاری کو ظاہر کرنے والا انتباہ/بازیافت ہوا اور Torii `/metrics` کو [Prometheus ٹرانزیشن] (./nexus-transition-notes) میں محفوظ شدہ دستاویزات کا کھرچنے والا اور کھرچنے والا۔ |
| `GAP-TELEM-002` | کاؤنٹر `nexus_config_diff_total{knob,profile}` کے ساتھ گارڈرییل `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` جو تعیناتیوں کو روکتا ہے (`docs/source/telemetry.md`)۔ | `@nexus-core` (انسٹرومینٹیشن) -> `@telemetry-ops` (الرٹ) ؛ جب کاؤنٹر غیر متوقع طور پر بڑھتا ہے تو گورننس ڈیوٹی آفیسر کو پیج کیا جاتا ہے۔ | 2026-02-26 | گورننس ڈرائی رن آؤٹ پٹس `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` کے ساتھ ہی محفوظ ہیں۔ ریلیز چیک لسٹ میں Prometheus استفسار کے علاوہ لاگ ان ایکسٹریکٹ کی گرفتاری شامل ہے جو یہ ثابت کرتی ہے کہ `StateTelemetry::record_nexus_config_diff` نے فرق جاری کیا ہے۔ |
| `GAP-TELEM-003` | واقعہ `TelemetryEvent::AuditOutcome` (میٹرک `nexus.audit.outcome`) انتباہ کے ساتھ ** `NexusAuditOutcomeFailure` ** جب ناکامی یا گمشدہ نتائج> 30 منٹ (`dashboards/alerts/nexus_audit_rules.yml`) کے لئے برقرار ہیں۔ | `@telemetry-ops` (پائپ لائن) `@sec-observability` میں اضافے کے ساتھ۔ | 2026-02-27 | `scripts/telemetry/check_nexus_audit_outcome.py` CI گیٹ آرکائیوز NDJSON پے لوڈ اور ناکام ہوجاتے ہیں جب ٹریس ونڈو میں کامیابی کا واقعہ نہیں ہوتا ہے۔ روٹ ٹریس رپورٹ سے منسلک الرٹ کیپچر۔ |
| `GAP-TELEM-004` | گیج `nexus_lane_configured_total` کے ساتھ گارڈریل `nexus_lane_configured_total != EXPECTED_LANE_COUNT` جو SRE آن کال چیک لسٹ کو کھانا کھلاتا ہے۔ | `@telemetry-ops` (گیج/برآمد) `@nexus-core` میں اضافے کے ساتھ جب نوڈس متضاد کیٹلاگ سائز کی اطلاع دیتے ہیں۔ | 2026-02-28 | `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` شیڈیولر ٹیلی میٹری ٹیسٹ اخراج کو ظاہر کرتا ہے۔ آپریٹرز Prometheus DIFF + LOG ACTRACT `StateTelemetry::set_nexus_catalogs` ٹریس ٹیسٹ پیکیج سے منسلک کرتے ہیں۔ |

# آپریشنل فلو1. ** ہفتہ وار ٹریج۔ ** مالکان Nexus تیاری کال پر پیشرفت کی اطلاع دیتے ہیں۔ بلاکرز اور الرٹ ٹیسٹ نمونے `status.md` میں لاگ ان ہیں۔
2. ** الرٹ ٹیسٹنگ۔ ** ہر الرٹ کا قاعدہ `dashboards/alerts/tests/*.test.yml` میں انٹری کے ساتھ ساتھ CI کے لئے `promtool test rules` پر عملدرآمد کرنے کے لئے پیش کیا جاتا ہے جب گارڈریل میں تبدیلی آتی ہے۔
3۔ ** آڈٹ ثبوت۔
4. ** اضافے۔

اس میٹرکس کے ساتھ شائع ہوا - اور `roadmap.md` اور `status.md` سے حوالہ دیا گیا ہے - روڈ میپ آئٹم ** B2 ** اب قبولیت کے معیار پر پورا اترتا ہے "ذمہ داری ، ڈیڈ لائن ، انتباہ ، توثیق"۔