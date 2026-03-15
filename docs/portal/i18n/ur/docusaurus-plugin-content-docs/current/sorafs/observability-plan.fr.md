---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/observability-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: مشاہدہ کرنے والا منصوبہ
عنوان: SoraFS کا مشاہدہ اور SLO پلان
سائڈبار_لیبل: مشاہدہ اور ایس ایل او
تفصیل: SoraFS گیٹ ویز ، نوڈس اور ملٹی سورس آرکیسٹریٹر کے لئے ٹیلی میٹری اسکیم ، ڈیش بورڈز اور غلطی کے بجٹ کی پالیسی۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs_observability_plan.md` میں برقرار رکھے گئے منصوبے کی عکاسی کرتا ہے۔ دونوں کاپیاں مطابقت پذیری میں رکھیں جب تک کہ پرانا اسفینکس سیٹ مکمل طور پر ہجرت نہ ہوجائے۔
:::

## مقاصد
- گیٹ ویز ، نوڈس اور ملٹی سورس آرکسٹریٹر کے لئے میٹرکس اور ساختی واقعات کی وضاحت کریں۔
- Grafana ڈیش بورڈز ، الرٹ دہلیز اور توثیق کے ہکس فراہم کریں۔
- غلطی کے بجٹ اور افراتفری کی مشق کی پالیسیوں کے ساتھ ایس ایل او اہداف قائم کریں۔

## میٹرکس کیٹلاگ

### گیٹ وے سطحیں

| میٹرک | قسم | لیبل | نوٹ |
| -------- | ------ | -------------- | ------- |
| `sorafs_gateway_active` | گیج (اپ ڈیٹون کاؤنٹر) | `endpoint` ، `method` ، `variant` ، `chunker` ، `profile` | `SorafsGatewayOtel` کے ذریعے جاری کیا گیا ؛ اختتامی نقطہ/طریقہ کے امتزاج کے ذریعہ پرواز میں HTTP آپریشنز کو ٹریک کرتا ہے۔ |
| `sorafs_gateway_responses_total` | کاؤنٹر | `endpoint` ، `method` ، `variant` ، `chunker` ، `profile` ، `result` ، `status` ، `error_code` | ہر ایک مکمل گیٹ وے کی درخواست میں ایک بار اضافہ ؛ `result` ∈ {`success` ، `error` ، `dropped`}۔ |
| `sorafs_gateway_ttfb_ms_bucket` | ہسٹوگرام | `endpoint` ، `method` ، `variant` ، `chunker` ، `profile` ، `result` ، `status` ، `error_code` | گیٹ وے کے ردعمل کے لئے وقت سے پہلے بائٹ لیٹینسی ؛ Prometheus `_bucket/_sum/_count` کو برآمد کیا گیا۔ |
| `sorafs_gateway_proof_verifications_total` | کاؤنٹر | `profile_version` ، `result` ، `error_code` | استفسار کے وقت (`result` ∈ {`success` ، `failure`}) پر پکڑے گئے ثبوت کی توثیق کے نتائج۔ |
| `sorafs_gateway_proof_duration_ms_bucket` | ہسٹوگرام | `profile_version` ، `result` ، `error_code` | پور کی رسیدوں کے لئے توثیق میں تاخیر کی تقسیم۔ |
| `telemetry::sorafs.gateway.request` | ساختہ واقعہ | `endpoint` ، `method` ، `variant` ، `result` ، `status` ، `error_code` ، `duration_ms` | لوکی/ٹیمپو ارتباط کے لئے استفسار کے ہر سرے پر سٹرکچرڈ لاگ جاری کیا گیا۔ |
| `torii_sorafs_chunk_range_requests_total` ، `torii_sorafs_gateway_refusals_total` | کاؤنٹر | میراثی لیبل سیٹ | میٹرکس Prometheus تاریخی ڈیش بورڈز کے لئے برقرار ہے۔ نئی OTLP سیریز کے متوازی طور پر جاری کیا گیا۔ |

واقعات `telemetry::sorafs.gateway.request` ساختہ پے لوڈ کے ساتھ اوٹیل کاؤنٹرز کی عکاسی کرتے ہیں ، `endpoint` ، `method` ، `variant` ، `status` ، `error_code` اور `duration_ms` ایس ایل او کی نگرانی کے لئے او ٹی ایل پی سیریز۔

### ثبوت صحت ٹیلی میٹری| میٹرک | قسم | لیبل | نوٹ |
| -------- | ------ | -------------- | ------- |
| `torii_sorafs_proof_health_alerts_total` | کاؤنٹر | `provider_id` ، `trigger` ، `penalty` | ہر بار `RecordCapacityTelemetry` ایک `SorafsProofHealthAlert` جاری کرتا ہے۔ `trigger` PDP/POTR/دونوں ناکامیوں کے درمیان فرق کرتا ہے ، جبکہ `penalty` نے گرفتاری کی ہے کہ آیا کولیٹرل کو واقعی کٹا ہوا تھا یا کولڈاؤن نے ہٹا دیا تھا۔ |
| `torii_sorafs_proof_health_pdp_failures` ، `torii_sorafs_proof_health_potr_breaches` | گیج | `provider_id` | ٹیموں کو فراہم کنندگان کے ذریعہ پالیسی کی خلاف ورزی کا اندازہ کرنے کے لئے مجرم ٹیلی میٹری ونڈو میں تازہ ترین PDP/POTR گنتی کی اطلاع دی گئی ہے۔ |
| `torii_sorafs_proof_health_penalty_nano` | گیج | `provider_id` | آخری انتباہ پر نینو زور کی رقم کاٹ دی گئی (جب کولڈاؤن نے درخواست کو حذف کردیا تو صفر)۔ |
| `torii_sorafs_proof_health_cooldown` | گیج | `provider_id` | بولین گیج (`1` = انتباہ کولڈاؤن کے ذریعہ دبایا جاتا ہے) جب اس کے بعد کے انتباہات عارضی طور پر خاموش ہوجاتے ہیں تو اشارہ کرنے کے لئے اشارہ کرنے کے لئے) |
| `torii_sorafs_proof_health_window_end_epoch` | گیج | `provider_id` | الرٹ سے متعلق ٹیلی میٹری ونڈو کے لئے ریکارڈ شدہ ایپچ تاکہ آپریٹرز Norito نمونے کے ساتھ وابستہ ہوسکیں۔ |

یہ بہاؤ اب تائیکائی ناظرین ڈیش بورڈ کی پروف ہیلتھ لائن کو کھانا کھاتے ہیں
(`dashboards/grafana/taikai_viewer.json`) ، سی ڈی این آپریٹرز کو براہ راست مرئیت فراہم کرنا
الرٹ جلدوں پر ، PDP/POTR ٹرگر مکس ، جرمانے اور کولڈاؤن کی حیثیت سے
فراہم کنندہ

اب وہی میٹرکس دو تائیکائی ناظرین کے انتباہ کے قواعد کی حمایت کرتی ہے۔
`SorafsProofHealthPenalty` محرکات جب
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` بڑھتا ہے
آخری 15 منٹ میں ، جبکہ `SorafsProofHealthCooldown` ایک انتباہ جاری کرتا ہے اگر a
فراہم کنندہ پانچ منٹ تک کولڈاؤن میں رہتا ہے۔ دونوں انتباہات میں رہتے ہیں
`dashboards/alerts/taikai_viewer_rules.yml` تاکہ SREs کا فوری سیاق و سباق ہو
جب پور/پوٹر انفورسمنٹ شدت اختیار کرتا ہے۔

