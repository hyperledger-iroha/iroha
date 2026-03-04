---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/observability-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: مشاہدہ کرنے والا منصوبہ
عنوان: SoraFS مشاہدہ کی منصوبہ بندی اور SLO
سائڈبار_لیبل: مشاہدہ اور سلو
تفصیل: ٹیلی میٹری اسکیم ، ڈیش بورڈز اور SoraFS گیٹ ویز ، NOS اور ملٹی سورس آرکیسٹریٹر کے لئے غلطی کے بجٹ کی پالیسی۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs_observability_plan.md` پر برقرار رکھے گئے منصوبے کی آئینہ دار ہے۔ دونوں کاپیاں ہم آہنگ رکھیں۔
:::

## مقاصد
- گیٹ ویز ، نوڈس اور ملٹی سورس آرکسٹریٹر کے لئے میٹرکس اور ساختی واقعات کی وضاحت کریں۔
- Grafana ڈیش بورڈز ، الرٹ دہلیز اور توثیق کے ہکس فراہم کریں۔
- غلطی کے بجٹ کی پالیسیاں اور افراتفری کی مشقوں کے ساتھ ایس ایل او مقاصد قائم کریں۔

## میٹرکس کیٹلاگ

### گیٹ وے سطحیں

| میٹرک | قسم | لیبل | نوٹ |
| --------- | ------ | -------- | ------- |
| `sorafs_gateway_active` | گیج (اپ ڈیٹون کاؤنٹر) | `endpoint` ، `method` ، `variant` ، `chunker` ، `profile` | `SorafsGatewayOtel` کے ذریعے جاری کیا گیا ؛ اختتامی نقطہ/طریقہ کے امتزاج کے ذریعہ پرواز میں HTTP آپریشنز کو ٹریک کرتا ہے۔ |
| `sorafs_gateway_responses_total` | کاؤنٹر | `endpoint` ، `method` ، `variant` ، `chunker` ، `profile` ، `result` ، `status` ، `error_code` | ہر ایک مکمل گیٹ وے کی درخواست میں ایک بار اضافہ ؛ `result` in {`success` ، `error` ، `dropped`}۔ |
| `sorafs_gateway_ttfb_ms_bucket` | ہسٹوگرام | `endpoint` ، `method` ، `variant` ، `chunker` ، `profile` ، `result` ، `status` ، `error_code` | گیٹ وے کے ردعمل کے لئے وقت سے پہلے بائٹ لیٹینسی ؛ Prometheus `_bucket/_sum/_count` کے بطور برآمد ہوا۔ |
| `sorafs_gateway_proof_verifications_total` | کاؤنٹر | `profile_version` ، `result` ، `error_code` | درخواست کے وقت (Prometheus ، `failure`}) کی درخواست کے وقت پکڑے گئے ثبوت کی توثیق کے نتائج۔ |
| `sorafs_gateway_proof_duration_ms_bucket` | ہسٹوگرام | `profile_version` ، `result` ، `error_code` | پور کی رسیدوں کے لئے توثیق میں تاخیر کی تقسیم۔ |
| `telemetry::sorafs.gateway.request` | ساختہ واقعہ | `endpoint` ، `method` ، `variant` ، `result` ، `status` ، `error_code` ، `duration_ms` | لوکی/وقت کے ارتباط کے لئے ہر درخواست کی تکمیل کے بعد تشکیل شدہ لاگ جاری کیا گیا۔ |
| `torii_sorafs_chunk_range_requests_total` ، `torii_sorafs_gateway_refusals_total` | کاؤنٹر | متبادل لیبل سیٹ | میٹرکس Prometheus تاریخی ڈیش بورڈز کے لئے برقرار ہے۔ نئی OTLP سیریز کے ساتھ مل کر جاری کیا گیا۔ |

`telemetry::sorafs.gateway.request` واقعات آئینہ اوٹیل کاؤنٹرز کے ساتھ سٹرکچرڈ پے لوڈز کے ساتھ ، `endpoint` ، `method` ، `variant` ، `status` ، `error_code` ، اور Prometheus ڈیش بورڈز ایس ایل او مانیٹرنگ کے لئے او ٹی ایل پی سیریز کا استعمال کرتے ہیں۔

### ٹیسٹ ہیلتھ ٹیلی میٹری| میٹرک | قسم | لیبل | نوٹ |
| --------- | ------ | -------- | ------- |
| `torii_sorafs_proof_health_alerts_total` | کاؤنٹر | `provider_id` ، `trigger` ، `penalty` | ہر بار `RecordCapacityTelemetry` ایک `SorafsProofHealthAlert` جاری کرتا ہے۔ `trigger` PDP/POTR/دونوں ناکامیوں کو ممتاز کرتا ہے ، جبکہ `penalty` نے گرفتاری کی ہے کہ آیا کولیٹرل کو واقعی کوولڈاؤن کے ذریعہ کاٹا یا دب گیا تھا۔ |
| `torii_sorafs_proof_health_pdp_failures` ، `torii_sorafs_proof_health_potr_breaches` | گیج | `provider_id` | گستاخانہ ٹیلی میٹری ونڈو کے اندر رپورٹ کردہ تازہ ترین PDP/POTR گنتی تاکہ ٹیمیں اس بات کا اندازہ کرسکیں کہ فراہم کنندگان نے پالیسی سے تجاوز کیا ہے۔ |
| `torii_sorafs_proof_health_penalty_nano` | گیج | `provider_id` | آخری الرٹ میں نانو زور کی قیمت کاٹ دی گئی (جب کولڈاؤن نے درخواست کو دبا دیا تو صفر)۔ |
| `torii_sorafs_proof_health_cooldown` | گیج | `provider_id` | بولین گیج (`1` = Couldown کے ذریعہ دبے ہوئے انتباہ) جب فالو اپ الرٹس عارضی طور پر خاموش ہوجاتے ہیں تو یہ ظاہر کرنے کے لئے ظاہر ہوتا ہے۔ |
| `torii_sorafs_proof_health_window_end_epoch` | گیج | `provider_id` | آپریٹرز کو Norito نمونے کے ساتھ وابستہ کرنے کے لئے الرٹ سے منسلک ٹیلی میٹری ونڈو کے لئے ریکارڈ کیا گیا ہے۔ |

یہ فیڈ اب تائیکائی ناظرین ڈیش بورڈ کی ٹیسٹ ہیلتھ لائن کو کھانا کھاتے ہیں
(`dashboards/grafana/taikai_viewer.json`) ، سی ڈی این آپریٹرز کو حقیقی وقت کی مرئیت دیتے ہوئے
الرٹ والیوم کے بارے میں ، PDP/POTR ٹرگر مکس ، جرمانے اور couldown کی حیثیت فی
فراہم کنندہ

اب وہی میٹرکس دو تائیکائی ناظرین کے انتباہ کے قواعد کی حمایت کرتی ہے۔
`SorafsProofHealthPenalty` جب سفر کرتا ہے
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` بڑھتا ہے
آخری 15 منٹ ، جبکہ `SorafsProofHealthCooldown` ایک انتباہ جاری کرتا ہے اگر a
فراہم کنندہ پانچ منٹ تک کوولڈون پر رہتا ہے۔ دونوں انتباہات زندہ رہتے ہیں
`dashboards/alerts/taikai_viewer_rules.yml` لہذا SREs فوری سیاق و سباق حاصل کرتے ہیں
جب POR/POTR ایپلی کیشن شدت اختیار کرتا ہے۔

