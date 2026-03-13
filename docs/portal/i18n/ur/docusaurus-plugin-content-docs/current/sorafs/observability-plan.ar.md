---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/observability-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: مشاہدہ کرنے والا منصوبہ
عنوان: SoraFS کے لئے مشاہدہ کی منصوبہ بندی اور SLOS
سائڈبار_لیبل: مشاہدہ اور سلو
تفصیل: SoraFS گیٹ ویز ، نوڈس ، اور ملٹی سورس کوآرڈینیٹر کے لئے ٹیلی میٹری چارٹ ، ڈیش بورڈز ، اور غلطی بجٹ پالیسی۔
---

::: منظور شدہ ماخذ کو نوٹ کریں
یہ صفحہ `docs/source/sorafs_observability_plan.md` کے تحت منصوبے کی عکاسی کرتا ہے۔ اس بات کو یقینی بنائیں کہ ان دونوں کاپیاں مطابقت پذیری میں رکھیں جب تک کہ پرانا اسفنکس کلسٹر مکمل طور پر ہجرت نہ ہوجائے۔
:::

## مقاصد
- گیٹ ویز ، نوڈس اور ملٹی سورس آرکیسٹریٹر کے لئے میٹرکس اور ساختی واقعات کی وضاحت کریں۔
- Grafana بورڈز ، الارم کی حدود اور توثیق کے ہکس فراہم کریں۔
- غلطی کے بجٹ کی پالیسیاں اور افراتفری کی مشقوں کے ساتھ ایس ایل او مقاصد انسٹال کریں۔

## میٹرکس کیٹلاگ

### پورٹل سطحیں

| اسکیل | قسم | لیبل | نوٹ |
| -------- | ------- | -------- | ----------- |
| `sorafs_gateway_active` | گیج (اپ ڈیٹون کاؤنٹر) | `endpoint` ، `method` ، `variant` ، `chunker` ، `profile` | `SorafsGatewayOtel` کے ذریعے جاری کیا گیا ؛ ہر اختتامی نقطہ/طریقہ کار کے امتزاج کے لئے جاری HTTP آپریشنوں کو ٹریک کرتا ہے۔ |
| `sorafs_gateway_responses_total` | کاؤنٹر | `endpoint` ، `method` ، `variant` ، `chunker` ، `profile` ، `result` ، `status` ، `error_code` | گیٹ وے کے لئے ہر ایک مکمل درخواست کاؤنٹر میں ایک بار اضافہ ؛ `result` ∈ {`success` ، `error` ، `dropped`}۔ |
| `sorafs_gateway_ttfb_ms_bucket` | ہسٹوگرام | Prometheus گیٹ وے کے ردعمل کی ٹائم ٹو فرسٹ بائٹ لیٹینسی ؛ Prometheus `_bucket/_sum/_count` کے بطور برآمد ہوا۔ |
| `sorafs_gateway_proof_verifications_total` | کاؤنٹر | `profile_version` ، `result` ، `error_code` | درخواست کے لمحے (`result` ∈ {`success` ، `failure`}) پر قبضہ کرنے والے ثبوت کے نتائج۔ |
| `sorafs_gateway_proof_duration_ms_bucket` | ہسٹوگرام | `profile_version` ، `result` ، `error_code` | پور کی رسیدوں کے لئے توثیق میں تاخیر کی تقسیم۔ |
| `telemetry::sorafs.gateway.request` | ساختہ واقعہ | `endpoint` ، `method` ، `variant` ، `result` ، `status` ، `error_code` ، `duration_ms` | لوکی/ٹیمپو کے ساتھ ہم آہنگی کے لئے ہر درخواست کی تکمیل کے بعد ایک ساختی لاگ جاری کیا گیا۔ |
| `torii_sorafs_chunk_range_requests_total` ، `torii_sorafs_gateway_refusals_total` | کاؤنٹر | پرانے لیبل مجموعہ | Prometheus پیمائش تاریخی پینٹنگز کے لئے مخصوص ہے۔ نئی OTLP سیریز کے ساتھ جاری کیا گیا۔ |