### آرکسٹریٹر سطحیں| میٹرک / واقعہ | قسم | لیبل | پروڈیوسر | نوٹ |
| ------------------------ | ------ | ------------ | -------------- | ------- |
| `sorafs_orchestrator_active_fetches` | گیج | `manifest_id` ، `region` | `FetchMetricsCtx` | فی الحال پرواز میں سیشنز۔ |
| `sorafs_orchestrator_fetch_duration_ms` | ہسٹوگرام | `manifest_id` ، `region` | `FetchMetricsCtx` | ملی سیکنڈ میں دورانیہ ہسٹگرام ؛ بالٹیاں 1 ایم ایس سے 30 سیکنڈ۔ |
| `sorafs_orchestrator_fetch_failures_total` | کاؤنٹر | `manifest_id` ، `region` ، `reason` | `FetchMetricsCtx` | وجوہات: `no_providers` ، `no_healthy_providers` ، `no_compatible_providers` ، `exhausted_retries` ، `observer_failed` ، `internal_invariant`۔ |
| `sorafs_orchestrator_retries_total` | کاؤنٹر | `manifest_id` ، `provider_id` ، `reason` | `FetchMetricsCtx` | دوبارہ کوشش کرنے کی وجوہات (`retry` ، `digest_mismatch` ، `length_mismatch` ، `provider_error`) میں فرق کرتا ہے۔ |
| `sorafs_orchestrator_provider_failures_total` | کاؤنٹر | `manifest_id` ، `provider_id` ، `reason` | `FetchMetricsCtx` | سیشن کی سطح پر غیر فعال ہونے اور ناکامی کی گنتی پر قبضہ کرتا ہے۔ |
| `sorafs_orchestrator_chunk_latency_ms` | ہسٹوگرام | `manifest_id` ، `provider_id` | `FetchMetricsCtx` | ان پٹ/ایس ایل او تجزیہ کے لئے فی حصہ (ایم ایس) کے مطابق لیٹینسی ڈسٹری بیوشن کو بازیافت کریں۔ |
| `sorafs_orchestrator_bytes_total` | کاؤنٹر | `manifest_id` ، `provider_id` | `FetchMetricsCtx` | منشور/فراہم کنندہ کے ذریعہ فراہم کردہ بائٹس ؛ `rate()` کے ذریعے تھروپپٹ کو پروم کیو ایل میں کم کریں۔ |
| `sorafs_orchestrator_stalls_total` | کاؤنٹر | `manifest_id` ، `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms` سے تجاوز کرنے والے حصوں کا شمار کریں۔ |
| `telemetry::sorafs.fetch.lifecycle` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `event` ، `status` ، `chunk_count` ، SoraFS ، `provider_candidates` ، `provider_candidates` ، `provider_candidates` ، `provider_candidates` ، `provider_candidates` | `FetchTelemetryCtx` | JSON پے لوڈ Norito کے ساتھ جاب لائف سائیکل (اسٹارٹ/مکمل) کی عکاسی کرتا ہے۔ |
| `telemetry::sorafs.fetch.retry` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `provider` ، `reason` ، `attempts` | `FetchTelemetryCtx` | سپلائر کے ذریعہ دوبارہ کوشش کے سلسلے کے ذریعہ جاری کیا گیا۔ `attempts` میں اضافہ کی کوششیں (≥ 1)۔ |
| `telemetry::sorafs.fetch.provider_failure` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `provider` ، `reason` ، `failures` | `FetchTelemetryCtx` | شائع ہوا جب ایک سپلائر ناکامی کی دہلیز کو عبور کرتا ہے۔ |
| `telemetry::sorafs.fetch.error` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `reason` ، `provider?` ، `provider_reason?` ، `duration_ms` | `FetchTelemetryCtx` | ٹرمینل کی ناکامی کا ریکارڈ ، لوکی/اسپلنک ادخال کے لئے موزوں ہے۔ |
| `telemetry::sorafs.fetch.stall` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `provider` ، `latency_ms` ، `bytes` | `FetchTelemetryCtx` | جاری کیا گیا جب حصہ لیٹینسی تشکیل شدہ حد سے تجاوز کرتا ہے (اسٹال کاؤنٹرز کی عکاسی کرتا ہے)۔ |