### آرکسٹریٹر سطحیں| میٹرک / واقعہ | قسم | لیبل | پروڈیوسر | نوٹ |
| -------------------- | ------ | -------- | ---------- | ------- |
| `sorafs_orchestrator_active_fetches` | گیج | `manifest_id` ، `region` | `FetchMetricsCtx` | فی الحال پرواز میں سیشنز۔ |
| `sorafs_orchestrator_fetch_duration_ms` | ہسٹوگرام | `manifest_id` ، `region` | `FetchMetricsCtx` | ملی سیکنڈ میں دورانیہ ہسٹگرام ؛ 1 ایم ایس سے 30 سیکنڈ تک بالٹیاں۔ |
| `sorafs_orchestrator_fetch_failures_total` | کاؤنٹر | `manifest_id` ، `region` ، `reason` | `FetchMetricsCtx` | وجوہات: `no_providers` ، `no_healthy_providers` ، `no_compatible_providers` ، `exhausted_retries` ، `observer_failed` ، `internal_invariant`۔ |
| `sorafs_orchestrator_retries_total` | کاؤنٹر | `manifest_id` ، `provider_id` ، `reason` | `FetchMetricsCtx` | دوبارہ کوشش کرنے کی وجوہات (`retry` ، `digest_mismatch` ، `length_mismatch` ، `provider_error`) میں فرق کریں۔ |
| `sorafs_orchestrator_provider_failures_total` | کاؤنٹر | `manifest_id` ، `provider_id` ، `reason` | `FetchMetricsCtx` | سیشن کی سطح پر معذوری اور ناکامی کی گنتی کو پکڑتا ہے۔ |
| `sorafs_orchestrator_chunk_latency_ms` | ہسٹوگرام | `manifest_id` ، `provider_id` | `FetchMetricsCtx` | ان پٹ/ایس ایل او تجزیہ کے لئے فی حصہ (ایم ایس) کے مطابق لیٹینسی ڈسٹری بیوشن کو بازیافت کریں۔ |
| `sorafs_orchestrator_bytes_total` | کاؤنٹر | `manifest_id` ، `provider_id` | `FetchMetricsCtx` | منشور/فراہم کنندہ کے ذریعہ فراہم کردہ بائٹس ؛ `rate()` کے ذریعے تھروپپٹ حاصل کریں۔ |
| `sorafs_orchestrator_stalls_total` | کاؤنٹر | `manifest_id` ، `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms` سے تجاوز کرنے والے حصوں کا شمار کریں۔ |
| `telemetry::sorafs.fetch.lifecycle` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `event` ، `status` ، `chunk_count` ، SoraFS ، `provider_candidates` ، `provider_candidates` ، `provider_candidates` ، `provider_candidates` ، `provider_candidates` | `FetchTelemetryCtx` | JSON پے لوڈ Norito کے ساتھ جاب لائف سائیکل (اسٹارٹ/مکمل) کا آئینہ دار ہے۔ |
| `telemetry::sorafs.fetch.retry` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `provider` ، `reason` ، `attempts` | `FetchTelemetryCtx` | فراہم کنندہ کے ذریعہ دوبارہ کوششوں کے ذریعہ جاری کیا گیا۔ `attempts` میں اضافہ کی کوششیں (> = 1)۔ |
| `telemetry::sorafs.fetch.provider_failure` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `provider` ، `reason` ، `failures` | `FetchTelemetryCtx` | شائع ہوا جب کوئی فراہم کنندہ ناکامی کی دہلیز کو عبور کرتا ہے۔ |
| `telemetry::sorafs.fetch.error` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `reason` ، `provider?` ، `provider_reason?` ، `duration_ms` | `FetchTelemetryCtx` | ٹرمینل کی ناکامی لاگنگ ، لوکی/اسپلنک انجشن کے لئے دوستانہ۔ |
| `telemetry::sorafs.fetch.stall` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `provider` ، `latency_ms` ، `bytes` | `FetchTelemetryCtx` | خارج ہونے پر خارج ہونے والے تاخیر سے تشکیل شدہ حد (آئینے کے اسٹال کاؤنٹرز) سے زیادہ ہوجاتی ہے۔ |

