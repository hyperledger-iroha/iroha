---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/observability-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: مشاہدہ کرنے والا منصوبہ
عنوان: SoraFS مشاہدہ کی منصوبہ بندی اور SLO
سائڈبار_لیبل: مشاہدہ اور سلو
تفصیل: ٹیلی میٹری اسکیما ، ڈیش بورڈز ، اور SoraFS گیٹ ویز ، نوڈس ، اور ملٹی سورس آرکیسٹریٹر کے لئے غلطی کے بجٹ کی پالیسی۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs_observability_plan.md` پر برقرار رکھے گئے منصوبے کی عکاسی کرتا ہے۔ دونوں کاپیاں مطابقت پذیری میں رکھیں جب تک کہ اسفینکس دستاویزات کا سیٹ مکمل طور پر ہجرت نہ ہوجائے۔
:::

## مقاصد
- گیٹ ویز ، نوڈس اور ملٹی سورس آرکسٹریٹر کے لئے میٹرکس اور ساختی واقعات کی وضاحت کریں۔
- Grafana ڈیش بورڈز ، الرٹ دہلیز اور توثیق کے ہکس فراہم کریں۔
- غلطی کے بجٹ کی پالیسیاں اور افراتفری کی مشقوں کے ساتھ ایس ایل او مقاصد قائم کریں۔

## میٹرکس کیٹلاگ

### گیٹ وے سطحیں

| میٹرک | قسم | ٹیگز | نوٹ |
| -------- | ------ | ----------- | ------- |
| `sorafs_gateway_active` | گیج (اپ ڈیٹون کاؤنٹر) | `endpoint` ، `method` ، `variant` ، `chunker` ، `profile` | `SorafsGatewayOtel` کے ذریعے جاری کیا گیا ؛ اختتامی نقطہ/طریقہ کے امتزاج کے ذریعہ پرواز میں HTTP آپریشنز کو ٹریک کرتا ہے۔ |
| `sorafs_gateway_responses_total` | کاؤنٹر | `endpoint` ، `method` ، `variant` ، `chunker` ، `profile` ، `result` ، `status` ، `error_code` | ہر ایک مکمل گیٹ وے کی درخواست میں ایک بار اضافہ ؛ `result` ∈ {`success` ، `error` ، `dropped`}۔ |
| `sorafs_gateway_ttfb_ms_bucket` | ہسٹوگرام | `endpoint` ، `method` ، `variant` ، `chunker` ، `profile` ، `result` ، `status` ، `error_code` | گیٹ وے کے ردعمل کے لئے وقت سے پہلے بائٹ لیٹینسی ؛ Prometheus `_bucket/_sum/_count` کے بطور برآمد ہوا۔ |
| `sorafs_gateway_proof_verifications_total` | کاؤنٹر | `profile_version` ، `result` ، `error_code` | ٹیسٹ کی توثیق کے نتائج درخواست کے وقت پکڑے گئے (`result` ∈ {`success` ، `failure`})۔ |
| `sorafs_gateway_proof_duration_ms_bucket` | ہسٹوگرام | `profile_version` ، `result` ، `error_code` | پور کی رسیدوں کے لئے توثیق میں تاخیر کی تقسیم۔ |
| `telemetry::sorafs.gateway.request` | ساختہ واقعہ | `endpoint` ، `method` ، `variant` ، `result` ، `status` ، `error_code` ، `duration_ms` | لوکی/ٹیمپو میں ارتباط کے لئے ہر درخواست کو مکمل کرنے پر سٹرکچرڈ لاگ جاری کیا گیا۔ |
| `torii_sorafs_chunk_range_requests_total` ، `torii_sorafs_gateway_refusals_total` | کاؤنٹر | میراثی لیبل سیٹ | Prometheus میٹرکس کو تاریخی ڈیش بورڈز کے لئے برقرار رکھا گیا ہے۔ نئی OTLP سیریز کے ساتھ مل کر جاری کیا گیا۔ |

واقعات `telemetry::sorafs.gateway.request` ساختی پے لوڈ کے ساتھ اوٹیل کاؤنٹرز کی عکاسی کرتے ہیں ، `endpoint` ، `method` ، `variant` ، `status` ، `error_code` اور Prometheus کے لئے `variant` ، `variant` ، `variant` ایس ایل او ٹریکنگ کے لئے او ٹی ایل پی سیریز کا استعمال کریں۔

### صحت ٹیلی میٹری کی جانچ کرنا| میٹرک | قسم | ٹیگز | نوٹ |
| -------- | ------ | ----------- | ------- |
| `torii_sorafs_proof_health_alerts_total` | کاؤنٹر | `provider_id` ، `trigger` ، `penalty` | اس میں ہر بار `RecordCapacityTelemetry` ایک `SorafsProofHealthAlert` جاری کیا جاتا ہے۔ `trigger` PDP/POTR/دونوں ناکامیوں کو ممتاز کرتا ہے ، جبکہ `penalty` نے گرفتاری کی ہے کہ آیا کولیٹرل کو واقعی تراش لیا گیا تھا یا کولڈاؤن کے ذریعہ حذف کیا گیا تھا۔ |
| `torii_sorafs_proof_health_pdp_failures` ، `torii_sorafs_proof_health_potr_breaches` | گیج | `provider_id` | ٹیموں کے لئے مجرمانہ ٹیلی میٹری ونڈو کے اندر حالیہ پی ڈی پی/پی او آر آر گنتی کی اطلاع دی گئی ہے تاکہ اس بات کا اندازہ کیا جاسکے کہ فراہم کنندگان نے پالیسی سے کتنا تجاوز کیا ہے۔ |
| `torii_sorafs_proof_health_penalty_nano` | گیج | `provider_id` | آخری انتباہ میں نینو زور کی رقم کاٹ دی گئی (جب صفر نے درخواست کو ہٹا دیا)۔ |
| `torii_sorafs_proof_health_cooldown` | گیج | `provider_id` | بولین گیج (`1` = Couldown کے ذریعہ دبے ہوئے انتباہ) ظاہر کرنے کے لئے جب ٹریکنگ الرٹس عارضی طور پر خاموش ہوجاتے ہیں۔ |
| `torii_sorafs_proof_health_window_end_epoch` | گیج | `provider_id` | آپریٹرز کو Norito نمونے سے وابستہ کرنے کے لئے الرٹ سے منسلک ٹیلی میٹری ونڈو کے لئے ریکارڈ شدہ عہد۔ |

یہ فیڈ اب تائیکائی ناظرین ڈیش بورڈ کی ٹیسٹ ہیلتھ قطار کو کھانا کھاتے ہیں
(`dashboards/grafana/taikai_viewer.json`) ، سی ڈی این آپریٹرز کو حقیقی وقت کی مرئیت دیتے ہوئے
الرٹ والیومز ، PDP/POTR ٹرگرز کا مرکب ، جرمانے اور کولڈاؤن کی حیثیت
فراہم کنندہ

اب وہی میٹرکس دو تائیکائی ناظرین کے انتباہ کے قواعد کی حمایت کرتی ہے۔
`SorafsProofHealthPenalty` جب متحرک ہے
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` بذریعہ بڑھتا ہے
آخری 15 منٹ ، جبکہ `SorafsProofHealthCooldown` ایک انتباہ پھینک دیتا ہے اگر a
فراہم کنندہ پانچ منٹ تک کولڈاؤن میں رہتا ہے۔ دونوں انتباہات میں رہتے ہیں
`dashboards/alerts/taikai_viewer_rules.yml` SREs کے لئے فوری سیاق و سباق حاصل کرنے کے لئے
جب POR/POTR ایپلی کیشن شدت اختیار کرتا ہے۔