`telemetry::sorafs.gateway.request` واقعات اوٹیل کاؤنٹرز کو ساختی پے لوڈ کے ساتھ ظاہر کرتے ہیں ، جس میں `endpoint` ، `method` ، `variant` ، `status` ، `error_code` ، اور Prometheus کے لئے `duration_ms` ، اور `duration_ms` کو ظاہر کیا گیا ہے۔ ایس ایل او کو ٹریک کرنے کے لئے چین۔

### ثبوت کی ٹیلی میٹرک جواز| اسکیل | قسم | لیبل | نوٹ |
| -------- | ------- | -------- | ----------- |
| `torii_sorafs_proof_health_alerts_total` | کاؤنٹر | `provider_id` ، `trigger` ، `penalty` | جب بھی `RecordCapacityTelemetry` `SorafsProofHealthAlert` واقعہ جاری کرتا ہے تو بڑھتا ہے۔ `trigger` PDP/POTR/دونوں ناکامیوں کے مابین فرق کرتا ہے ، جبکہ `penalty` نے گرفت میں لیا ہے کہ آیا کولیٹرل کو واقعی میں چھوٹ دیا گیا ہے یا کوولڈاؤن کے ذریعے خاموش کردیا گیا ہے۔ |
| `torii_sorafs_proof_health_pdp_failures` ، `torii_sorafs_proof_health_potr_breaches` | گیج | `provider_id` | خلاف ورزی ٹیلی میٹرک ونڈو کے اندر تازہ ترین PDP/POTR گنتی ہے تاکہ ٹیمیں اس بات کی پیمائش کرسکیں کہ فراہم کرنے والوں نے پالیسی کی کتنی خلاف ورزی کی ہے۔ |
| `torii_sorafs_proof_health_penalty_nano` | گیج | `provider_id` | آخری الرٹ میں نینو زور کی مقدار کٹوتی کی گئی (جب کوولڈاؤن کی وجہ سے ایپ کو خاموش کیا جاتا ہے تو صفر)۔ |
| `torii_sorafs_proof_health_cooldown` | گیج | `provider_id` | پولی میٹرک (`1` = cooldown دبے ہوئے انتباہ) ظاہر کرنے کے لئے جب فالو اپ الرٹس عارضی طور پر خاموش ہوجاتے ہیں۔ |
| `torii_sorafs_proof_health_window_end_epoch` | گیج | `provider_id` | الارم کے ساتھ وابستہ ٹیلی میٹرک ونڈو کا ریکارڈ شدہ دور آپریٹرز کو Norito کے نشانات سے وابستہ کرنے کے قابل بناتا ہے۔ |

یہ سلسلہ اب تائکائی ناظرین کے پینل میں پروف صحت کی قطار کو کھانا کھاتے ہیں
(`dashboards/grafana/taikai_viewer.json`) ، سی ڈی این آپریٹرز کو فوری مرئیت دیتے ہوئے
الرٹ سائز کے لئے ، PDP/POTR ٹرگر مکس ، جرمانے ، اور ہر فراہم کنندہ کے لئے cooldown کی حیثیت۔