### نہیں/نقل کی سطحیں| میٹرک | قسم | لیبل | نوٹ |
| --------- | ------ | -------- | ------- |
| `sorafs_node_capacity_utilisation_pct` | ہسٹوگرام | `provider_id` | اسٹوریج کے استعمال کی فیصد کا اوٹیل ہسٹگرام (`_bucket/_sum/_count` کے بطور برآمد)۔ |
| `sorafs_node_por_success_total` | کاؤنٹر | `provider_id` | کامیاب پور نمونوں کا مونوٹون کاؤنٹر ، جو شیڈولر اسنیپ شاٹس سے اخذ کیا گیا ہے۔ |
| `sorafs_node_por_failure_total` | کاؤنٹر | `provider_id` | ناکام مونوٹون پور نمونہ کاؤنٹر۔ |
| `torii_sorafs_storage_bytes_*` ، `torii_sorafs_storage_por_*` | گیج | `provider` | استعمال شدہ بائٹس کے لئے موجودہ Prometheus گیجز ، قطار کی گہرائی ، ان فلائٹ پور گنتی کے لئے۔ |
| `torii_sorafs_capacity_*` ، `torii_sorafs_uptime_bps` ، `torii_sorafs_por_bps` | گیج | `provider` | صلاحیت کے ڈیش بورڈ میں کامیاب فراہم کنندہ کی گنجائش/اپ ٹائم ڈیٹا ظاہر ہوتا ہے۔ |
| `torii_sorafs_por_ingest_backlog` ، `torii_sorafs_por_ingest_failures_total` | گیج | `provider` ، `manifest` | جب بھی `/v1/sorafs/por/ingestion/{manifest}` "پور اسٹالز" پینل/الرٹ کو کھانا کھلایا جاتا ہے تو بیک بلاگ گہرائی کے علاوہ جمع غلطی کاؤنٹرز برآمد ہوتے ہیں۔ |

