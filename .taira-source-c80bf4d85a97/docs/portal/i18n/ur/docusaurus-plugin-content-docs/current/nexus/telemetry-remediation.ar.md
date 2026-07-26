---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-ٹیلی میٹری-ریمیڈیشن
عنوان: ٹیلی میٹرک پروسیسنگ پلان Nexus (B2)
تفصیل: `docs/source/nexus_telemetry_remediation_plan.md` کی عین مطابق کاپی پیمائش کے فرق میٹرکس اور آپریشنل ورک فلو کی دستاویزات۔
---

# جائزہ

روڈ میپ عنصر ** بی 2 - پیمائش گیپ کی ملکیت ** کے لئے ایک شائع شدہ منصوبے کی ضرورت ہوتی ہے جو Nexus میں ہر باقی پیمائش کے فرق کو سگنل ، الرٹ رکاوٹ ، مالک ، ڈیڈ لائن ، اور تصدیقی ٹریس سے Q1 2026 آڈٹ ونڈوز کے آغاز سے پہلے جوڑتا ہے۔ یہ صفحہ `docs/source/nexus_telemetry_remediation_plan.md` کی عکاسی کرتا ہے تاکہ ریلیز انجینئرنگ ٹیم ، پیمائش کی کارروائیوں ، اور ایس ڈی کے مالکان روٹ ٹریس مشقوں اور `TRACE-TELEMETRY-BRIDGE` سے پہلے کوریج کی تصدیق کرسکیں۔

# گیپس میٹرکس

| گیپ آئی ڈی | سگنل اور انتباہ رکاوٹ مالک / اضافے | پختگی (UTC) | ثبوت اور توثیق |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
| `GAP-TELEM-001` | ہسٹگرام `torii_lane_admission_latency_seconds{lane_id,endpoint}` انتباہ کے ساتھ ** `SoranetLaneAdmissionLatencyDegraded` ** جاری ہے جب `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` 5 منٹ (`dashboards/alerts/soranet_lane_rules.yml`) سے چل رہا ہے۔ | `@torii-sdk` (سگنل) + `@telemetry-ops` (الرٹ) ؛ Nexus کے لئے آن کال روٹ ٹریس کے ذریعے بڑھتی ہوئی۔ | 02-23-2026 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` کے تحت Prometheus کے ایک مشق اسنیپ شاٹ کے ساتھ الرٹ ٹیسٹ کرتا ہے جس میں [Nexus [Nexus کے لانچ/بازیافت اور آرکائیوز کو [Prometheus] (./nexus-transition-notes) پر انتباہ دکھایا گیا ہے۔ |
| `GAP-TELEM-002` | کاؤنٹر `nexus_config_diff_total{knob,profile}` رکاوٹ کے ساتھ `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` تعیناتی (`docs/source/telemetry.md`) سے بچتا ہے۔ | `@nexus-core` (آلات) -> `@telemetry-ops` (الرٹ) ؛ جب کاؤنٹر غیر متوقع طور پر بڑھتا ہے تو ڈیوٹی پر موجود گورننس آفیسر کو الرٹ کردیا جاتا ہے۔ | 02-26-2026 | گورننس ڈرائی رن آؤٹ پٹ `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` کے ساتھ ہی محفوظ ہے۔ ریلیز کی فہرست میں Prometheus کا ایک سوال سنیپ شاٹ بھی شامل ہے جس کے ساتھ ہی یہ ثابت ہوتا ہے کہ `StateTelemetry::record_nexus_config_diff` نے فرق جاری کیا ہے۔ |
| `GAP-TELEM-003` | ایک `TelemetryEvent::AuditOutcome` (میٹرک `nexus.audit.outcome`) واقعہ ایک انتباہ کے ساتھ پیش آیا ** `NexusAuditOutcomeFailure` ** جب ناکامی یا گمشدہ نتائج 30 منٹ سے زیادہ (`dashboards/alerts/nexus_audit_rules.yml`) تک برقرار رہتے ہیں۔ | `@telemetry-ops` (پائپ لائن) `@sec-observability` میں اضافے کے ساتھ۔ | 02-27-2026 | سی آئی گیٹ وے `scripts/telemetry/check_nexus_audit_outcome.py` آرکائیوز پے لوڈ NDJSON اور ناکام ہوجاتا ہے جب ٹریس ونڈو میں کامیابی کا کوئی واقعہ نہیں ہوتا ہے۔ الرٹ سنیپ شاٹس روٹ ٹریس رپورٹ کے ساتھ منسلک ہیں۔ |
| `GAP-TELEM-004` | گیج `nexus_lane_configured_total` بفر `nexus_lane_configured_total != EXPECTED_LANE_COUNT` کے ساتھ ڈیوٹی پر SRE کی چیک لسٹ کو کھانا کھلاتا ہے۔ | `@telemetry-ops` (گیج/برآمد) `@nexus-core` میں اضافے کے ساتھ جب نوڈس متضاد کیٹلاگ سائز کی اطلاع دیتے ہیں۔ | 02-28-2026 | شیڈولر ٹیلی میٹری ٹیسٹ `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` انسٹال ورژن ؛ آپریٹرز Prometheus DIFF + `StateTelemetry::set_nexus_catalogs` لاگ انپیٹ کو ٹریس ورزش پیکیج سے جوڑیں۔ |

# آپریشنل ورک فلو1. ** ہفتہ وار ٹریج۔ ** مالکان تیاری پر پیشرفت کی اطلاع دیتے ہیں Nexus ؛ رکاوٹیں اور الارم ٹیسٹ کے اشارے `status.md` پر رجسٹرڈ ہیں۔
2. ** الارم کے تجربات۔
3. ** آڈٹ ثبوت۔
4. ** اضافے۔

اس میٹرکس کے ساتھ شائع ہوا - اور `roadmap.md` اور Nexus کے ذریعہ حوالہ دیا گیا ہے - روڈ میپ عنصر ** B2 ** اب قبولیت کے معیار کو پورا کرتا ہے "ذمہ داری ، ڈیڈ لائن ، انتباہ ، توثیق"۔