### نوڈ/نقل کی سطحیں| میٹرک | قسم | لیبل | نوٹ |
| -------- | ------ | -------------- | ------- |
| `sorafs_node_capacity_utilisation_pct` | ہسٹوگرام | `provider_id` | اسٹوریج کے استعمال کی فیصد کا اوٹیل ہسٹگرام (`_bucket/_sum/_count` کے بطور برآمد)۔ |
| `sorafs_node_por_success_total` | کاؤنٹر | `provider_id` | کامیاب پور نمونوں کا مونوٹونک کاؤنٹر ، جو شیڈولر اسنیپ شاٹس سے اخذ کیا گیا ہے۔ |
| `sorafs_node_por_failure_total` | کاؤنٹر | `provider_id` | ناکام پور نمونوں کا مونوٹونک کاؤنٹر۔ |
| `torii_sorafs_storage_bytes_*` ، `torii_sorafs_storage_por_*` | گیج | `provider` | استعمال شدہ بائٹس کے لئے موجودہ Prometheus گیجز ، قطار کی گہرائی ، ان فلائٹ پور گنتی کے لئے۔ |
| `torii_sorafs_capacity_*` ، `torii_sorafs_uptime_bps` ، `torii_sorafs_por_bps` | گیج | `provider` | سپلائر کی گنجائش/کامیاب اپ ٹائم ڈیٹا صلاحیت ڈیش بورڈ میں بے نقاب ہے۔ |
| `torii_sorafs_por_ingest_backlog` ، `torii_sorafs_por_ingest_failures_total` | گیج | `provider` ، `manifest` | بیک بلاگ کی گہرائی کے علاوہ مجموعی ناکامی کاؤنٹرز برآمد کیے جاتے ہیں جب ہر بار `/v1/sorafs/por/ingestion/{manifest}` سے استفسار کیا جاتا ہے ، جس سے "پور اسٹالز" پینل/الرٹ کھلایا جاتا ہے۔ |

