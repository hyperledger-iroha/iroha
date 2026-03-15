---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/observability-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: مشاہدہ کرنے والا منصوبہ
عنوان: مشاہدہ کی منصوبہ بندی اور SoraFS کے لئے SLO
سائڈبار_لیبل: مشاہدہ اور ایس ایل او
تفصیل: گیٹ ویز SoraFS ، نوڈس اور ملٹی سورس آرکیسٹریٹر کے لئے ٹیلی میٹری اسکیم ، ڈیش بورڈز اور غلطی کے بجٹ کی پالیسی۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs_observability_plan.md` میں تعاون یافتہ منصوبے کی عکاسی کرتا ہے۔ دونوں کاپیاں مطابقت پذیری میں رکھیں جب تک کہ پرانا اسفینکس سیٹ مکمل طور پر ہجرت نہ ہوجائے۔
:::

## اہداف
- گیٹ ویز ، نوڈس اور ملٹی سورس آرکیسٹریٹر کے لئے میٹرکس اور ساختی واقعات کی وضاحت کریں۔
- Grafana ڈیش بورڈز ، الرٹ دہلیز اور توثیق کے ہکس فراہم کریں۔
- غلطی کے بجٹ کی پالیسیاں اور افراتفری کی مشقوں کے ساتھ ایس ایل او اہداف کو ٹھیک کریں۔

## میٹرکس ڈائرکٹری

### گیٹ وے سطحیں

| میٹرک | قسم | ٹیگز | نوٹ |
| --------- | ----- | ------- | ----------- |
| `sorafs_gateway_active` | گیج (اپ ڈیٹون کاؤنٹر) | `endpoint` ، `method` ، `variant` ، `chunker` ، `profile` | `SorafsGatewayOtel` کے ذریعے جاری کیا گیا ؛ اختتامی نقطہ/طریقہ کے امتزاج کے ذریعہ پرواز میں HTTP کارروائیوں کی نگرانی کرتا ہے۔ |
| `sorafs_gateway_responses_total` | کاؤنٹر | `endpoint` ، `method` ، `variant` ، `chunker` ، `profile` ، `result` ، `status` ، `error_code` | گیٹ وے کی درخواست کے ذریعہ مکمل ہونے والے ہر آپریشن سے ایک بار کاؤنٹر میں اضافہ ہوتا ہے۔ `result` ∈ {`success` ، `error` ، `dropped`}۔ |
| `sorafs_gateway_ttfb_ms_bucket` | ہسٹوگرام | `endpoint` ، `method` ، `variant` ، `chunker` ، `profile` ، `result` ، `status` ، `error_code` | گیٹ وے کے ردعمل کے لئے وقت سے پہلے بائٹ لیٹینسی ؛ Prometheus `_bucket/_sum/_count` کے بطور برآمد ہوا۔ |
| `sorafs_gateway_proof_verifications_total` | کاؤنٹر | `profile_version` ، `result` ، `error_code` | شواہد چیک کے نتائج درخواست کے وقت ریکارڈ کیے جاتے ہیں (`result` ∈ {`success` ، `failure`})۔ |
| `sorafs_gateway_proof_duration_ms_bucket` | ہسٹوگرام | `profile_version` ، `result` ، `error_code` | POR نقلوں کے لئے توثیق میں تاخیر کی تقسیم۔ |
| `telemetry::sorafs.gateway.request` | ساختہ واقعہ | `endpoint` ، `method` ، `variant` ، `result` ، `status` ، `error_code` ، `duration_ms` | لوکی/ٹیمپو ارتباط کے لئے ہر درخواست کی تکمیل کے بعد ساختہ لاگ ان۔ |
| `torii_sorafs_chunk_range_requests_total` ، `torii_sorafs_gateway_refusals_total` | کاؤنٹر | میراثی لیبل سیٹ | Prometheus میٹرکس تاریخی ڈیش بورڈز کے لئے محفوظ کیا گیا ہے۔ نئی OTLP سیریز کے ساتھ مل کر جاری کیا جاتا ہے۔ |

واقعات `telemetry::sorafs.gateway.request` ساختی پے لوڈ کے ساتھ اوٹیل کاؤنٹرز کی عکاسی کرتے ہیں ، جس میں `endpoint` ، `method` ، `variant` ، `status` ، `error_code` اور `duration_ms` کے لئے `variant` ، `variant` ، `variant` ، `variant` ، `variant` ، `method` کو ظاہر کرتا ہے۔ ایس ایل او کو ٹریک کرنے کے لئے او ٹی ایل پی سیریز۔

### صحت ٹیلی میٹری کے ثبوت| میٹرک | قسم | ٹیگز | نوٹ |
| --------- | ----- | ------- | ----------- |
| `torii_sorafs_proof_health_alerts_total` | کاؤنٹر | `provider_id` ، `trigger` ، `penalty` | ہر بار `RecordCapacityTelemetry` `SorafsProofHealthAlert` کا اخراج۔ `trigger` PDP/POTR/دونوں ناکامیوں کے درمیان فرق کرتا ہے ، اور `penalty` سے پتہ چلتا ہے کہ آیا کولیٹرل واقعی میں لکھا گیا تھا یا کوولڈاؤن کے ذریعہ دبا دیا گیا تھا۔ |
| `torii_sorafs_proof_health_pdp_failures` ، `torii_sorafs_proof_health_potr_breaches` | گیج | `provider_id` | مسئلہ ٹیلی میٹری ونڈو میں PDP/POTR کی تازہ ترین اقدار تاکہ ٹیمیں اس بات کا اندازہ کرسکیں کہ فراہم کنندگان نے پالیسی سے کس حد سے تجاوز کیا ہے۔ |
| `torii_sorafs_proof_health_penalty_nano` | گیج | `provider_id` | آخری الرٹ میں نینو زور کی رقم لکھی گئی (صفر اگر کولڈاؤن نے درخواست کو دبا دیا)۔ |
| `torii_sorafs_proof_health_cooldown` | گیج | `provider_id` | بولین گیج (`1` = انتباہ کو کولڈاؤن کے ذریعہ دبایا جاتا ہے) اس بات کی نشاندہی کرنے کے لئے کہ جب اس کے بعد کے انتباہات عارضی طور پر خاموش ہوجاتے ہیں۔ |
| `torii_sorafs_proof_health_window_end_epoch` | گیج | `provider_id` | الرٹ کے ساتھ وابستہ ٹیلی میٹری ونڈو کا دور تاکہ آپریٹرز نمونے Norito سے وابستہ ہوسکتے ہیں۔ |

یہ فیڈ اب تائیکائی ناظرین ڈیش بورڈ میں پروف ہیلتھ لائن کو کھانا کھاتے ہیں
(`dashboards/grafana/taikai_viewer.json`) ، سی ڈی این آپریٹرز کو براہ راست مرئیت دیتے ہوئے
انتباہات کی جلدیں ، PDP/POTR ٹرگرز کا مرکب ، جرمانے اور کولڈاؤن کی حیثیت سے
فراہم کرنے والے

یہ وہی میٹرکس اب تائیکائی ناظرین کے دو الرٹ کے قواعد کی حمایت کرتے ہیں۔
`SorafsProofHealthPenalty` جب متحرک ہے
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` بڑھ رہا ہے
پچھلے 15 منٹ سے ، اور `SorafsProofHealthCooldown` ایک انتباہ اٹھاتا ہے اگر
فراہم کنندہ پانچ منٹ تک کولڈاؤن میں رہتا ہے۔ دونوں انتباہات میں رہتے ہیں
`dashboards/alerts/taikai_viewer_rules.yml` تاکہ SREs کو سیاق و سباق مل سکے
فوری طور پر پور/پوٹر جبر کے اضافے پر۔