### آرکسٹریٹر سطحیں| میٹرک / واقعہ | قسم | ٹیگز | پروڈیوسر | نوٹ |
| ------------------- | ------ | ----------- | ----------- | ------- |
| `sorafs_orchestrator_active_fetches` | گیج | `manifest_id` ، `region` | `FetchMetricsCtx` | فی الحال پرواز میں سیشنز۔ |
| `sorafs_orchestrator_fetch_duration_ms` | ہسٹوگرام | `manifest_id` ، `region` | `FetchMetricsCtx` | ملی سیکنڈ میں دورانیہ ہسٹگرام ؛ 1 ایم ایس سے 30 سیکنڈ تک بالٹیاں۔ |
| `sorafs_orchestrator_fetch_failures_total` | کاؤنٹر | `manifest_id` ، `region` ، `reason` | `FetchMetricsCtx` | وجوہات: `no_providers` ، `no_healthy_providers` ، `no_compatible_providers` ، `exhausted_retries` ، `observer_failed` ، `internal_invariant`۔ |
| `sorafs_orchestrator_retries_total` | کاؤنٹر | `manifest_id` ، `provider_id` ، `reason` | `FetchMetricsCtx` | دوبارہ کوشش کرنے والے وجوہات (`retry` ، `digest_mismatch` ، `length_mismatch` ، `provider_error`) میں فرق کرتا ہے۔ |
| `sorafs_orchestrator_provider_failures_total` | کاؤنٹر | `manifest_id` ، `provider_id` ، `reason` | `FetchMetricsCtx` | سیشن کی سطح پر غیر فعال اور حادثے کی گنتی پر قبضہ کرتا ہے۔ |
| `sorafs_orchestrator_chunk_latency_ms` | ہسٹوگرام | `manifest_id` ، `provider_id` | `FetchMetricsCtx` | ان پٹ/ایس ایل او تجزیہ کے لئے فی حصہ (ایم ایس) کے مطابق لیٹینسی ڈسٹری بیوشن کو بازیافت کریں۔ |
| `sorafs_orchestrator_bytes_total` | کاؤنٹر | `manifest_id` ، `provider_id` | `FetchMetricsCtx` | منشور/فراہم کنندہ کے ذریعہ فراہم کردہ بائٹس ؛ `rate()` کے ذریعے تھروپپٹ کو پروم کیو ایل میں اخذ کرتا ہے۔ |
| `sorafs_orchestrator_stalls_total` | کاؤنٹر | `manifest_id` ، `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms` سے تجاوز کرنے والے حصوں کا شمار کریں۔ |
| `telemetry::sorafs.fetch.lifecycle` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `event` ، `status` ، `chunk_count` ، SoraFS ، `provider_candidates` ، `provider_candidates` ، `provider_candidates` ، `provider_candidates` ، `provider_candidates` | `FetchTelemetryCtx` | پے لوڈ JSON Norito کے ساتھ ملازمت کے زندگی کے چکر (اسٹارٹ/تکمیل) کی عکاسی کرتا ہے۔ |
| `telemetry::sorafs.fetch.retry` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `provider` ، `reason` ، `attempts` | `FetchTelemetryCtx` | ہر فراہم کنندہ کی دوبارہ کوشش جاری ؛ `attempts` میں اضافہ کی کوششیں (≥ 1)۔ |
| `telemetry::sorafs.fetch.provider_failure` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `provider` ، `reason` ، `failures` | `FetchTelemetryCtx` | یہ شائع ہوتا ہے جب کوئی فراہم کنندہ ناکامی کی دہلیز کو عبور کرتا ہے۔ |
| `telemetry::sorafs.fetch.error` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `reason` ، `provider?` ، `provider_reason?` ، `duration_ms` | `FetchTelemetryCtx` | ٹرمینل کی ناکامی لاگ ، لوکی/اسپلنک میں ادخال کے لئے دوستانہ۔ |
| `telemetry::sorafs.fetch.stall` | ساختہ واقعہ | `manifest` ، `region` ، `job_id` ، `provider` ، `latency_ms` ، `bytes` | `FetchTelemetryCtx` | خارج ہونے پر خارج ہونے کی وجہ سے تشکیل شدہ حد سے زیادہ (اسٹال کاؤنٹرز کی عکاسی ہوتی ہے)۔ |