### بروقت بازیافت (POTR) اور chunk SLA کا ثبوت

| میٹرک | قسم | لیبل | پروڈیوسر | نوٹ |
| --------- | ------ | -------------- | -------------- | ------- |
| `sorafs_potr_deadline_ms` | ہسٹوگرام | `tier` ، `provider` | پوٹ کوآرڈینیٹر | ملی سیکنڈ میں ڈیڈ لائن مارجن (مثبت = احترام) |
| `sorafs_potr_failures_total` | کاؤنٹر | `tier` ، `provider` ، `reason` | پوٹ کوآرڈینیٹر | وجوہات: `expired` ، `missing_proof` ، `corrupt_proof`۔ |
| `sorafs_chunk_sla_violation_total` | کاؤنٹر | `provider` ، `manifest_id` ، `reason` | SLA مانیٹر | جب کام کی ترسیل ایس ایل او (تاخیر ، کامیابی کی شرح) سے محروم ہوجاتی ہے تو متحرک۔ |
| `sorafs_chunk_sla_violation_active` | گیج | `provider` ، `manifest_id` | SLA مانیٹر | فعال خلاف ورزی ونڈو کے دوران بولین گیج (0/1) چالو ہوا۔ |

## ایس ایل او مقاصد

- بے اعتماد گیٹ وے کی دستیابی: ** 99.9 ٪ ** (HTTP 2XX/304 جوابات)۔
- TTFB P95 بے اعتماد: گرم ٹائر ≤ 120 ایم ایس ، گرم ٹائر ≤ 300 ایم ایس۔
- پروف کامیابی کی شرح: ≥99.5 ٪ فی دن۔
- آرکسٹریٹر کامیابی (ٹکڑوں کو حتمی شکل دینا): ≥ 99 ٪۔

## ڈیش بورڈز اور الرٹس

1. ** گیٹ وے مشاہدہ ** (`dashboards/grafana/sorafs_gateway_observability.json`) - قابل اعتماد دستیابی ، TTFB P95 ، انکار کی تقسیم اور POR/POTR کی ناکامیوں کو OTEL میٹرکس کے ذریعے ٹریک کرتا ہے۔
2.
3.

