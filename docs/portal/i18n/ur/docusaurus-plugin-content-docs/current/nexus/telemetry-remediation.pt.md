---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-ٹیلی میٹری-ریمیڈیشن
عنوان: Nexus (B2) ٹیلی میٹری ریمیڈیشن پلان
تفصیل: ٹیلی میٹری گیپ میٹرکس اور آپریشنل بہاؤ کی دستاویز کرتے ہوئے ، `docs/source/nexus_telemetry_remediation_plan.md` کا آئینہ۔
---

# جائزہ

روڈ میپ آئٹم ** بی 2 - ٹیلی میٹری گیپ کی ملکیت ** کے لئے ایک شائع شدہ منصوبے کی ضرورت ہے جو ہر Nexus بقایا ٹیلی میٹری گیپ کو سگنل ، ایک انتباہ گارڈرییل ، ​​ایک اسسنی ، اور ایک توثیق نمونہ سے منسلک کرتا ہے۔ یہ صفحہ `docs/source/nexus_telemetry_remediation_plan.md` کو آئینہ دار کرتا ہے تاکہ جاری رکھیں انجینئرنگ ، ٹیلی میٹری او پی ایس ، اور ایس ڈی کے روٹ ٹریس اور `TRACE-TELEMETRY-BRIDGE` ٹیسٹ سے پہلے کوریج کی تصدیق کریں۔

# گیپ میٹرکس

| گیپ آئی ڈی | انتباہ کا نشان اور محافظ | ذمہ دار / اضافے | ڈیڈ لائن (UTC) | ثبوت اور توثیق |
| -------- | ------------------------ |
| `GAP-TELEM-001` | ہسٹگرام `torii_lane_admission_latency_seconds{lane_id,endpoint}` انتباہ کے ساتھ ** `SoranetLaneAdmissionLatencyDegraded` ** فائرنگ جب `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` 5 منٹ کے لئے (`dashboards/alerts/soranet_lane_rules.yml`)۔ | `@torii-sdk` (سگنل) + `@telemetry-ops` (الرٹ) ؛ Nexus کے آن کال روٹ ٹریس کے ذریعے شیڈولنگ۔ | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` پر الرٹ ٹیسٹ پلس `TRACE-LANE-ROUTING` ٹرائلڈ/بازیافت الرٹ اور Nexus سکریپ کو [Nexus ٹرانزیشن نوٹس] (./nexus-transition-notes) کے تحت دائر دکھایا گیا ہے۔ |
| `GAP-TELEM-002` | کاؤنٹر `nexus_config_diff_total{knob,profile}` کے ساتھ گارڈرییل `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` بلاک کرنے والی تعیناتی (`docs/source/telemetry.md`)۔ | `@nexus-core` (انسٹرومینٹیشن) -> `@telemetry-ops` (الرٹ) ؛ جب کاؤنٹر غیر متوقع طور پر بڑھتا ہے تو گورننس آفیسر کو پیج کیا جاتا ہے۔ | 2026-02-26 | گورننس ڈرائی رن آؤٹ پٹس `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` کے ساتھ ہی محفوظ ہیں۔ ریلیز چیک لسٹ میں Prometheus استفسار کے علاوہ لاگ انکیپیٹ کی گرفتاری شامل ہے جس سے یہ ثابت ہوتا ہے کہ `StateTelemetry::record_nexus_config_diff` نے فرق جاری کیا ہے۔ |
| `GAP-TELEM-003` | واقعہ `TelemetryEvent::AuditOutcome` (میٹرک `nexus.audit.outcome`) انتباہ کے ساتھ ** `NexusAuditOutcomeFailure` ** جب ناکامی یا گمشدہ نتائج> 30 منٹ (`dashboards/alerts/nexus_audit_rules.yml`) کے لئے برقرار ہیں۔ | `@telemetry-ops` (پائپ لائن) `@sec-observability` پر اسکیلنگ کے ساتھ۔ | 2026-02-27 | CI گیٹ `scripts/telemetry/check_nexus_audit_outcome.py` آرکائیوز NDJSON پے لوڈ اور ناکام ہوجاتے ہیں جب ٹریس ونڈو میں کامیابی کا کوئی واقعہ نہیں ہوتا ہے۔ روٹ ٹریس رپورٹ سے منسلک الرٹ کیپچر۔ |
| `GAP-TELEM-004` | گیج `nexus_lane_configured_total` کے ساتھ گارڈریل `nexus_lane_configured_total != EXPECTED_LANE_COUNT` SRE آن کال چیک لسٹ کو کھانا کھلا رہا ہے۔ | `@telemetry-ops` (گیج/برآمد) Nexus پر اسکیلنگ جب صارفین متضاد کیٹلاگ سائز کی اطلاع دیتے ہیں۔ | 2026-02-28 | `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` شیڈیولر ٹیلی میٹری ٹیسٹ اس مسئلے کو ثابت کرتا ہے۔ آپریٹرز Prometheus DIFF + `StateTelemetry::set_nexus_catalogs` لاگ انپیٹ ٹریس پرکھ پیکیج میں شامل کریں۔ |

# آپریشنل فلو1. ** ہفتہ وار ٹریج۔ ** مالکان Nexus تیاری کال پر پیشرفت کی اطلاع دیتے ہیں۔ بلاکرز اور الرٹ ٹیسٹ نمونے `status.md` میں لاگ ان ہیں۔
2. ** الرٹ ٹیسٹ۔
3. ** آڈٹ ثبوت۔
4. ** اضافے۔

اس میٹرکس کے ساتھ شائع ہوا - اور `roadmap.md` اور Nexus - روڈ میپ آئٹم ** B2 ** پر حوالہ دیا گیا ہے "اب قبولیت کے معیار کو پورا کرتا ہے" ذمہ داری ، ڈیڈ لائن ، انتباہ ، توثیق "۔