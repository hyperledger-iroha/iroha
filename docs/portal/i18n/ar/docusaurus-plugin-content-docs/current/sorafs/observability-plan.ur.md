---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/observability-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة المراقبة
العنوان: SoraFS إمكانية الملاحظة وخطة SLO
Sidebar_label: إمكانية الملاحظة وSLOs
الوصف: بوابات SoraFS والعقد والمنسق متعدد المصادر لمخطط القياس عن بعد ولوحات المعلومات وسياسة ميزانية الخطأ.
---

:::ملاحظة مستند ماخذ
هذه الصفحة `docs/source/sorafs_observability_plan.md` ستساعدك على حل المشكلة. لم يكتمل انتقال برانا أبو الهول بعد الآن إلى ما هو أبعد من ذلك.
:::

## الأهداف
- البوابات والعقد والمنسق متعدد المصادر التي تستخدم المقاييس والأحداث المنظمة.
- Grafana لوحات المعلومات وحدود التنبيه وخطافات التحقق من الصحة.
- سياسات الخطأ في الميزانية والحفر في الفوضى SLO تستهدف القائمة.

## كتالوج متري

### أسطح البوابة| متري | اكتب | التسميات | ملاحظات |
|--------|------|--------|-------|
| `sorafs_gateway_active` | مقياس (UpDownCounter) | `endpoint`، `method`، `variant`، `chunker`، `profile` | `SorafsGatewayOtel` ينبعث منها ذريعة؛ تم إنشاء نقطة النهاية/الطريقة لعمليات HTTP على متن الطائرة. |
| `sorafs_gateway_responses_total` | عداد | `endpoint`، `method`، `variant`، `chunker`، `profile`، `result`، `status`، `error_code` | ہر مکمل بوابة طلب زيادة شريطية ہوتی ہے؛ `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | الرسم البياني | `endpoint`، `method`، `variant`، `chunker`، `profile`، `result`، `status`، `error_code` | استجابات البوابة هي زمن الوصول من الوقت إلى البايت الأول؛ Prometheus `_bucket/_sum/_count` للتصدير. |
| `sorafs_gateway_proof_verifications_total` | عداد | `profile_version`، `result`، `error_code` | طلب الوقت پر إثبات نتائج التحقق من التقاط کیے جاتے ہیں (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | الرسم البياني | `profile_version`، `result`، `error_code` | إيصالات PoR لتوزيع زمن الوصول للتحقق. || `telemetry::sorafs.gateway.request` | حدث منظم | `endpoint`، `method`، `variant`، `result`، `status`، `error_code`، `duration_ms` | قم بطلب الإكمال من خلال انبعاث السجل المنظم وارتباط Loki/Tempo. |

أحداث `telemetry::sorafs.gateway.request` عدادات OTEL التي تعكس الحمولات النافعة المنظمة، ارتباط Loki/Tempo لـ `endpoint`، `method`، `variant`، يتم استخدام `status` و`error_code` و`duration_ms` لتتبع لوحات المعلومات SLO لسلسلة OTLP.

### القياس عن بعد لإثبات الصحة| متري | اكتب | التسميات | ملاحظات |
|--------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | عداد | `provider_id`، `trigger`، `penalty` | بالإضافة إلى `RecordCapacityTelemetry` أي `SorafsProofHealthAlert` ينبعث منها زيادة في الطاقة. `trigger` PDP/PoTR/كلا الفشلين فرق كرتا، قفز `penalty` دقات قطع جانبية حقيقية أو فترة تهدئة لا قمعها. |
| `torii_sorafs_proof_health_pdp_failures`، `torii_sorafs_proof_health_potr_breaches` | مقياس | `provider_id` | تعد نافذة القياس عن بعد المخالفة بمثابة تقرير تعداد PDP/PoTR الذي لا يتجاوز عدد مقدمي الخدمة السياسة. |
| `torii_sorafs_proof_health_penalty_nano` | مقياس | `provider_id` | تنبيه أخير لخفض قيمة Nano-XOR (فترة التهدئة وعدم فرض القمع حتى لا تكون صفرًا). |
| `torii_sorafs_proof_health_cooldown` | مقياس | `provider_id` | مقياس منطقي (`1` = تنبيه التهدئة لا يقمع هذا) حتى يتم كتم تنبيهات المتابعة عند كتم الصوت مرة أخرى. |
| `torii_sorafs_proof_health_window_end_epoch` | مقياس | `provider_id` | تنبيه نافذة القياس عن بعد لمشغلي العصر Norito المصنوعات اليدوية وارتباط كر سكيں. |

هناك خلاصات على لوحة معلومات عارض Taikai، وهي عبارة عن صف إثبات الصحة
(`dashboards/grafana/taikai_viewer.json`)، جس سے مشغلي CDN هم حجم التنبيه، مزيج مشغل PDP/PoTR، العقوبات وحالة التهدئة في مزود الرؤية المباشرة متعددة ہے۔هناك مقاييس باستخدام عارض Taikai تتضمن قواعد تنبيه للرياضة:
`SorafsProofHealthPenalty` لقد حان الوقت لإطلاق النار
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}`
تم إضافة 15 شهرًا إضافيًا، مع تحذير `SorafsProofHealthCooldown` مرة أخرى
مزود خدمة Cooldown متجدد. تنبيهات دونوں
`dashboards/alerts/taikai_viewer_rules.yml` متاح حتى SREs من PoR/PoTR
إنفاذ القانون في سياق محدد.