### آرکسٹریٹر سطحیں| میٹرک/واقعہ | قسم | ٹیگز | مینوفیکچر | نوٹ |
| ---------- | ----- | ------- | -------------- | -------------- |
| `sorafs_orchestrator_active_fetches` | گیج | `manifest_id` ، `region` | `FetchMetricsCtx` | پرواز میں سیشن۔ |
| `sorafs_orchestrator_fetch_duration_ms` | ہسٹوگرام | `manifest_id` ، `region` | `FetchMetricsCtx` | ملی سیکنڈ میں دورانیہ ہسٹگرام ؛ 1 ایم ایس سے 30 سیکنڈ تک بالٹیاں۔ |
| `sorafs_orchestrator_fetch_failures_total` | کاؤنٹر | `manifest_id` ، `region` ، `reason` | `FetchMetricsCtx` | وجوہات: `no_providers` ، `no_healthy_providers` ، `no_compatible_providers` ، `exhausted_retries` ، `observer_failed` ، `internal_invariant`۔ |
| `sorafs_orchestrator_retries_total` | کاؤنٹر | `manifest_id` ، `provider_id` ، `reason` | `FetchMetricsCtx` | (`retry` ، `digest_mismatch` ، `length_mismatch` ، `provider_error`) کی وجوہات کے درمیان فرق کرتا ہے۔ |
| `sorafs_orchestrator_provider_failures_total` | کاؤنٹر | `manifest_id` ، `provider_id` ، `reason` | `FetchMetricsCtx` | سیشن کی سطح پر ریکارڈ بندش اور ناکامی کے کاؤنٹرز۔ |
| `sorafs_orchestrator_chunk_latency_ms` | ہسٹوگرام | `manifest_id` ، `provider_id` | `FetchMetricsCtx` | تھرو پٹ/ایس ایل او تجزیہ کے ل ch چنک (ایم ایس) کے ذریعہ بازیافت میں تاخیر کی تقسیم۔ |
| `sorafs_orchestrator_bytes_total` | کاؤنٹر | `manifest_id` ، `provider_id` | `FetchMetricsCtx` | بائٹس کو ظاہر/فراہم کنندہ کو پہنچایا گیا۔ تھرو پٹ کا حساب `rate()` کے ذریعے پروم کیو ایل میں کیا جاتا ہے۔ |
| `sorafs_orchestrator_stalls_total` | کاؤنٹر | `manifest_id` ، `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms` سے بڑے حصوں کا شمار۔ |
| `telemetry::sorafs.fetch.lifecycle` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `event` ، `status` ، `chunk_count` ، SoraFS ، `provider_candidates` ، `provider_candidates` ، `provider_candidates` ، `provider_candidates` ، `provider_candidates` | `FetchTelemetryCtx` | Norito JSON پے لوڈ کے ساتھ جاب لائف سائیکل (اسٹارٹ/مکمل) کی عکاسی کرتا ہے۔ |
| `telemetry::sorafs.fetch.retry` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `provider` ، `reason` ، `attempts` | `FetchTelemetryCtx` | فراہم کنندہ کی ہر سیریز کے لئے جاری کیا گیا ؛ `attempts` میں اضافہ (≥ 1) بڑھتا ہے۔ |
| `telemetry::sorafs.fetch.provider_failure` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `provider` ، `reason` ، `failures` | `FetchTelemetryCtx` | جب فراہم کنندہ انکار کی دہلیز کو عبور کرتا ہے تو شائع ہوا۔ |
| `telemetry::sorafs.fetch.error` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `reason` ، `provider?` ، `provider_reason?` ، `duration_ms` | `FetchTelemetryCtx` | حتمی ناکامی کا ریکارڈ ، لوکی/اسپلنک میں انجانی کے لئے آسان ہے۔ |
| `telemetry::sorafs.fetch.stall` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `provider` ، `latency_ms` ، `bytes` | `FetchTelemetryCtx` | جاری کیا گیا جب حصہ لیٹینسی تشکیل شدہ حد سے تجاوز کرتا ہے (اسٹال کاؤنٹرز کی عکاسی کرتا ہے)۔ |