الرٹ پیکیجز:- `dashboards/alerts/sorafs_gateway_rules.yml` - گیٹ وے کی دستیابی ، TTFB ، پروف ناکامی چوٹیوں۔
- `dashboards/alerts/sorafs_fetch_rules.yml` - آرکسٹریٹر کی ناکامی/دوبارہ کوششیں/اسٹال ؛ `scripts/telemetry/test_sorafs_fetch_alerts.sh` ، `dashboards/alerts/tests/sorafs_fetch_rules.test.yml` ، `dashboards/alerts/tests/soranet_privacy_rules.test.yml` اور `dashboards/alerts/tests/soranet_policy_rules.test.yml` کے ذریعے توثیق شدہ۔
- `dashboards/alerts/soranet_privacy_rules.yml` - رازداری کے انحطاط اسپائکس ، حذف کرنے کے الارم ، غیر فعال کلکٹر کا پتہ لگانے اور غیر فعال کلکٹر الرٹس (`soranet_privacy_last_poll_unixtime` ، `soranet_privacy_collector_enabled`)۔
- `dashboards/alerts/soranet_policy_rules.yml` - گمنامی براؤن آؤٹ الارمز `sorafs_orchestrator_brownouts_total` پر وائرڈ۔
- `dashboards/alerts/taikai_viewer_rules.yml` - Taikai دیکھنے والا بہاؤ/انجسٹ/CEK وقفہ الارم کے علاوہ SoraFS سے `torii_sorafs_proof_health_*` سے چلنے والی نئی صحت جرمانہ/COOLDOWN انتباہات۔

## ٹریسنگ حکمت عملی

-اوپن لیمٹری کو اختتام سے آخر میں اپنائیں:
  - گیٹ ویز ایٹ ایل پی (HTTP) کی درخواستوں کی IDs ، ظاہر ہضموں اور ٹوکن ہیشوں کے ساتھ تشریح کی گئی۔
  - آرکیسٹریٹر `tracing` + `opentelemetry` کا استعمال بازیافت کی کوششوں کے دوران برآمد کرنے کے لئے استعمال کرتا ہے۔
  - جہاز SoraFS نوڈس POR چیلنجوں اور اسٹوریج کی کارروائیوں کے لئے برآمد اسپینز۔ تمام اجزاء `x-sorafs-trace` کے ذریعے پھیلائے گئے مشترکہ ٹریس ID کا اشتراک کرتے ہیں۔
- `SorafsFetchOtel` آرکسٹریٹر میٹرکس کو OTLP ہسٹوگرام سے لنک کرتا ہے جبکہ `telemetry::sorafs.fetch.*` واقعات لاگ سینٹرک بیک اینڈ کے لئے ہلکا پھلکا JSON پے لوڈ فراہم کرتے ہیں۔
- جمع کرنے والے: Prometheus/LOKI/TEMPO (ترجیحی ٹیمپو) کے ساتھ OTEL جمع کرنے والے چلائیں۔ جیگر API برآمد کنندگان اختیاری ہیں۔
- اعلی کارڈنلٹی آپریشنز کا نمونہ لیا جانا چاہئے (کامیابی کے راستوں کے لئے 10 ٪ ، ناکامیوں کے لئے 100 ٪)۔

## TLS ٹیلی میٹری کوآرڈینیشن (SF-5B)

- میٹرکس سیدھ:
  - TLS آٹومیشن `sorafs_gateway_tls_cert_expiry_seconds` ، `sorafs_gateway_tls_renewal_total{result}` اور `sorafs_gateway_tls_ech_enabled` شائع کرتا ہے۔
  - TLS/سرٹیفکیٹ پینل کے تحت گیٹ وے جائزہ ڈیش بورڈ میں ان گیجز کو شامل کریں۔
- لنکنگ الرٹس:
  - جب TLS میعاد ختم ہونے والے انتباہات ٹرگر (≤ 14 دن باقی ہیں) تو ، بے اعتماد دستیابی SLO سے وابستہ ہوں۔
  - غیر فعال کرنے سے ای سی ایچ ٹی ٹی ایل ایس اور دستیابی پینلز دونوں کا حوالہ دیتے ہوئے ثانوی انتباہ جاری کرتا ہے۔