### نوڈ/نقل کی سطحیں| میٹرک | قسم | ٹیگز | نوٹ |
| -------- | ------ | ----------- | ------- |
| `sorafs_node_capacity_utilisation_pct` | ہسٹوگرام | `provider_id` | اسٹوریج کے استعمال کی فیصد کا اوٹیل ہسٹگرام (`_bucket/_sum/_count` کے بطور برآمد)۔ |
| `sorafs_node_por_success_total` | کاؤنٹر | `provider_id` | کامیاب پور نمونوں کا مونوٹونک کاؤنٹر ، جو شیڈولر اسنیپ شاٹس سے اخذ کیا گیا ہے۔ |
| `sorafs_node_por_failure_total` | کاؤنٹر | `provider_id` | ناکام پور نمونوں کا مونوٹونک کاؤنٹر۔ |
| `torii_sorafs_storage_bytes_*` ، `torii_sorafs_storage_por_*` | گیج | `provider` | استعمال شدہ بائٹس کے لئے موجودہ Prometheus گیجز ، قطار کی گہرائی ، ان فلائٹ پور گنتی کے لئے۔ |
| `torii_sorafs_capacity_*` ، `torii_sorafs_uptime_bps` ، `torii_sorafs_por_bps` | گیج | `provider` | سپلائر کی گنجائش/کامیاب اپ ٹائم ڈیٹا صلاحیت ڈیش بورڈ پر ظاہر ہوتا ہے۔ |
| `torii_sorafs_por_ingest_backlog` ، `torii_sorafs_por_ingest_failures_total` | گیج | `provider` ، `manifest` | بیک بلاگ کی گہرائی کے علاوہ جمع ہونے والے کریش کاؤنٹرز ہر بار برآمد کیے جاتے ہیں `/v2/sorafs/por/ingestion/{manifest}` سے استفسار کیا جاتا ہے ، جس سے "پور اسٹالز" پینل/الرٹ کھلایا جاتا ہے۔ |