### نوڈ/نقل کی سطحیں| میٹرک | قسم | ٹیگز | نوٹ |
| --------- | ----- | ------- | ----------- |
| `sorafs_node_capacity_utilisation_pct` | ہسٹوگرام | `provider_id` | اسٹوریج کے استعمال کی فیصد کا اوٹیل ہسٹگرام (`_bucket/_sum/_count` کے بطور برآمد)۔ |
| `sorafs_node_por_success_total` | کاؤنٹر | `provider_id` | اسنیپ شاٹ شیڈیولر سے موصول ہونے والے کامیاب پور نمونوں کا ایک نیرس کاؤنٹر۔ |
| `sorafs_node_por_failure_total` | کاؤنٹر | `provider_id` | ناکام پور نمونوں کا مونوٹونک کاؤنٹر۔ |
| `torii_sorafs_storage_bytes_*` ، `torii_sorafs_storage_por_*` | گیج | `provider` | استعمال شدہ بائٹس ، قطار کی گہرائی ، پور ان فلائٹ کاؤنٹرز کے لئے موجودہ Prometheus گیج۔ |
| `torii_sorafs_capacity_*` ، `torii_sorafs_uptime_bps` ، `torii_sorafs_por_bps` | گیج | `provider` | فراہم کنندہ کی صلاحیت/اپ ٹائم کامیابی کا ڈیٹا گنجائش ڈیش بورڈ میں ظاہر ہوتا ہے۔ |
| `torii_sorafs_por_ingest_backlog` ، `torii_sorafs_por_ingest_failures_total` | گیج | `provider` ، `manifest` | بیکلاگ کی گہرائی کے علاوہ ہر `/v2/sorafs/por/ingestion/{manifest}` پول کے ساتھ برآمد ہونے والے جمع شدہ غلطی کاؤنٹرز "پور اسٹالز" پینل/الرٹ کو کھانا کھلاتے ہیں۔ |