- پائپ لائن: TLS آٹومیشن جاب اسی اسٹیک Prometheus میں گیٹ وے میٹرکس کی طرح برآمد کرتا ہے۔ SF-5B کے ساتھ کوآرڈینیشن کٹوتی کے آلے کو یقینی بناتا ہے۔

## میٹرک نام اور لیبلنگ کنونشنز- میٹرک کے نام موجودہ `torii_sorafs_*` یا `sorafs_*` کے سابقہ ​​Torii اور گیٹ وے کے ذریعہ استعمال کیے جاتے ہیں۔
- لیبل سیٹ معیاری ہیں:
  - `result` → HTTP نتیجہ (`success` ، `refused` ، `failed`)۔
  - `reason` → انکار/غلطی کا کوڈ (`unsupported_chunker` ، `timeout` ، وغیرہ)۔
  - `provider` → سپلائر شناخت کنندہ ہیکس میں انکوڈ کیا گیا۔
  - `manifest` → کیننیکل منشور ڈائجسٹ (جب کارڈینلٹی زیادہ ہو تو چھوٹا ہوا)۔
  - `tier` → اعلامیہ درجے کے لیبل (`hot` ، `warm` ، `archive`)۔
- ٹیلی میٹری ٹرانسمیشن پوائنٹس:
  - گیٹ وے میٹرکس `torii_sorafs_*` کے تحت رہتے ہیں اور `crates/iroha_core/src/telemetry.rs` کے کنونشنوں کو دوبارہ استعمال کریں۔
  - آرکیسٹریٹر `sorafs_orchestrator_*` میٹرکس اور `telemetry::sorafs.fetch.*` واقعات (لائف سائیکل ، دوبارہ کوشش ، فراہم کنندہ کی ناکامی ، غلطی ، اسٹال) مینی فیسٹ ڈائجسٹ ، جاب ID ، خطے اور فراہم کنندہ IDs کے ساتھ ٹیگ کردہ خارج کرتا ہے۔
  - نوڈس `torii_sorafs_storage_*` ، `torii_sorafs_capacity_*` ، اور `torii_sorafs_por_*` کو بے نقاب کریں۔
- مشترکہ نام کی دستاویز Prometheus میں میٹرکس کیٹلاگ کو بچانے کے لئے مشاہدہ کے ساتھ ہم آہنگی ، جس میں لیبل کارڈنلٹی کی توقعات (وینڈر/مینی فیسٹ اوپری حدود) شامل ہیں۔

## ڈیٹا پائپ لائن

- جمع کرنے والے ہر جزو کے ساتھ تعینات کرتے ہیں ، Prometheus (میٹرکس) اور لوکی/ٹیمپو (لاگ/ٹریس) پر OTLP برآمد کرتے ہیں۔
- اختیاری ای بی پی ایف (ٹیٹراگون) گیٹ ویز/نوڈس کے لئے نچلی سطح کا ٹریسنگ میں اضافہ کرتا ہے۔
- Torii اور ایمبیڈڈ نوڈس کے لئے `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` استعمال کریں۔ آرکسٹریٹر `install_sorafs_fetch_otlp_exporter` پر کال کرنا جاری رکھے ہوئے ہے۔

## توثیق ہکس

- CI میں `scripts/telemetry/test_sorafs_fetch_alerts.sh` چلائیں تاکہ یہ یقینی بنایا جاسکے کہ Prometheus الرٹ قواعد اسٹال میٹرکس اور رازداری کے دباؤ چیکوں کے ساتھ منسلک رہیں۔
- ڈیش بورڈز Grafana کو ورژن کنٹرول (`dashboards/grafana/`) کے تحت رکھیں اور پینل تبدیل ہونے پر کیپچرز/لنکس کو اپ ڈیٹ کریں۔
- افراتفری کی مشق `scripts/telemetry/log_sorafs_drill.sh` کے ذریعے لاگ نتائج ؛ توثیق `scripts/telemetry/validate_drill_log.sh` پر مبنی ہے (دیکھیں [آپریشنز پلے بوک] (operations-playbook.md))۔