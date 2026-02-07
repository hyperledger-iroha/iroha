---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/observability-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة المراقبة
العنوان: Plan de observabilidad y SLO de SoraFS
Sidebar_label: إمكانية المراقبة وSLOs
الوصف: مقياس القياس عن بعد ولوحات المعلومات وافتراض الخطأ السياسي للبوابات SoraFS والعقد والمنظم متعدد الوظائف.
---

:::ملاحظة فوينتي كانونيكا
تعرض هذه الصفحة الخطة المحتفظ بها في `docs/source/sorafs_observability_plan.md`. حافظ على النسخ المتزامنة حتى يتم نقل مجموعة وثائق أبو الهول بالكامل.
:::

##الأهداف
- تحديد المقاييس والأحداث المصممة للبوابات والعقد والمنسق متعدد الوظائف.
- إثبات لوحات المعلومات Grafana ومظلات التنبيه وخطافات التحقق من الصحة.
- تحديد أهداف SLO جنبًا إلى جنب مع سياسات افتراض الخطأ والتدريبات على الأخطاء.

## كتالوج المقاييس

### سطحيات البوابة| متريكا | تيبو | اتيكيت | نوتاس |
|--------|------|-----------|-------|
| `sorafs_gateway_active` | مقياس (UpDownCounter) | `endpoint`، `method`، `variant`، `chunker`، `profile` | صادر عبر `SorafsGatewayOtel`; عمليات HTTP البعيدة المدى من خلال الجمع بين نقطة النهاية/الطريقة. |
| `sorafs_gateway_responses_total` | عداد | `endpoint`، `method`، `variant`، `chunker`، `profile`، `result`، `status`، `error_code` | كل طلب مكتمل من البوابة يتزايد مرة واحدة؛ `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | الرسم البياني | `endpoint`، `method`، `variant`، `chunker`، `profile`، `result`، `status`، `error_code` | زمن الاستجابة من البايت الأول إلى الاستجابة للبوابة؛ التصدير مثل Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | عداد | `profile_version`، `result`، `error_code` | تم التقاط نتائج التحقق من الاختبارات في لحظة الطلب (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | الرسم البياني | `profile_version`، `result`، `error_code` | توزيع زمن الوصول للتحقق من استلاماتك. || `telemetry::sorafs.gateway.request` | حدث البناء | `endpoint`، `method`، `variant`، `result`، `status`، `error_code`، `duration_ms` | تم إنشاء السجل بواسطة كل طلب كامل للارتباط في Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`، `torii_sorafs_gateway_refusals_total` | عداد | مجموعات الآداب الموروثة | مقاييس Prometheus متجددة للوحات المعلومات التاريخية؛ صدرت مع السلسلة الجديدة OTLP. |

تشير الأحداث `telemetry::sorafs.gateway.request` إلى أجهزة OTEL ذات الحمولات المبنية، والتي تعرض `endpoint`، و`method`، و`variant`، و`status`، و`error_code`. `duration_ms` للارتباط في Loki/Tempo، بينما تستهلك لوحات المعلومات سلسلة OTLP لتتبع SLO.

### قياس الصحة عن بعد| متريكا | تيبو | اتيكيت | نوتاس |
|--------|------|-----------|-------|
| `torii_sorafs_proof_health_alerts_total` | عداد | `provider_id`، `trigger`، `penalty` | ستتم الزيادة كل مرة عندما يصدر `RecordCapacityTelemetry` `SorafsProofHealthAlert`. `trigger` يميز السقوط PDP/PoTR/Ambos، بينما يلتقط `penalty` ما إذا كانت الضمانات يتم تسجيلها بالفعل أو يتم حذفها من خلال فترة التهدئة. |
| `torii_sorafs_proof_health_pdp_failures`، `torii_sorafs_proof_health_potr_breaches` | مقياس | `provider_id` | تم الإبلاغ عن أحدث أحدث تقارير PDP/PoTR من خلال نافذة القياس عن بعد التي تحتوي على مخالفات بحيث تتفوق الفرق الكمية على موردي السياسة. |
| `torii_sorafs_proof_health_penalty_nano` | مقياس | `provider_id` | تم استعادة Monto Nano-XOR في التنبيه الأخير (هذا عند تجاوز فترة التهدئة للتطبيق). |
| `torii_sorafs_proof_health_cooldown` | مقياس | `provider_id` | مقياس منطقي (`1` = تنبيه متجاوز للتبريد) لعرض عندما تكون تنبيهات المتابعة صامتة مؤقتًا. |
| `torii_sorafs_proof_health_window_end_epoch` | مقياس | `provider_id` | تم تسجيل الوقت لنافذة القياس عن بعد لتنبيه المشغلين المرتبطين بالمصنوعات Norito. |