### بروقت بازیافت (POTR) اور chunk SLA کا ثبوت

| میٹرک | قسم | ٹیگز | پروڈیوسر | نوٹ |
| -------- | ------ | ----------- | ----------- | ------- |
| `sorafs_potr_deadline_ms` | ہسٹوگرام | `tier` ، `provider` | پوٹ کوآرڈینیٹر | ملی سیکنڈ میں ڈیڈ لائن سلیک (مثبت = میٹ)۔ |
| `sorafs_potr_failures_total` | کاؤنٹر | `tier` ، `provider` ، `reason` | پوٹ کوآرڈینیٹر | وجوہات: `expired` ، `missing_proof` ، `corrupt_proof`۔ |
| `sorafs_chunk_sla_violation_total` | کاؤنٹر | `provider` ، `manifest_id` ، `reason` | SLA مانیٹر | جب کام کی ترسیل ایس ایل او (تاخیر ، کامیابی کی شرح) کی خلاف ورزی کرتی ہے تو متحرک۔ |
| `sorafs_chunk_sla_violation_active` | گیج | `provider` ، `manifest_id` | SLA مانیٹر | بولین گیج (0/1) فعال ڈیفالٹ ونڈو کے دوران ٹوگل ہوا۔ |

## ایس ایل او مقاصد

- بے اعتماد گیٹ وے کی دستیابی: ** 99.9 ٪ ** (HTTP جوابات 2xx/304)۔
- TTFB P95 بے اعتماد: گرم ٹائر ≤ 120 ایم ایس ، گرم ٹائر ≤ 300 ایم ایس۔
- کامیابی کی شرح کی جانچ: .5 99.5 ٪ فی دن۔
- آرکیسٹریٹر کامیابی (حصہ کی تکمیل): ≥ 99 ٪۔

## ڈیش بورڈز اور الرٹس

1.
2.
3.