### أسطح المنسق| متري / حدث | اكتب | التسميات | منتج | ملاحظات |
|----------------|------|--------|---------|-------|
| `sorafs_orchestrator_active_fetches` | مقياس | `manifest_id`، `region` | `FetchMetricsCtx` | جلسات متوفرة على متن الطائرة. |
| `sorafs_orchestrator_fetch_duration_ms` | الرسم البياني | `manifest_id`، `region` | `FetchMetricsCtx` | الرسم البياني للمدة (ملي ثانية)؛ 1 مللي ثانية سے 30 ثانية دلاء۔ |
| `sorafs_orchestrator_fetch_failures_total` | عداد | `manifest_id`، `region`، `reason` | `FetchMetricsCtx` | الأسباب: `no_providers`، `no_healthy_providers`، `no_compatible_providers`، `exhausted_retries`، `observer_failed`، `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | عداد | `manifest_id`، `provider_id`، `reason` | `FetchMetricsCtx` | تؤدي إعادة المحاولة إلى حدوث فرق کرتا ہے (`retry`، `digest_mismatch`، `length_mismatch`، `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | عداد | `manifest_id`، `provider_id`، `reason` | `FetchMetricsCtx` | تلتقط سجلات التعطيل/الفشل على مستوى الجلسة کرتا ہے۔ |
| `sorafs_orchestrator_chunk_latency_ms` | الرسم البياني | `manifest_id`، `provider_id` | `FetchMetricsCtx` | لكل قطعة، جلب توزيع الكمون (ملي ثانية) الإنتاجية/تحليل SLO. |
| `sorafs_orchestrator_bytes_total` | عداد | `manifest_id`، `provider_id` | `FetchMetricsCtx` | البيان/المزود کے حساب سے وحدات البايت المسلمة؛ يعمل جهاز PromQL على `rate()` على زيادة الإنتاجية. |
| `sorafs_orchestrator_stalls_total` | عداد | `manifest_id`، `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms` تجاوز القطع المقطوعة. || `telemetry::sorafs.fetch.lifecycle` | حدث منظم | `manifest`، `region`، `job_id`، `event`، `status`، `chunk_count`، `total_bytes`، `provider_candidates`، `retry_budget`، `global_parallel_limit` | `FetchTelemetryCtx` | دورة حياة الوظيفة (البدء/الإكمال) الحمولة Norito JSON تعمل كمرآة متطابقة. |
| `telemetry::sorafs.fetch.retry` | حدث منظم | `manifest`، `region`، `job_id`، `provider`، `reason`، `attempts` | `FetchTelemetryCtx` | خط إعادة محاولة الموفر کے لیے ينبعث منها ہوتا ہے؛ `attempts` عمليات إعادة المحاولة التزايدية رقم کرتا ہے (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | حدث منظم | `manifest`، `region`، `job_id`، `provider`، `reason`، `failures` | `FetchTelemetryCtx` | يجب أن يتم تجاوز عتبة فشل الموفر. |
| `telemetry::sorafs.fetch.error` | حدث منظم | `manifest`، `region`، `job_id`، `reason`، `provider?`، `provider_reason?`، `duration_ms` | `FetchTelemetryCtx` | سجل الفشل النهائي، ابتلاع Loki/Splunk مناسب. |
| `telemetry::sorafs.fetch.stall` | حدث منظم | `manifest`، `region`، `job_id`، `provider`، `latency_ms`، `bytes` | `FetchTelemetryCtx` | الحد الأقصى لتكوين زمن الاستجابة للقطعة هو انبعاث عدادات (عدادات المماطلة مرآة كرتا). |

### أسطح العقدة/النسخ المتماثل| متري | اكتب | التسميات | ملاحظات |
|--------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | الرسم البياني | `provider_id` | النسبة المئوية لاستخدام التخزين في الرسم البياني OTEL (`_bucket/_sum/_count` للتصدير). |
| `sorafs_node_por_success_total` | عداد | `provider_id` | لقطات المجدول تستمد عينات PoR ناجحة كعداد رتيب. |
| `sorafs_node_por_failure_total` | عداد | `provider_id` | عينات PoR الفاشلة هي عداد رتيب. |
| `torii_sorafs_storage_bytes_*`، `torii_sorafs_storage_por_*` | مقياس | `provider` | البايتات المستخدمة وعمق قائمة الانتظار وأعداد PoR على متن الطائرة هي أجهزة قياس Prometheus الموجودة. |
| `torii_sorafs_capacity_*`، `torii_sorafs_uptime_bps`، `torii_sorafs_por_bps` | مقياس | `provider` | قدرة الموفر/بيانات النجاح في وقت التشغيل. |
| `torii_sorafs_por_ingest_backlog`، `torii_sorafs_por_ingest_failures_total` | مقياس | `provider`، `manifest` | يتم عرض عمق الأعمال المتراكمة وعدادات الفشل التراكمي في استطلاع `/v2/sorafs/por/ingestion/{manifest}` للتصدير، وتغذية لوحة/تنبيه "PoR Stalls". |

### إثبات الاسترجاع في الوقت المناسب (PoTR) واتفاقية مستوى الخدمة (SLA).| متري | اكتب | التسميات | منتج | ملاحظات |
|--------|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | الرسم البياني | `tier`، `provider` | منسق PoTR | الموعد النهائي متأخر بالمللي ثانية (إيجابي = تم الوفاء به). |
| `sorafs_potr_failures_total` | عداد | `tier`، `provider`، `reason` | منسق PoTR | الأسباب: `expired`، `missing_proof`، `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | عداد | `provider`، `manifest_id`، `reason` | مراقب جيش تحرير السودان | جب تسليم قطعة SLO تفوت کرے تو النار ہوتا ہے (زمن الوصول، معدل النجاح). |
| `sorafs_chunk_sla_violation_active` | مقياس | `provider`، `manifest_id` | مراقب جيش تحرير السودان | يمكن تبديل المقياس المنطقي (0/1) في نافذة الاختراق النشطة. |

## أهداف SLO

- توفر البوابة غير الموثوق بها: **99.9%** (استجابات HTTP 2xx/304).
- TTFB P95 غير الموثوق به: الطبقة الساخنة ≥ 120 مللي ثانية، الطبقة الدافئة ≥ 300 مللي ثانية.
- معدل نجاح الإثبات: ≥ 99.5% في اليوم.
- نجاح المنسق (إكمال القطعة): ≥ 99%.

## لوحات المعلومات والتنبيهات1. **إمكانية مراقبة البوابة** (`dashboards/grafana/sorafs_gateway_observability.json`) — التوفر غير الموثوق به، TTFB P95، انهيار الرفض وفشل PoR/PoTR في مقاييس OTEL.
2. **صحة الأوركسترا** (`dashboards/grafana/sorafs_fetch_observability.json`) — التحميل متعدد المصادر، وإعادة المحاولة، وفشل الموفر، وانفجارات التوقف المفاجئة.
3. **SoraNet Privacy Metrics** (`dashboards/grafana/soranet_privacy_metrics.json`) — دلاء الترحيل مجهولة المصدر، ونوافذ القمع، وصحة المجمع مثل `soranet_privacy_last_poll_unixtime` و`soranet_privacy_collector_enabled` و`soranet_privacy_poll_errors_total{provider}`.

حزم التنبيه:

- `dashboards/alerts/sorafs_gateway_rules.yml` — توفر البوابة، TTFB، ارتفاعات دليل الفشل۔
- `dashboards/alerts/sorafs_fetch_rules.yml` - فشل المنسق/إعادة المحاولة/الأكشاك؛ يتم التحقق من صحة `scripts/telemetry/test_sorafs_fetch_alerts.sh` و`dashboards/alerts/tests/sorafs_fetch_rules.test.yml` و`dashboards/alerts/tests/soranet_privacy_rules.test.yml` و`dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — ارتفاعات خفض مستوى الخصوصية، وإنذارات القمع، واكتشاف خمول المجمع، وتنبيهات المجمع المعطل (`soranet_privacy_last_poll_unixtime`، `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — إنذار انقطاع التيار الكهربائي لإخفاء الهوية جو `sorafs_orchestrator_brownouts_total` سلكي.
- `dashboards/alerts/taikai_viewer_rules.yml` — تنبيهات انجراف عارض Taikai/الاستيعاب/CEK المتأخرة جديدة SoraFS تنبيهات الجزاء الصحي/التبريد `torii_sorafs_proof_health_*` تعمل بالطاقة.

## استراتيجية التتبع- القياس عن بعد المفتوح من طرف إلى طرف:
  - بوابات امتدادات OTLP (HTTP) تُصدر معرفات الطلب وملخصات البيان وتجزئات الرمز المميز.
  - المنسق `tracing` + `opentelemetry` استخدام محاولات الجلب التي تمتد إلى تصدير کرتا ہے۔
  - عقد SoraFS المضمنة تحديات PoR وعمليات التخزين التي تمتد إلى التصدير. يتم نشر جميع المكونات `x-sorafs-trace` من خلال مشاركة معرف التتبع المشترك ومشاركة المعلومات.
- تعمل مقاييس المنسق `SorafsFetchOtel` مع الرسوم البيانية لـ OTLP على جسر وواجهة `telemetry::sorafs.fetch.*` للواجهات الخلفية المرتكزة على سجل الأحداث والتي توفر حمولات JSON خفيفة الوزن.
- جامعي: جامعي OTEL کو Prometheus/Loki/Tempo کے ساتھ چلایں (يفضل Tempo). اختير مصدرو Jaeger API.
- العمليات ذات الأهمية العالية كعينة (مسارات النجاح تصل إلى 10%، والفشل إلى 100%).

## تنسيق القياس عن بعد TLS (SF-5b)- المحاذاة المترية:
  - أتمتة TLS `sorafs_gateway_tls_cert_expiry_seconds`، `sorafs_gateway_tls_renewal_total{result}`، و`sorafs_gateway_tls_ech_enabled` بھیجتی.
  - تتضمن المقاييس الموجودة في لوحة معلومات نظرة عامة على البوابة لوحة TLS/الشهادات التي تتضمن معلومات.
- ربط التنبيه:
  - عند إطلاق تنبيهات انتهاء صلاحية TLS (أقل من 14 يومًا متبقية) يمكنك ربط SLO غير الموثوق به بالتوفر.
  - تعطيل ECH هو تنبيه ثانوي يصدر تنبيهًا وTLS وتوافر اللوحات المرجعية.
- خط الأنابيب: وظيفة أتمتة TLS مثل Prometheus مكدس تصدير الكرتا ومقاييس البوابة؛ SF-5b عبارة عن تنسيق ثابت للأجهزة المكررة للفتيات.

## اصطلاحات تسمية وتسمية المقاييس- أسماء المقاييس موجودة وهي البادئات `torii_sorafs_*` أو `sorafs_*` والتي تتبع البطاقة وTorii وتستخدم البوابة.
- مجموعات التسمية موحدة ہیں:
  - `result` → نتيجة HTTP (`success`، `refused`، `failed`).
  - `reason` → رمز الرفض/الخطأ (`unsupported_chunker`، `timeout`، وما إلى ذلك).
  - `provider` → معرف الموفر المشفر بالشفرة السداسية ۔
  - `manifest` → ملخص البيان القانوني (العناصر الأساسية العالية مزخرفة). 
  - `tier` → تسميات الطبقة التعريفية (`hot`، `warm`، `archive`).
- نقاط انبعاث القياس عن بعد:
  - مقاييس البوابة `torii_sorafs_*` التي تم إنشاؤها و `crates/iroha_core/src/telemetry.rs` تعيد استخدام الاتفاقيات.
  - مقاييس `sorafs_orchestrator_*` للمنسق وأحداث `telemetry::sorafs.fetch.*` (دورة الحياة، إعادة المحاولة، فشل الموفر، الخطأ، المماطلة) تنبعث منها كرتا وجن پر ملخص البيان، معرف الوظيفة، المنطقة وعلامات معرفات الموفر.
  - العقد `torii_sorafs_storage_*` و`torii_sorafs_capacity_*` و`torii_sorafs_por_*`.
- إمكانية الملاحظة والتنسيق المستمر من خلال الكتالوج المتري المشترك Prometheus تسمية الوثيقة يمكن تسجيلها، كما يمكن تسمية التوقعات الأساسية (المزود/إظهار الحدود العليا) شاملة.

## خط أنابيب البيانات- يقوم المجمِّعون بنشر المكونات باستمرار، وOTLP إلى Prometheus (المقاييس) وLoki/Tempo (السجلات/الآثار) لبطاقة التصدير.
- بوابات/عقد eBPF (Tetragon) الاختيارية للتتبع منخفض المستوى لإثراء البيانات.
- `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` إلى Torii والعقد المضمنة المستخدمة؛ المنسق `install_sorafs_fetch_otlp_exporter` هو من قام بالتسجيل.

## خطافات التحقق

- CI أثناء دوران `scripts/telemetry/test_sorafs_fetch_alerts.sh` قم بتشغيل Prometheus قواعد التنبيه توقف المقاييس وفحوصات قمع الخصوصية بشكل مستمر.
- Grafana لوحات المعلومات کو التحكم في الإصدار (`dashboards/grafana/`) کے تحت رکھیں وألواح میں تبدیلی پر لقطات/روابط اپڈیٹ کریں.
- تم تسجيل نتائج تدريبات الفوضى `scripts/telemetry/log_sorafs_drill.sh`؛ التحقق من صحة استخدام `scripts/telemetry/validate_drill_log.sh` (دیکھیے [دليل العمليات](operations-playbook.md)).