### بروقت بازیافت (POTR) اور حصہ SLA کا ثبوت

| میٹرک | قسم | لیبل | پروڈیوسر | نوٹ |
| --------- | ------ | -------- | ---------- | ------- |
| `sorafs_potr_deadline_ms` | ہسٹوگرام | `tier` ، `provider` | پوٹ کوآرڈینیٹر | ملی سیکنڈ میں آخری تاریخ کا فرق (مثبت = میٹ)۔ |
| `sorafs_potr_failures_total` | کاؤنٹر | `tier` ، `provider` ، `reason` | پوٹ کوآرڈینیٹر | وجوہات: `expired` ، `missing_proof` ، `corrupt_proof`۔ |
| `sorafs_chunk_sla_violation_total` | کاؤنٹر | `provider` ، `manifest_id` ، `reason` | SLA مانیٹر | جب ایس ایل او (تاخیر ، کامیابی کی شرح) میں حصہ کی ترسیل ناکام ہوجاتی ہے تو متحرک۔ |
| `sorafs_chunk_sla_violation_active` | گیج | `provider` ، `manifest_id` | SLA مانیٹر | فعال خلاف ورزی ونڈو کے دوران بولین گیج (0/1) نے ٹوگل کیا۔ |

## ایس ایل او مقاصد

- بے اعتماد گیٹ وے کی دستیابی: ** 99.9 ٪ ** (HTTP 2XX/304 جوابات)۔
- TTFB P95 بے اعتماد: گرم ٹائر  = 99.5 ٪ فی دن۔
- آرکیسٹریٹر کامیابی (حصہ کی تکمیل):> = 99 ٪۔

## ڈیش بورڈز اور الرٹس

1. ** گیٹ وے مشاہدہ ** (`dashboards/grafana/sorafs_gateway_observability.json`) - بے اعتماد دستیابی ، TTFB P95 ، OTEL میٹرکس کے ذریعہ انکار اور POR/POTR کی ناکامیوں کی تفصیلات۔
2.
3.