### بروقت بازیافت کا ثبوت (POTR) اور SLA بذریعہ ٹکڑوں

| میٹرک | قسم | ٹیگز | مینوفیکچر | نوٹ |
| --------- | ----- | ------- | -------------- | -------------- |
| `sorafs_potr_deadline_ms` | ہسٹوگرام | `tier` ، `provider` | پوٹ کوآرڈینیٹر | ملی سیکنڈ میں ڈیڈ لائن مارجن (مثبت = مکمل) |
| `sorafs_potr_failures_total` | کاؤنٹر | `tier` ، `provider` ، `reason` | پوٹ کوآرڈینیٹر | وجوہات: `expired` ، `missing_proof` ، `corrupt_proof`۔ |
| `sorafs_chunk_sla_violation_total` | کاؤنٹر | `provider` ، `manifest_id` ، `reason` | SLA مانیٹر | جب ٹکڑوں کی فراہمی ایس ایل او (تاخیر ، کامیابی کی شرح) میں فٹ نہیں بیٹھتی ہے۔ |
| `sorafs_chunk_sla_violation_active` | گیج | `provider` ، `manifest_id` | SLA مانیٹر | بولین گیج (0/1) ، فعال خلاف ورزی ونڈو کے دوران ٹوگل کیا گیا۔ |

## SLO اہداف

- بے اعتماد گیٹ وے کی دستیابی: ** 99.9 ٪ ** (HTTP 2XX/304 جوابات)۔
- اعتماد کے بغیر TTFB P95: گرم ٹائر ≤ 120 ایم ایس ، گرم ٹائر ≤ 300 ایم ایس۔
- ثبوت کی کامیابی کی شرح: .5 99.5 ٪ فی دن۔
- آرکسٹریٹر کامیابی (ٹکڑوں کی تکمیل): ≥ 99 ٪۔

## ڈیش بورڈز اور الرٹس

1. ** گیٹ وے مشاہدہ ** (`dashboards/grafana/sorafs_gateway_observability.json`) - OTEL میٹرکس کے ذریعہ بے اعتماد دستیابی ، TTFB P95 ، غلطی تقسیم اور POR/POTR کی ناکامیوں کی نگرانی کرتا ہے۔
2.
3.