يغذي هذا الآن شريط الصحة واختبار لوحة القيادة Taikai viewer
(`dashboards/grafana/taikai_viewer.json`)، وإمكانية رؤية مشغلي CDN في الوقت الحقيقي
حجم التنبيهات، ومزيج تعطيل PDP/PoTR، والعقوبات، وحالة التهدئة
مورد.تستجيب المقاييس التالية الآن لقواعد التنبيه الخاصة بعارض Taikai:
يتم تغيير `SorafsProofHealthPenalty` عند ذلك
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` aumenta en
آخر 15 دقيقة، بينما يبدأ `SorafsProofHealthCooldown` إعلانًا إذا كان
يبقى المورد في فترة التهدئة لمدة خمس دقائق. تنبيهات السفير حية أون
`dashboards/alerts/taikai_viewer_rules.yml` لكي تتلقى ملفات SRE السياق الفوري
عندما يتم تكثيف تطبيق PoR/PoTR.

### سطحيات orquestador| متريكا / إيفنتو | تيبو | اتيكيت | المنتج | نوتاس |
|-----------------|------|----------|-----------|-------|
| `sorafs_orchestrator_active_fetches` | مقياس | `manifest_id`، `region` | `FetchMetricsCtx` | الجلسات الفعلية أثناء الطيران. |
| `sorafs_orchestrator_fetch_duration_ms` | الرسم البياني | `manifest_id`، `region` | `FetchMetricsCtx` | رسم بياني للمدة على بعد ميليجوندوس؛ دلاء دي 1 مللي ثانية في 30 ثانية. |
| `sorafs_orchestrator_fetch_failures_total` | عداد | `manifest_id`، `region`، `reason` | `FetchMetricsCtx` | المناطق: `no_providers`، `no_healthy_providers`، `no_compatible_providers`، `exhausted_retries`، `observer_failed`، `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | عداد | `manifest_id`، `provider_id`، `reason` | `FetchMetricsCtx` | التمييز بين أسباب إعادة التشغيل (`retry`، `digest_mismatch`، `length_mismatch`، `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | عداد | `manifest_id`، `provider_id`، `reason` | `FetchMetricsCtx` | التقاط العجز وحالات السقوط على مستوى الجلسة. |
| `sorafs_orchestrator_chunk_latency_ms` | الرسم البياني | `manifest_id`، `provider_id` | `FetchMetricsCtx` | توزيع زمن الوصول للقطعة (مللي ثانية) لتحليل الإنتاجية/SLO. |
| `sorafs_orchestrator_bytes_total` | عداد | `manifest_id`، `provider_id` | `FetchMetricsCtx` | البايتات المدخلة حسب البيان/المزود؛ يمكنك استخلاص الإنتاجية عبر `rate()` في PromQL. |
| `sorafs_orchestrator_stalls_total` | عداد | `manifest_id`، `provider_id` | `FetchMetricsCtx` | هذه القطع التي تتجاوز `ScoreboardConfig::latency_cap_ms`. || `telemetry::sorafs.fetch.lifecycle` | حدث البناء | `manifest`، `region`، `job_id`، `event`، `status`، `chunk_count`، `total_bytes`، `provider_candidates`، `retry_budget`، `global_parallel_limit` | `FetchTelemetryCtx` | قم بإعادة تدوير حلقة الحياة المهمة (البداية/الكاملة) مع الحمولة JSON Norito. |
| `telemetry::sorafs.fetch.retry` | حدث البناء | `manifest`، `region`، `job_id`، `provider`، `reason`، `attempts` | `FetchTelemetryCtx` | Emitido por racha de retentos por profeedor؛ `attempts` cuenta retentos incrementales (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | حدث البناء | `manifest`، `region`، `job_id`، `provider`، `reason`، `failures` | `FetchTelemetryCtx` | سيتم نشره عندما يقوم محقق برحلة إلى ظل السقوط. |
| `telemetry::sorafs.fetch.error` | حدث البناء | `manifest`، `region`، `job_id`، `reason`، `provider?`، `provider_reason?`، `duration_ms` | `FetchTelemetryCtx` | سجل المحطة الطرفية، سهل الاستخدام على Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | حدث البناء | `manifest`، `region`، `job_id`، `provider`، `latency_ms`، `bytes` | `FetchTelemetryCtx` | يتم إصداره عندما يكون زمن الوصول للقطعة أعلى من الحد المحدد (يعكس مفاتيح التوقف). |

### سطح العقدة/النسخ المتماثل| متريكا | تيبو | اتيكيت | نوتاس |
|--------|------|-----------|-------|
| `sorafs_node_capacity_utilisation_pct` | الرسم البياني | `provider_id` | Histograma OTEL del porcentaje de utilización de التخزين (تم تصديره كـ `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | عداد | `provider_id` | وحدة تحكم رتيبة للعروض الخروجية، المستمدة من لقطات الجدولة. |
| `sorafs_node_por_failure_total` | عداد | `provider_id` | Contador monotónico de muestras PoR Fallidas. |
| `torii_sorafs_storage_bytes_*`، `torii_sorafs_storage_por_*` | مقياس | `provider` | توجد أجهزة قياس Prometheus للبايتات المستخدمة وعمق الكولا وحسابات PoR في السرعة. |
| `torii_sorafs_capacity_*`، `torii_sorafs_uptime_bps`، `torii_sorafs_por_bps` | مقياس | `provider` | تظهر بيانات السعة/وقت التشغيل للمورد على لوحة معلومات السعة. |
| `torii_sorafs_por_ingest_backlog`، `torii_sorafs_por_ingest_failures_total` | مقياس | `provider`، `manifest` | يؤدي عمق تراكم البيانات إلى زيادة تراكم أجهزة القياس الصادرة كل مرة تقوم فيها بمراجعة `/v1/sorafs/por/ingestion/{manifest}`، ويتم تشغيل اللوحة/التنبيه "PoR Stalls". |

### اختبار فرص الاسترداد (PoTR) وSLA للقطع| متريكا | تيبو | اتيكيت | المنتج | نوتاس |
|--------|------|-----------|-----------|-------|
| `sorafs_potr_deadline_ms` | الرسم البياني | `tier`، `provider` | منسق بوتر | Holgura del الموعد النهائي en milisegundos (positivo = cumplido). |
| `sorafs_potr_failures_total` | عداد | `tier`، `provider`، `reason` | منسق بوتر | المناطق: `expired`، `missing_proof`، `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | عداد | `provider`، `manifest_id`، `reason` | مونيتور دي SLA | تتم إزالة الجزء المتبقي من القطع بما في ذلك SLO (زمن الاستجابة، وطاقة النجاح). |
| `sorafs_chunk_sla_violation_active` | مقياس | `provider`، `manifest_id` | مونيتور دي SLA | مقياس منطقي (0/1) يتغير أثناء تشغيل النافذة غير النشطة. |

## أوبجيتيفوس سلو

- إمكانية الوصول إلى البوابة غير الموثوقة: **99.9%** (استجابة HTTP 2xx/304).
- TTFB P95 Trustless: الطبقة الساخنة ≥ 120 مللي ثانية، الطبقة الدافئة ≥ 300 مللي ثانية.
- نسبة النجاح في الاختبار: ≥ 99.5% يوميًا.
- نجاح الأوركيستادور (إنهاء القطع): ≥ 99%.

## لوحات المعلومات والتنبيهات1. **مراقبة البوابة** (`dashboards/grafana/sorafs_gateway_observability.json`) — شبكة التوفر الموثوقة، TTFB P95، إلغاء الاشتراكات والسقوط PoR/PoTR عبر مقاييس OTEL.
2. **سلامة الأوركيستادور** (`dashboards/grafana/sorafs_fetch_observability.json`) — شحنة مكعبة متعددة الفوهات، ومتجددة، وسقطات من الموردين، وخراطيم الأكشاك.
3. **مقاييس الخصوصية الخاصة بـ SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — جرافات الترحيل غير المميزة ونوافذ القمع ومجموعات الطاقة عبر `soranet_privacy_last_poll_unixtime` و`soranet_privacy_collector_enabled` و`soranet_privacy_poll_errors_total{provider}`.

حزم التنبيهات:

- `dashboards/alerts/sorafs_gateway_rules.yml` — إتاحة البوابة، TTFB، صور الاختبار.
- `dashboards/alerts/sorafs_fetch_rules.yml` - السقوط/الإعادة/أكشاك الأوركويستادور؛ تم التحقق من صحته عبر `scripts/telemetry/test_sorafs_fetch_alerts.sh` و`dashboards/alerts/tests/sorafs_fetch_rules.test.yml` و`dashboards/alerts/tests/soranet_privacy_rules.test.yml` و`dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` - صور انخفاض الخصوصية، إنذارات القمع، اكتشاف المجمع غير النشط وتنبيهات المجمع المعطل (`soranet_privacy_last_poll_unixtime`، `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — إنذارات انقطاع التيار الكهربائي المجهول المتصل بـ `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` - تعرض إنذارات الاشتقاق/الاستقبال/تأخر CEK لـ Taikai المزيد من التنبيهات الجديدة للعقوبة/تبريد اختبار النبضات SoraFS لـ `torii_sorafs_proof_health_*`.

## استراتيجية Trazas- اعتماد OpenTelemetry من أقصى الحدود:
  - تصدر البوابات امتدادات OTLP (HTTP) مشروحة مع معرفات الطلب وملخصات البيان وتجزئة الرمز المميز.
  - الأوركيستادور `tracing` + `opentelemetry` للتصدير على نطاق واسع لأغراض الجلب.
  - العقد SoraFS متضمنة في نطاقات التصدير لتحديات عمليات النقل والتخزين. تحتوي جميع المكونات على معرف تتبع تم نشره عبر `x-sorafs-trace`.
- يقوم `SorafsFetchOtel` بتوصيل مقاييس برنامج التشغيل إلى المدرج التكراري OTLP بينما تقوم الأحداث `telemetry::sorafs.fetch.*` بتزويد حمولات JSON الخفيفة للواجهات الخلفية المركزية والسجلات.
- المجمعات: جامعات التنفيذ OTEL junto con Prometheus/Loki/Tempo (Tempo Favoritedo). المصدرون API Jaeger يتصفحون الخيارات.
- يجب أن تكون عمليات الكارديناليد العالية قوية (10% لطرق النجاح، 100% للسقوط).

## تنسيق القياس عن بعد TLS (SF-5b)- تفاوت المقاييس:
  - يتم إرسال أتمتة TLS عبر `sorafs_gateway_tls_cert_expiry_seconds` و`sorafs_gateway_tls_renewal_total{result}` و`sorafs_gateway_tls_ech_enabled`.
  - قم بتضمين هذه المقاييس في نظرة عامة على بوابة لوحة القيادة بالإضافة إلى لوحة TLS/الشهادات.
- تنبيهات التنبيه:
  - عند ترك تنبيهات انتهاء صلاحية TLS (≥ 14 يومًا المتبقية) المرتبطة بـ SLO للتوفر غير الموثوق به.
  - يُصدر تعطيل ECH تنبيهًا ثانويًا يشير إلى لوحات TLS كميزة التوفر.
- خط الأنابيب: يتم تصدير مهمة أتمتة TLS بنفس المكدس Prometheus مثل مقاييس البوابة؛ التنسيق مع SF-5b أداة آمنة غير مكررة.

## اتفاقيات الأسماء وآداب القياس- أرقام المقاييس تتبع التفضيلات الموجودة `torii_sorafs_*` أو `sorafs_*` المستخدمة من خلال Torii والبوابة.
- مجموعات الآداب موحدة:
  - `result` → نتيجة HTTP (`success`، `refused`، `failed`).
  - `reason` → رمز الإصلاح/الخطأ (`unsupported_chunker`، `timeout`، وما إلى ذلك).
  - `provider` → معرف المصحح المشفر بالست عشري.
  - `manifest` → ملخص canónico de Manifest (recortado cuando hay alta Cardinalidad).
  - `tier` → علامات الطبقة المعلنة (`hot`، `warm`، `archive`).
- نقاط الانبعاث للقياس عن بعد:
  - تعمل مقاييس البوابة على `torii_sorafs_*` وإعادة استخدام اتفاقيات `crates/iroha_core/src/telemetry.rs`.
  - يُصدر المُنظم مقاييس `sorafs_orchestrator_*` والأحداث `telemetry::sorafs.fetch.*` (دورة الحياة، إعادة المحاولة، فشل الموفر، الخطأ، التوقف) مع ملخص البيان ومعرف الوظيفة والمنطقة ومعرفات المورد.
  - تم توضيح العقد `torii_sorafs_storage_*` و`torii_sorafs_capacity_*` و`torii_sorafs_por_*`.
- التنسيق مع إمكانية الملاحظة لتسجيل كتالوج المقاييس في المستند المقسم بالأرقام Prometheus، بما في ذلك التوقعات الأساسية للعلامات (الحدود العليا للمورد/البيانات).

## خط أنابيب البيانات- يتم تجميع المجمعات جنبًا إلى جنب مع كل مكون، وتصدير OTLP إلى Prometheus (المقاييس) وLoki/Tempo (السجلات/المسارات).
- يعمل eBPF الاختياري (Tetragon) على زيادة تأثير المستوى الأدنى للبوابات/العقد.
- الولايات المتحدة الأمريكية `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` para Torii و nodos Embebidos؛ يستمر الأوركيستادور في الاتصال إلى `install_sorafs_fetch_otlp_exporter`.

## خطافات التحقق من الصحة

- قم بتشغيل `scripts/telemetry/test_sorafs_fetch_alerts.sh` خلال CI للتأكد من أن قواعد التنبيه الخاصة بـ Prometheus يمكن أن تظل متزامنة مع مقاييس التوقف وفحوصات الخصوصية.
- حافظ على لوحات المعلومات Grafana بعد التحكم في الإصدارات (`dashboards/grafana/`) وتحديث اللقطات/الروابط عند تغيير اللوحات.
- تم تسجيل نتائج التدريبات عبر `scripts/telemetry/log_sorafs_drill.sh`؛ التحقق من الصحة باستخدام `scripts/telemetry/validate_drill_log.sh` (راجع [دليل العمليات](operations-playbook.md)).