الرٹ پیکیجز:- `dashboards/alerts/sorafs_gateway_rules.yml` - گیٹ وے کی دستیابی ، TTFB ، ٹیسٹ کی ناکامی کے اسپائکس۔
- `dashboards/alerts/sorafs_fetch_rules.yml` - آرکسٹریٹر کی ناکامی/دوبارہ کوششیں/اسٹال ؛ `scripts/telemetry/test_sorafs_fetch_alerts.sh` ، `dashboards/alerts/tests/sorafs_fetch_rules.test.yml` ، `dashboards/alerts/tests/soranet_privacy_rules.test.yml` اور `dashboards/alerts/tests/soranet_policy_rules.test.yml` کے ذریعے توثیق شدہ۔
- `dashboards/alerts/soranet_privacy_rules.yml` - رازداری کے انحطاط کے سپائکس ، دبانے والے الارم ، بیکار کلکٹر کا پتہ لگانے اور غیر فعال کلکٹر الرٹس (`soranet_privacy_last_poll_unixtime` ، `soranet_privacy_collector_enabled`)۔
- `dashboards/alerts/soranet_policy_rules.yml` - گمنامی براؤن آؤٹ الارمز `sorafs_orchestrator_brownouts_total` سے منسلک ہیں۔
- `dashboards/alerts/taikai_viewer_rules.yml` - TAIKAI ناظرین سے بڑھنے/انجسٹ/CEK وقفہ کے علاوہ SoraFS ریس سے `torii_sorafs_proof_health_*` سے چلنے والی نئی صحت کے پنلٹی/کولڈاؤن الرٹس۔

## ٹریسنگ حکمت عملی

-آخر سے آخر تک اوپینٹیلمیٹری کو اپنائیں:
  - گیٹ ویز ایٹ ایل پی (HTTP) کی درخواستوں کی IDs ، ظاہر ہضموں ، اور ٹوکن ہیشوں کے ساتھ تشریح کی گئی۔
  - آرکیسٹریٹر `tracing` + `opentelemetry` برآمد کرنے کی کوششوں کو برآمد کرنے کے لئے استعمال کرتا ہے۔
  - POR چیلنجوں اور اسٹوریج آپریشنز کے لئے بلٹ میں SoraFS برآمد اسپینز۔ تمام اجزاء `x-sorafs-trace` کے ذریعے پھیلائے گئے مشترکہ ٹریس ID کا اشتراک کرتے ہیں۔
- `SorafsFetchOtel` آرکسٹریٹر میٹرکس کو OTLP ہسٹوگرام سے لنک کرتا ہے جبکہ `telemetry::sorafs.fetch.*` واقعات لاگ سینٹرک بیک اینڈ کے لئے ہلکا پھلکا JSON پے لوڈ فراہم کرتے ہیں۔
- جمع کرنے والے: Prometheus/LOKI/TEMPO (ترجیحی ٹیمپو) کے ساتھ ساتھ OTEL جمع کرنے والے چلائیں۔ جیگر کے مطابق برآمد کنندگان اختیاری ہیں۔
- اعلی کارڈنلٹی آپریشنوں کو نمونہ بنانا ضروری ہے (کامیاب راستوں کے لئے 10 ٪ ، ناکامیوں کے لئے 100 ٪)۔

## TLS ٹیلی میٹری کوآرڈینیشن (SF-5B)

- میٹرکس کی سیدھ:
  - TLS آٹومیشن `sorafs_gateway_tls_cert_expiry_seconds` ، `sorafs_gateway_tls_renewal_total{result}` اور `sorafs_gateway_tls_ech_enabled` بھیجتا ہے۔
  - TLS/سرٹیفکیٹ ڈیش بورڈ کے تحت گیٹ وے جائزہ ڈیش بورڈ میں ان گیجز کو شامل کریں۔
- الرٹ لنک:
  - جب TLS میعاد ختم ہونے سے آگ لگ جاتی ہے (<= 14 دن باقی) تو ، بے اعتماد دستیابی SLO سے وابستہ ہوں۔
  - غیر فعال کرنے سے ای سی ایچ ٹی ٹی ایل ایس اور دستیابی پینلز دونوں کا حوالہ دیتے ہوئے ثانوی انتباہ جاری کرتا ہے۔
- پائپ لائن: TLS آٹومیشن جاب Prometheus کو اسی اسٹیک میں گیٹ وے میٹرکس کی طرح برآمد کرتا ہے۔ SF-5B کے ساتھ کوآرڈینیشن کٹوتی کے آلے کی ضمانت دیتا ہے۔