الرٹ پیکیجز:- `dashboards/alerts/sorafs_gateway_rules.yml` - گیٹ وے کی دستیابی ، TTFB ، ثبوت کی ناکامی پھٹ جاتی ہے۔
- `dashboards/alerts/sorafs_fetch_rules.yml` - orchestrator کی ناکامی/دوبارہ/اسٹال ؛ `scripts/telemetry/test_sorafs_fetch_alerts.sh` ، `dashboards/alerts/tests/sorafs_fetch_rules.test.yml` ، `dashboards/alerts/tests/soranet_privacy_rules.test.yml` اور `dashboards/alerts/tests/soranet_policy_rules.test.yml` کے ذریعے توثیق شدہ۔
- `dashboards/alerts/soranet_privacy_rules.yml` - رازداری کے انحطاط ، دباؤ کے الارم ، بیکار کلیکٹر کا پتہ لگانے اور غیر فعال کلکٹر الرٹس (`soranet_privacy_last_poll_unixtime` ، `soranet_privacy_collector_enabled`) کے پھٹ۔
- `dashboards/alerts/soranet_policy_rules.yml` - گمنام براؤن آؤٹ الارمز ، `sorafs_orchestrator_brownouts_total` سے بندھا ہوا۔
- `dashboards/alerts/taikai_viewer_rules.yml` - `torii_sorafs_proof_health_*` پر مبنی ، بہاؤ/انجسٹ/CEK LAG TAIKAI ناظرین کے الارم کے علاوہ نیا جرمانہ/COOLDOWN صحت الرٹس SoraFS ثبوت۔

## ٹریسنگ حکمت عملی

-اوپن لیمٹری اختتام سے آخر تک قبول کریں:
  - گیٹ ویز نے درخواست کی شناختوں ، ظاہر ہضموں اور ٹوکن ہیشوں کی تشریحات کے ساتھ OTLP اسپینز (HTTP) جاری کیا۔
  - آرکیسٹریٹر `tracing` + `opentelemetry` کو برآمد کرنے کی کوششوں کو برآمد کرنے کے لئے استعمال کرتا ہے۔
  - POR چیلنجوں اور اسٹوریج آپریشنز کے لئے بلٹ میں SoraFS نوڈس برآمد اسپینز۔ تمام اجزاء ایک مشترکہ ٹریس ID کا اشتراک کرتے ہیں ، جو `x-sorafs-trace` کے ذریعے منتقل ہوتا ہے۔
- `SorafsFetchOtel` برجز آرکسٹریٹر میٹرکس OTLP ہسٹوگرام میں ، اور `telemetry::sorafs.fetch.*` واقعات لاگ ان پر مبنی بیک اینڈ کے لئے ہلکا پھلکا JSON پے لوڈ فراہم کرتے ہیں۔
- جمع کرنے والے: Prometheus/LOKI/TEMPO کے قریب OTEL جمع کرنے والے چلائیں (TEMPO کو ترجیح دی جاتی ہے)۔ جیگر کے مطابق برآمد کنندگان اختیاری ہیں۔
- اعلی کارڈنلٹی آپریشنز کا نمونہ لیا جانا چاہئے (کامیاب راستوں کے لئے 10 ٪ ، ناکامیوں کے لئے 100 ٪)۔

## TLS ٹیلی میٹری کوآرڈینیشن (SF-5B)

- میٹرکس کی سیدھ:
  - TLS آٹومیشن `sorafs_gateway_tls_cert_expiry_seconds` ، `sorafs_gateway_tls_renewal_total{result}` اور `sorafs_gateway_tls_ech_enabled` شائع کرتا ہے۔
  - TLS/سرٹیفکیٹ پینل میں گیٹ وے جائزہ ڈیش بورڈ میں ان گیجز کو فعال کریں۔
- الرٹ مواصلات:
  - جب ٹی ایل ایس کی میعاد ختم ہونے والے انتباہات کو متحرک کیا جاتا ہے (≤ 14 دن باقی) ، تو اعتماد کے بغیر دستیابی ایس ایل او سے وابستہ ہوں۔
  - غیر فعال کرنے سے ای سی ایچ ٹی ٹی ایل ایس اور دستیابی پینلز دونوں کا حوالہ دیتے ہوئے ثانوی انتباہ جاری کرتا ہے۔