وہی میٹرکس اب تائکائی ناظرین میں دو الرٹ قواعد کی حمایت کرتے ہیں:
`SorafsProofHealthPenalty` لانچ ہوتا ہے
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` کے دوران بڑھتا ہے
آخری 15 منٹ ، جبکہ `SorafsProofHealthCooldown` اگر کوئی فراہم کنندہ رہتا ہے تو انتباہ اٹھاتا ہے
پانچ منٹ کے لئے کولڈاؤن۔ دونوں انتباہات اندر ہیں
`dashboards/alerts/taikai_viewer_rules.yml` تاکہ بڑھتے وقت SREs کو فوری سیاق و سباق مل سکے
POR/POTR درخواست۔

### کوآرڈینیٹر سطحیں| میٹرک/واقعہ | قسم | لیبل | پروڈیوسر | نوٹ |
| ------------------ | ------- | -------- | --------- | ----------- |
| `sorafs_orchestrator_active_fetches` | گیج | `manifest_id` ، `region` | `FetchMetricsCtx` | فی الحال جاری سیشنز۔ |
| `sorafs_orchestrator_fetch_duration_ms` | ہسٹوگرام | `manifest_id` ، `region` | `FetchMetricsCtx` | ملی سیکنڈ میں ہسٹگرام کی مدت ؛ رینجز 1 ایم ایس سے 30 سیکنڈ۔ |
| `sorafs_orchestrator_fetch_failures_total` | کاؤنٹر | `manifest_id` ، `region` ، `reason` | `FetchMetricsCtx` | وجوہات: `no_providers` ، `no_healthy_providers` ، `no_compatible_providers` ، `exhausted_retries` ، `observer_failed` ، `internal_invariant`۔ |
| `sorafs_orchestrator_retries_total` | کاؤنٹر | `manifest_id` ، `provider_id` ، `reason` | `FetchMetricsCtx` | دوبارہ کوشش کرنے کی وجوہات (`retry` ، `digest_mismatch` ، `length_mismatch` ، `provider_error`) میں فرق کرتا ہے۔ |
| `sorafs_orchestrator_provider_failures_total` | کاؤنٹر | `manifest_id` ، `provider_id` ، `reason` | `FetchMetricsCtx` | سیشن کی سطح پر سیشن میں خلل یا ناکامی کی گرفتاری پر قبضہ کرتا ہے۔ |
| `sorafs_orchestrator_chunk_latency_ms` | ہسٹوگرام | `manifest_id` ، `provider_id` | `FetchMetricsCtx` | تھرو پٹ/ایس ایل او تجزیہ کے لئے سلائس لیٹینسی ڈسٹری بیوشن (ایم ایس)۔ |
| `sorafs_orchestrator_bytes_total` | کاؤنٹر | `manifest_id` ، `provider_id` | `FetchMetricsCtx` | بائٹس فی منشور/فراہم کنندہ کی فراہمی ؛ `rate()` کے ذریعے تھروپپٹ نکالیں۔ |
| `sorafs_orchestrator_stalls_total` | کاؤنٹر | `manifest_id` ، `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms` سے تجاوز کرنے والے چپ سیٹوں کا شمار کرتا ہے۔ |
| `telemetry::sorafs.fetch.lifecycle` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `event` ، `status` ، `chunk_count` ، SoraFS ، `provider_candidates` ، `provider_candidates` ، `provider_candidates` ، `provider_candidates` ، `provider_candidates` | `FetchTelemetryCtx` | Norito JSON پے لوڈ کے ساتھ کام کے لائف سائیکل (اسٹارٹ/مکمل) کی عکاسی کرتا ہے۔ |
| `telemetry::sorafs.fetch.retry` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `provider` ، `reason` ، `attempts` | `FetchTelemetryCtx` | ہر سلسلہ کسی فراہم کنندہ کو دوبارہ کوشش کرتا ہے۔ `attempts` مجموعی کوششوں (≥ 1) کی گنتی کرتا ہے۔ |
| `telemetry::sorafs.fetch.provider_failure` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `provider` ، `reason` ، `failures` | `FetchTelemetryCtx` | جب فراہم کنندہ ناکامی کی حد سے تجاوز کرتا ہے تو ظاہر ہوتا ہے۔ |
| `telemetry::sorafs.fetch.error` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `reason` ، `provider?` ، `provider_reason?` ، `duration_ms` | `FetchTelemetryCtx` | لوکی/اسپلنک انجشن کے لئے موزوں ایک حتمی ناکامی لاگ ان۔ |
| `telemetry::sorafs.fetch.stall` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `provider` ، `latency_ms` ، `bytes` | `FetchTelemetryCtx` | جب سلائس لیٹینسی سیٹ کی حد سے تجاوز کر جاتی ہے (اسٹال کاؤنٹرز کی عکاسی کرتے ہیں)۔ |

### نوڈ/تکرار کی سطحیں| اسکیل | قسم | لیبل | نوٹ |
| -------- | ------- | -------- | ----------- |
| `sorafs_node_capacity_utilisation_pct` | ہسٹوگرام | `provider_id` | اسٹوریج کے استعمال کے تناسب کا اوٹیل ہسٹگرام (`_bucket/_sum/_count` کے بطور جاری کیا گیا)۔ |
| `sorafs_node_por_success_total` | کاؤنٹر | `provider_id` | کامیاب پور نمونوں کے لئے ایک واحد کاؤنٹر ، جو شیڈولر اسنیپ شاٹس سے اخذ کیا گیا ہے۔ |
| `sorafs_node_por_failure_total` | کاؤنٹر | `provider_id` | ناکام پور نمونوں کے لئے سنگل کاؤنٹر۔ |
| `torii_sorafs_storage_bytes_*` ، `torii_sorafs_storage_por_*` | گیج | `provider` | استعمال شدہ بائٹس ، قطار کی گہرائی ، اور چلانے والے پور کاؤنٹرز کے لئے Prometheus موجودہ میٹرکس۔ |
| `torii_sorafs_capacity_*` ، `torii_sorafs_uptime_bps` ، `torii_sorafs_por_bps` | گیج | `provider` | فراہم کنندہ صلاحیت کی کامیابی/تیاری کا ڈیٹا صلاحیت کے پینل میں ظاہر ہوتا ہے۔ |
| `torii_sorafs_por_ingest_backlog` ، `torii_sorafs_por_ingest_failures_total` | گیج | `provider` ، `manifest` | جب "پور اسٹالز" پینل/الرٹ فیڈ کے لئے `/v2/sorafs/por/ingestion/{manifest}` سے استفسار کیا جاتا ہے تو بیک بلاگ کی گہرائی کے علاوہ مجموعی ناکامی کاؤنٹرز برآمد ہوتے ہیں۔ |

### بروقت بازیافت (POTR) اور SLA طبقات کا ثبوت

| اسکیل | قسم | لیبل | پروڈیوسر | نوٹ |
| -------- | ------- | -------- | --------- | ----------- |
| `sorafs_potr_deadline_ms` | ہسٹوگرام | `tier` ، `provider` | پوٹ کوآرڈینیٹر | ملی سیکنڈ میں ڈیڈ لائن مارجن (مثبت = میٹ)۔ |
| `sorafs_potr_failures_total` | کاؤنٹر | `tier` ، `provider` ، `reason` | پوٹ کوآرڈینیٹر | وجوہات: `expired` ، `missing_proof` ، `corrupt_proof`۔ |
| `sorafs_chunk_sla_violation_total` | کاؤنٹر | `provider` ، `manifest_id` ، `reason` | SLA مانیٹر | جب طبقہ کی ترسیل ایس ایل او (تاخیر ، کامیابی کی شرح) کو پورا کرنے میں ناکام ہوجاتی ہے تو متحرک۔ |
| `sorafs_chunk_sla_violation_active` | گیج | `provider` ، `manifest_id` | SLA مانیٹر | فعال ناکامی ونڈو کے دوران پولی (0/1) پیمانے میں تبدیلیاں۔ |

## ایس ایل او مقاصد

- صفر ٹرسٹ گیٹ وے کی دستیابی: ** 99.9 ٪ ** (2xx/304 HTTP جوابات)۔
- اعتماد کے بغیر TTFB P95: گرم ٹائر ≤ 120 ایم ایس ، گرم ٹائر ≤ 300 ایم ایس۔
- ثبوت کی کامیابی کی شرح: .5 99.5 ٪ فی دن۔
- ماڈریٹر کی کامیابی (سلائیڈ کی تکمیل): ≥ 99 ٪۔

## نگرانی کرنے والے پینل اور انتباہات

1. ** گیٹ وے مشاہدہ ** (`dashboards/grafana/sorafs_gateway_observability.json`) - قابل اعتماد اور TTFB P95 کی دستیابی اور تفصیل سے انکار اور پور/POTR کی ناکامیوں کو OTEL میٹرکس میں ٹریک کریں۔
2.
3.

الرٹ پیکیجز:

- `dashboards/alerts/sorafs_gateway_rules.yml` - گیٹ وے ، TTFB ، اور شواہد کی ناکامی کی اونچائیوں کی دستیابی۔
- `dashboards/alerts/sorafs_fetch_rules.yml` - کوآرڈینیٹر کی ناکامی/دوبارہ کوششیں/اسٹال ؛ توثیق `scripts/telemetry/test_sorafs_fetch_alerts.sh` ، `dashboards/alerts/tests/sorafs_fetch_rules.test.yml` ، `dashboards/alerts/tests/soranet_privacy_rules.test.yml` ، اور `dashboards/alerts/tests/soranet_policy_rules.test.yml` کے ذریعے کی جاتی ہے۔
- `dashboards/alerts/soranet_privacy_rules.yml` - رازداری کے انحطاط کی چوٹیوں ، گونگا الارم ، بیکار کلکٹر مانیٹرنگ ، اور غیر فعال کلکٹر الرٹس (`soranet_privacy_last_poll_unixtime` ، `soranet_privacy_collector_enabled`)۔
- `dashboards/alerts/soranet_policy_rules.yml` - رازداری براؤن آؤٹ الارمز `sorafs_orchestrator_brownouts_total` پر پابند ہیں۔
- `dashboards/alerts/taikai_viewer_rules.yml` - `torii_sorafs_proof_health_*` پر مبنی SoraFS میں شواہد کی توثیق کرنے کے لئے تائیکائی ناظرین میں بہاؤ/انجسٹ/سی ای کے لیگ الرٹس کے ساتھ ساتھ جرمانہ/کوولڈون الرٹس۔

## ٹریکنگ کی حکمت عملیآخر سے آخر تک اوپن لیمٹری اپنانا:
  - گیٹ ویز درخواست IDs ، ظاہر ہضموں ، اور ٹوکن ہیشوں کے ساتھ OTLP اسپینز (HTTP) جاری کرتے ہیں۔
  - فارمیٹر `tracing` + `opentelemetry` استعمال کرنے کی کوششوں کے لئے اسپین برآمد کرنے کے لئے استعمال کرتا ہے۔
  - SoraFS POR چیلنجوں اور اسٹوریج آپریشنز کے لئے نوڈس کے ساتھ مل گئے۔ تمام اجزاء `x-sorafs-trace` کے ذریعہ منتقل کردہ متحدہ ٹریس ID کا اشتراک کرتے ہیں۔
- `SorafsFetchOtel` ایسوسی ایٹس فارمیٹر میٹرکس کے ساتھ OTLP ہسٹوگرام کے ساتھ جبکہ `telemetry::sorafs.fetch.*` واقعات ریکارڈ پر مبنی پسدیدوں کے لئے ہلکا پھلکا JSON پے لوڈ فراہم کرتے ہیں۔
- جمع کرنے والے: Prometheus/LOKI/TEMPO (TEMPO ترجیحی) کے ساتھ OTEL جمع کرنے والے چلائیں۔ جیگر ہم آہنگ برآمد کنندگان اب بھی اختیاری ہیں۔
- انتہائی کارڈنل عملوں کا نمونہ لیا جانا چاہئے (کامیابیوں کے لئے 10 ٪ ، ناکامیوں کے لئے 100 ٪)۔

## TLS ٹیلی میٹک فارمیٹ (SF-5B)

- سیدھ کرنے والی میٹرکس:
  - TLS آٹومیشن `sorafs_gateway_tls_cert_expiry_seconds` ، `sorafs_gateway_tls_renewal_total{result}` ، اور `sorafs_gateway_tls_ech_enabled` بھیجتا ہے۔
  - TLS/سرٹیفکیٹ پینل کے تحت گیٹ وے جائزہ پینل میں ان میٹرکس کی فہرست بنائیں۔
- لنک الرٹس:
  - جب ٹی ایل ایس کی میعاد ختم ہونے والے انتباہات کو متحرک کیا جاتا ہے (≤ 14 دن باقی) انہیں بے اعتماد ایس ایل او کے ساتھ جوڑ دیتے ہیں۔
  - ایکچ کو غیر فعال کرنا ثانوی انتباہ کا سبب بنتا ہے جو TLS اور دستیابی بورڈ دونوں کا اشارہ کرتا ہے۔
- پائپ لائننگ: TLS آٹومیشن ٹاسک گیٹ وے کی طرح اسی Prometheus اسٹیک میں برآمد کرتا ہے۔ SF-5B کے ساتھ کوآرڈینیشن اس بات کو یقینی بناتا ہے کہ پیمائش میں فالتو پن ختم ہوجائے۔

## معیارات اور لیبلوں کے لئے کنونشنوں کا نام دینا

- معیاری نام `torii_sorafs_*` یا `sorafs_*` کے ذریعہ Torii اور گیٹ وے کے ذریعہ استعمال کیا جاتا ہے۔
- لیبل گروپس متحد ہیں:
  - `result` → HTTP آؤٹ پٹ (`success` ، `refused` ، `failed`)۔
  - `reason` → مسترد/غلطی کا کوڈ (`unsupported_chunker` ، `timeout` ، وغیرہ)۔
  - `provider` → فراہم کنندہ ID ہیکس انکوڈڈ ہے۔
  - `manifest` → ڈائجسٹ قانونی منشور (کارڈنل اونچائی پر کٹائی)۔
  - `tier` → لیبلز شناختی پرت (`hot` ، `warm` ، `archive`)۔
- ٹیلی میٹک جاری کرنے والے پوائنٹس:
  - گیٹ وے کے معیارات `torii_sorafs_*` کے تحت رہتے ہیں اور `crates/iroha_core/src/telemetry.rs` کنونشنز کو دوبارہ استعمال کریں۔
  - کوآرڈینیٹر `sorafs_orchestrator_*` میٹرکس اور `telemetry::sorafs.fetch.*` واقعات (لائف سائیکل ، دوبارہ کوشش ، فراہم کرنے میں ناکامی ، غلطی ، اسٹال) ڈائجسٹ ٹیگز ، جاب ID ، خطے ، اور فراہم کنندہ IDs کے ساتھ جاری کرتا ہے۔
  - نوڈس `torii_sorafs_storage_*` ، `torii_sorafs_capacity_*` اور `torii_sorafs_por_*` دکھاتا ہے۔
- عام ناموں کی دستاویز Prometheus میں میٹرکس کیٹلاگ کو ریکارڈ کرنے کے لئے مشاہدہ کے ساتھ مربوط ہوں ، بشمول لیبل کارڈنلٹی (سپلائر/مینی فیسٹ اوپری حد)۔

## ڈیٹا پائپ لائن

- جمع کرنے والے ہر جزو کے ساتھ شائع ہوتے ہیں ، اور Prometheus (میٹرکس) اور لوکی/ٹیمپو (لاگ/ٹریس) کو OTLP برآمد کرتے ہیں۔
- اختیاری ای بی پی ایف (ٹیٹراگون) گیٹ ویز/نوڈس کی نچلی سطح سے باخبر رہنے کو افزودہ کرتا ہے۔
- Torii اور ایمبیڈڈ نوڈس کے لئے `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` استعمال کریں۔ کوآرڈینیٹر `install_sorafs_fetch_otlp_exporter` پر کال کرنا جاری رکھے ہوئے ہے۔

## توثیق ہکس

- CI کے دوران `scripts/telemetry/test_sorafs_fetch_alerts.sh` چلائیں Prometheus الرٹ قواعد اسٹال میٹرکس اور پرائیویسی گونگا چیک کے ساتھ ہم آہنگی میں رہیں۔
- Grafana بورڈز کو ورژن کنٹرول (`dashboards/grafana/`) کے تحت رکھیں اور جب بورڈز تبدیل ہوتے ہیں تو اسنیپ شاٹس/لنکس کو اپ ڈیٹ کریں۔
- افراتفری کی مشق `scripts/telemetry/log_sorafs_drill.sh` کے ذریعے لاگ نتائج کی مشق کرتی ہے۔ توثیق `scripts/telemetry/validate_drill_log.sh` (دیکھیں [آپریشنز دستی] (operations-playbook.md))۔