## میٹرک نام اور لیبلنگ کنونشنز- میٹرک کے نام موجودہ سابقہ ​​`torii_sorafs_*` یا `sorafs_*` کی پیروی کرتے ہیں Torii اور گیٹ وے کے ذریعہ استعمال کیا جاتا ہے۔
- لیبل سیٹ معیاری ہیں:
  - `result` -> HTTP نتیجہ (`success` ، `refused` ، `failed`)۔
  - `reason` -> انکار/غلطی کا کوڈ (`unsupported_chunker` ، `timeout` ، وغیرہ)۔
  - `provider` -> ہیکس انکوڈڈ فراہم کنندہ شناخت کنندہ۔
  - `manifest` -> منشور کی کیننیکل ہاضم (جب کارڈینلٹی زیادہ ہو تو کاٹ دیں)۔
  - `tier` -> اعلامیہ درجے کے لیبل (`hot` ، `warm` ، `archive`)۔
- ٹیلی میٹری کے اخراج پوائنٹس:
  - گیٹ وے میٹرکس `torii_sorafs_*` کے تحت رہتے ہیں اور `crates/iroha_core/src/telemetry.rs` سے کنونشنوں کو دوبارہ استعمال کرتے ہیں۔
  - آرکیسٹریٹر میٹرکس `sorafs_orchestrator_*` اور واقعات `telemetry::sorafs.fetch.*` (لائف سائیکل ، دوبارہ کوشش ، فراہم کنندہ کی ناکامی ، غلطی ، اسٹال) مینی فیسٹ ڈائجسٹ ، جاب ID ، خطے اور فراہم کنندہ شناخت کنندگان کے ساتھ ٹیگ کردہ خارج کرتا ہے۔
  - وہ ہمیں `torii_sorafs_storage_*` ، `torii_sorafs_capacity_*` اور `torii_sorafs_por_*` پر بے نقاب کرتے ہیں۔
- مشترکہ نام دستاویز Prometheus میں میٹرکس کیٹلاگ کو ریکارڈ کرنے کے مشاہدہ کے ساتھ ہم آہنگی ، جس میں لیبل کارڈنلٹی کی توقعات (فراہم کنندہ/مینی فیسٹ اوپری حدود) شامل ہیں۔

## ڈیٹا پائپ لائن

- جمع کرنے والوں کو ہر جزو کے ساتھ نافذ کیا جاتا ہے ، Prometheus (میٹرکس) اور لوکی/ٹیمپو (لاگ/ٹریس) کو OTLP برآمد کرتے ہیں۔
- اختیاری ای بی پی ایف (ٹیٹراگون) گیٹ ویز/نوڈس کے لئے کم سطح کا ٹریسنگ کو افزودہ کرتا ہے۔
- Torii اور بلٹ ان میں `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` استعمال کریں۔ آرکسٹریٹر `install_sorafs_fetch_otlp_exporter` پر کال کرتا رہتا ہے۔

## توثیق ہکس

- CI کے دوران `scripts/telemetry/test_sorafs_fetch_alerts.sh` چلائیں تاکہ یہ یقینی بنایا جاسکے کہ Prometheus الرٹ قواعد اسٹال میٹرکس اور رازداری کے دباؤ کی جانچ پڑتال کے ساتھ ہم آہنگی میں رہیں۔
- Grafana ڈیش بورڈز کو ورژن کنٹرول (`dashboards/grafana/`) کے تحت رکھیں اور جب ڈیش بورڈز تبدیل ہوتے ہیں تو اسکرین شاٹس/لنکس کو اپ ڈیٹ کریں۔
- افراتفری کی مشق `scripts/telemetry/log_sorafs_drill.sh` کے ذریعے لاگ نتائج ؛ توثیق `scripts/telemetry/validate_drill_log.sh` (دیکھیں [آپریشنز پلے بوک] (operations-playbook.md))۔