الرٹ پیکیجز:- `dashboards/alerts/sorafs_gateway_rules.yml` - گیٹ وے کی دستیابی ، TTFB ، چوٹی ٹیسٹ کی ناکامی۔
- `dashboards/alerts/sorafs_fetch_rules.yml` - آرکسٹریٹر کی ناکامی/دوبارہ کوششیں/اسٹال ؛ `scripts/telemetry/test_sorafs_fetch_alerts.sh` ، `dashboards/alerts/tests/sorafs_fetch_rules.test.yml` ، `dashboards/alerts/tests/soranet_privacy_rules.test.yml` اور `dashboards/alerts/tests/soranet_policy_rules.test.yml` کے ذریعے توثیق شدہ۔
- `dashboards/alerts/soranet_privacy_rules.yml` - رازداری کے انحطاط کے سپائکس ، دبانے والے الارم ، بیکار کلکٹر کا پتہ لگانے ، اور غیر فعال کلکٹر الرٹس (`soranet_privacy_last_poll_unixtime` ، `soranet_privacy_collector_enabled`)۔
- `dashboards/alerts/soranet_policy_rules.yml` - `sorafs_orchestrator_brownouts_total` سے منسلک گمنامی براؤن آؤٹ الارمز۔
- `dashboards/alerts/taikai_viewer_rules.yml` - تائیکائی ناظرین کا بہاؤ/انجشن/سی ای کے لیگ الارمز کے علاوہ نیا ٹیسٹ ہیلتھ جرمانہ/کوولڈون الرٹس SoraFS `torii_sorafs_proof_health_*` کے ذریعہ طاقت سے۔

## ٹریس حکمت عملی

-اوپن لیمٹری کو اختتام سے آخر میں اپنائیں:
  - گیٹ ویز جاری کرتے ہیں OTLP (HTTP) درخواست کی IDs ، ظاہر ہضموں ، اور ٹوکن ہیشوں کے ساتھ تشریح کی گئی۔
  - آرکیسٹریٹر `tracing` + `opentelemetry` کا استعمال بازیافت کی کوششوں کے دوران برآمد کرنے کے لئے استعمال کرتا ہے۔
  - ایمبیڈڈ SoraFS نوڈس برآمدی اسپینز برائے پور چیلنجز اور اسٹوریج آپریشنز۔ تمام اجزاء `x-sorafs-trace` کے ذریعے پھیلائے گئے مشترکہ ٹریس ID کا اشتراک کرتے ہیں۔
- `SorafsFetchOtel` آرکسٹریٹر میٹرکس کو OTLP ہسٹوگرام سے جوڑتا ہے جبکہ `telemetry::sorafs.fetch.*` واقعات لاگ سینٹرک بیک اینڈ کے لئے ہلکا پھلکا JSON پے لوڈ فراہم کرتے ہیں۔
- جمع کرنے والے: Prometheus/LOKI/TEMPO (ترجیحی ٹیمپو) کے ساتھ ساتھ OTEL جمع کرنے والے چلاتے ہیں۔ جیگر API برآمد کنندگان اختیاری ہیں۔
- اعلی کارڈنلٹی آپریشنز کا نمونہ لیا جانا چاہئے (کامیابی کے راستوں کے لئے 10 ٪ ، ناکامیوں کے لئے 100 ٪)۔

## TLS ٹیلی میٹری کوآرڈینیشن (SF-5B)

- میٹرکس سیدھ:
  - TLS آٹومیشن `sorafs_gateway_tls_cert_expiry_seconds` ، `sorafs_gateway_tls_renewal_total{result}` اور `sorafs_gateway_tls_ech_enabled` بھیجتا ہے۔
  - TLS/سرٹیفکیٹ پینل کے تحت گیٹ وے جائزہ ڈیش بورڈ میں ان گیجز کو شامل کریں۔
- لنکنگ الرٹس:
  - جب ٹی ایل ایس کی میعاد ختم ہونے والے انتباہات کو متحرک کیا جاتا ہے (≤ 14 دن باقی) ، تو اعتماد کے بغیر دستیابی ایس ایل او سے وابستہ ہوں۔
  - غیر فعال کرنا ایک ثانوی انتباہ جاری کرتا ہے جو TLS اور دستیابی کے پینل دونوں کا حوالہ دیتا ہے۔