- پائپ لائن: TLS آٹومیشن جاب گیٹ وے میٹرکس کے طور پر اسی Prometheus اسٹیک میں برآمد کرتا ہے۔ SF-5B کے ساتھ کوآرڈینیشن آلہ کی کٹوتی کو یقینی بناتا ہے۔

## نام اور ٹیگنگ کنونشن- میٹرک کے نام موجودہ `torii_sorafs_*` یا `sorafs_*` کے سابقہ ​​Torii اور گیٹ وے کے ذریعہ استعمال کرتے ہیں۔
- لیبل سیٹ معیاری ہیں:
  - `result` → HTTP نتیجہ (`success` ، `refused` ، `failed`)۔
  - `reason` → ناکامی/غلطی کا کوڈ (`unsupported_chunker` ، `timeout` ، وغیرہ)۔
  - `provider` → ہیکس انکوڈڈ فراہم کنندہ شناخت کنندہ۔
  - `manifest` → کیننیکل ڈائجسٹ مینی فیسٹ (اعلی کارڈنلٹی پر چھوٹا ہوا)۔
  - `tier` → ٹائر ڈیکلیریٹو لیبل (`hot` ، `warm` ، `archive`)۔
- ٹیلی میٹری کے اخراج پوائنٹس:
  - گیٹ وے میٹرکس `torii_sorafs_*` کے تحت رہتے ہیں اور `crates/iroha_core/src/telemetry.rs` سے کنونشنوں کو دوبارہ استعمال کرتے ہیں۔
  - آرکیسٹریٹر میٹرکس `sorafs_orchestrator_*` اور واقعات `telemetry::sorafs.fetch.*` (لائف سائیکل ، دوبارہ کوشش ، فراہم کنندہ کی ناکامی ، غلطی ، اسٹال) کا اخراج کرتا ہے ، جس میں ڈائجسٹ مینی فیسٹ ، جاب ID ، خطے اور فراہم کنندہ شناخت کنندگان کے ساتھ نشان لگا دیا گیا ہے۔
  - نوڈس `torii_sorafs_storage_*` ، `torii_sorafs_capacity_*` اور `torii_sorafs_por_*` شائع کریں۔
- Prometheus کے نام کے تحت مشترکہ دستاویز میں میٹرکس کے کیٹلاگ کو رجسٹر کرنے کے مشاہدے کے ساتھ مربوط ہوں ، بشمول لیبل کارڈینلٹی (اوپری حدود فراہم کرنے والے/ظاہر کرنے والے) کی توقعات۔

## ڈیٹا پائپ لائن

- جمع کرنے والوں کو ہر جزو کے ساتھ تعینات کیا جاتا ہے ، Prometheus (میٹرکس) اور لوکی/ٹیمپو (لاگ/ٹریس) میں OTLP برآمد کرتے ہیں۔
- اختیاری ای بی پی ایف (ٹیٹراگون) گیٹ ویز/نوڈس کے لئے کم سطح کا ٹریسنگ کو افزودہ کرتا ہے۔
- Torii اور بلٹ ان نوڈس کے لئے `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` استعمال کریں۔ آرکسٹریٹر `install_sorafs_fetch_otlp_exporter` پر کال کرنا جاری رکھے ہوئے ہے۔

## توثیق ہکس

- CI میں `scripts/telemetry/test_sorafs_fetch_alerts.sh` چلائیں تاکہ Prometheus الرٹ قواعد اسٹالز میٹرکس اور رازداری کے دبانے کی جانچ پڑتال کے ساتھ ہم آہنگ رہیں۔
- Grafana ڈیش بورڈز کو ورژن کنٹرول (`dashboards/grafana/`) کے تحت رکھیں اور جب ڈیش بورڈز تبدیل ہوتے ہیں تو اسکرین شاٹس/لنکس کو اپ ڈیٹ کریں۔
- افراتفری کی مشق `scripts/telemetry/log_sorafs_drill.sh` کے ذریعے لاگ نتائج ؛ توثیق `scripts/telemetry/validate_drill_log.sh` (دیکھیں [آپریشنز پلے بوک] (operations-playbook.md))۔