- پائپ لائن: TLS آٹومیشن ملازمت گیٹ وے میٹرکس کی طرح اسی Prometheus اسٹیک میں برآمد کرتی ہے۔ SF-5B کے ساتھ کوآرڈینیشن کٹوتی کے آلے کو یقینی بناتا ہے۔

## میٹرک نام اور لیبلنگ کنونشنز- میٹرک کے نام موجودہ سابقہ ​​`torii_sorafs_*` یا `sorafs_*` کی پیروی کرتے ہیں Torii اور گیٹ وے کے ذریعہ استعمال کیا جاتا ہے۔
- لیبل سیٹ معیاری ہیں:
  - `result` → HTTP نتیجہ (`success` ، `refused` ، `failed`)۔
  - `reason` → مسترد/غلطی کا کوڈ (`unsupported_chunker` ، `timeout` ، وغیرہ)۔
  - `provider` → ہیکس انکوڈڈ سپلائر شناخت کنندہ۔
  - `manifest` → منشور کینونیکل ڈائجسٹ (جب اعلی کارڈنلٹی ہو تو تراشے ہوئے)۔
  - `tier` → اعلامیہ درجے کے لیبل (`hot` ، `warm` ، `archive`)۔
- ٹیلی میٹری کے اخراج پوائنٹس:
  - گیٹ وے میٹرکس `torii_sorafs_*` کے تحت رہتے ہیں اور `crates/iroha_core/src/telemetry.rs` سے کنونشنوں کو دوبارہ استعمال کرتے ہیں۔
  - آرکیسٹریٹر میٹرکس `sorafs_orchestrator_*` اور واقعات `telemetry::sorafs.fetch.*` (لائف سائیکل ، دوبارہ کوشش ، فراہم کنندہ کی ناکامی ، غلطی ، اسٹال) مینی فیسٹ ڈائجسٹ ، جاب ID ، خطے اور فراہم کنندہ شناخت کنندگان کے ساتھ ٹیگ کردہ خارج کرتا ہے۔
  - نوڈس `torii_sorafs_storage_*` ، `torii_sorafs_capacity_*` اور `torii_sorafs_por_*` کو بے نقاب کرتے ہیں۔
- مشترکہ نام دستاویز Prometheus میں میٹرکس کیٹلاگ کو رجسٹر کرنے کے مشاہدے کے ساتھ کوآرڈینیٹ ، بشمول لیبل کارڈنلٹی کی توقعات (وینڈر/اوپری حدود کو ظاہر کرتا ہے)۔

## ڈیٹا پائپ لائن

- جمع کرنے والے ہر جزو کے ساتھ دکھائے جاتے ہیں ، Prometheus (میٹرکس) اور لوکی/ٹیمپو (لاگ/ٹریسز) میں OTLP برآمد کرتے ہیں۔
- اختیاری ای بی پی ایف (ٹیٹراگون) گیٹ ویز/نوڈس کے لئے کم سطح کا ٹریسنگ کو افزودہ کرتا ہے۔
- Torii اور ایمبیڈڈ نوڈس کے لئے `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` استعمال کریں۔ آرکسٹریٹر `install_sorafs_fetch_otlp_exporter` پر کال کرنا جاری رکھے ہوئے ہے۔

## توثیق ہکس

- CI کے دوران `scripts/telemetry/test_sorafs_fetch_alerts.sh` چلائیں Prometheus الرٹ کے قواعد اسٹال میٹرکس اور رازداری کے دبانے کی جانچ پڑتال کے مطابق رہیں۔
- Grafana ڈیش بورڈز کو ورژن کنٹرول (`dashboards/grafana/`) کے تحت رکھیں اور جب پینل تبدیل ہوتے ہیں تو اسکرین شاٹس/لنکس کو اپ ڈیٹ کریں۔
- افراتفری کی مشق `scripts/telemetry/log_sorafs_drill.sh` کے ذریعے ریکارڈ کے نتائج ؛ توثیق `scripts/telemetry/validate_drill_log.sh` (دیکھیں [آپریشنز پلے بوک] (operations-playbook.md))۔