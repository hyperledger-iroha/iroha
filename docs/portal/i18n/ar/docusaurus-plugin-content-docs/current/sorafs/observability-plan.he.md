---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/observability-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e170c4e3d9f63a896cd026ea91570da98f406c6906a690a043494ebdb925771
source_last_modified: "2026-01-21T07:38:35+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: observability-plan
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/observability-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note المصدر المعتمد
تعكس هذه الصفحة الخطة الموجودة ضمن `docs/source/sorafs_observability_plan.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم ترحيل مجموعة Sphinx القديمة بالكامل.
:::

## الأهداف
- تحديد المقاييس والأحداث المُهيكلة للبوابات والعقد والمُنسِّق متعدد المصادر.
- توفير لوحات Grafana وحدود التنبيه وخطافات التحقق.
- تثبيت أهداف SLO جنبًا إلى جنب مع سياسات ميزانية الخطأ وتمارين الفوضى.

## كتالوج المقاييس

### أسطح البوابة

| المقياس | النوع | Labels | الملاحظات |
|--------|-------|--------|-----------|
| `sorafs_gateway_active` | Gauge (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | يُصدر عبر `SorafsGatewayOtel`؛ يتتبع عمليات HTTP الجارية لكل تركيبة endpoint/method. |
| `sorafs_gateway_responses_total` | Counter | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | كل طلب مكتمل للبوابة يزيد العداد مرة واحدة؛ `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histogram | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | كمون time-to-first-byte لاستجابات البوابة؛ يُصدَّر كـ Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Counter | `profile_version`, `result`, `error_code` | نتائج تحقق الأدلة الملتقطة لحظة الطلب (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histogram | `profile_version`, `result`, `error_code` | توزيع كمون التحقق لإيصالات PoR. |
| `telemetry::sorafs.gateway.request` | حدث مُهيكل | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | سجل مُهيكل يُصدر عند اكتمال كل طلب للتنسيق مع Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Counter | مجموعات Labels قديمة | مقاييس Prometheus محفوظة للّوحات التاريخية؛ تُصدر مع سلسلة OTLP الجديدة. |

تعكس أحداث `telemetry::sorafs.gateway.request` عدادات OTEL مع حمولة مُهيكلة، فتُظهر `endpoint` و`method` و`variant` و`status` و`error_code` و`duration_ms` لتنسيق Loki/Tempo، بينما تستهلك لوحات المتابعة سلسلة OTLP لتعقب SLO.

### تليمترية صحة الأدلة

| المقياس | النوع | Labels | الملاحظات |
|--------|-------|--------|-----------|
| `torii_sorafs_proof_health_alerts_total` | Counter | `provider_id`, `trigger`, `penalty` | يزيد كلما أصدرت `RecordCapacityTelemetry` حدث `SorafsProofHealthAlert`. يميز `trigger` بين إخفاقات PDP/PoTR/Both، بينما يلتقط `penalty` ما إذا كان الضمان قد خُصم فعليًا أو تم كتمه عبر cooldown. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Gauge | `provider_id` | أحدث أعداد PDP/PoTR داخل نافذة التليمترية المخالِفة حتى تتمكن الفرق من قياس مقدار تجاوز المزوّدين للسياسة. |
| `torii_sorafs_proof_health_penalty_nano` | Gauge | `provider_id` | مقدار Nano-XOR المخصوم في آخر تنبيه (صفر عند كتم التطبيق بسبب cooldown). |
| `torii_sorafs_proof_health_cooldown` | Gauge | `provider_id` | مقياس بولي (`1` = تنبيه مكبوت بسبب cooldown) لإظهار متى تكون تنبيهات المتابعة مكتومة مؤقتًا. |
| `torii_sorafs_proof_health_window_end_epoch` | Gauge | `provider_id` | الحقبة المسجّلة لنافذة التليمترية المرتبطة بالتنبيه لتمكين المشغلين من الربط مع آثار Norito. |

تغذي هذه التدفقات الآن صف proof-health في لوحة Taikai viewer
(`dashboards/grafana/taikai_viewer.json`)، مما يمنح مشغلي CDN رؤية فورية
لأحجام التنبيهات ومزيج محفزات PDP/PoTR والعقوبات وحالة cooldown لكل مزوّد.

تدعم المقاييس نفسها الآن قاعدتي تنبيه في Taikai viewer:
تطلق `SorafsProofHealthPenalty` عندما
يزداد `torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` خلال
آخر 15 دقيقة، بينما يرفع `SorafsProofHealthCooldown` تحذيرًا إذا بقي مزوّد في
cooldown لمدة خمس دقائق. كلا التنبيهين موجودان في
`dashboards/alerts/taikai_viewer_rules.yml` لكي يحصل SREs على سياق فوري عند تصاعد
تطبيق PoR/PoTR.

### أسطح المُنسِّق

| المقياس / الحدث | النوع | Labels | المُنتِج | الملاحظات |
|----------------|-------|--------|---------|-----------|
| `sorafs_orchestrator_active_fetches` | Gauge | `manifest_id`, `region` | `FetchMetricsCtx` | الجلسات الجارية حاليًا. |
| `sorafs_orchestrator_fetch_duration_ms` | Histogram | `manifest_id`, `region` | `FetchMetricsCtx` | هيستوغرام المدة بالميلي ثانية؛ نطاقات 1 ms إلى 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Counter | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | الأسباب: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Counter | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | يميز أسباب إعادة المحاولة (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Counter | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | يلتقط تعطيل الجلسة أو تعداد الإخفاقات على مستوى الجلسة. |
| `sorafs_orchestrator_chunk_latency_ms` | Histogram | `manifest_id`, `provider_id` | `FetchMetricsCtx` | توزيع كمون جلب الشرائح (ms) لتحليل throughput/SLO. |
| `sorafs_orchestrator_bytes_total` | Counter | `manifest_id`, `provider_id` | `FetchMetricsCtx` | البايتات المسلّمة لكل manifest/provider؛ استخرج throughput عبر `rate()` في PromQL. |
| `sorafs_orchestrator_stalls_total` | Counter | `manifest_id`, `provider_id` | `FetchMetricsCtx` | يحصي الشرائح التي تتجاوز `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | حدث مُهيكل | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | يعكس دورة حياة المهمة (بدء/اكتمال) بحمولة Norito JSON. |
| `telemetry::sorafs.fetch.retry` | حدث مُهيكل | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | يصدر لكل سلسلة إعادة محاولة لمزوّد؛ `attempts` تحصي المحاولات التراكمية (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | حدث مُهيكل | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | يُظهر عند تجاوز المزوّد لعتبة الإخفاق. |
| `telemetry::sorafs.fetch.error` | حدث مُهيكل | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | سجل فشل نهائي مناسب لابتلاع Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | حدث مُهيكل | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | يُطلق عند تجاوز كمون الشريحة للحد المضبوط (يعكس عدادات stall). |

### أسطح العقد / التكرار

| المقياس | النوع | Labels | الملاحظات |
|--------|-------|--------|-----------|
| `sorafs_node_capacity_utilisation_pct` | Histogram | `provider_id` | هيستوغرام OTEL لنسبة استخدام التخزين (يُصدر كـ `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Counter | `provider_id` | عداد أحادي لعينات PoR الناجحة، مشتق من لقطات المجدول. |
| `sorafs_node_por_failure_total` | Counter | `provider_id` | عداد أحادي لعينات PoR الفاشلة. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Gauge | `provider` | مقاييس Prometheus الحالية للبايتات المستخدمة وعمق الطابور وعدادات PoR الجارية. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Gauge | `provider` | بيانات نجاح السعة/الجاهزية للمزوّد المعروضة في لوحة السعة. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Gauge | `provider`, `manifest` | عمق التراكم بالإضافة إلى عدادات الفشل التراكمية المصدّرة عند الاستعلام عن `/v2/sorafs/por/ingestion/{manifest}` لتغذية لوحة/تنبيه "PoR Stalls". |

### الإصلاح وSLA

| المقياس | النوع | Labels | الملاحظات |
|--------|------|--------|-----------|
| `sorafs_repair_tasks_total` | Counter | `status` | عداد OTEL لتحولات مهام الإصلاح. |
| `sorafs_repair_latency_minutes` | Histogram | `outcome` | مدرج OTEL لزمن دورة حياة الإصلاح. |
| `sorafs_repair_queue_depth` | Histogram | `provider` | مدرج OTEL لعدد المهام المصطفة لكل مزود (لقطة زمنية). |
| `sorafs_repair_backlog_oldest_age_seconds` | Histogram | — | مدرج OTEL لعمر أقدم مهمة في الطابور (بالثواني). |
| `sorafs_repair_lease_expired_total` | Counter | `outcome` | عداد OTEL لانتهاء مدد الإيجار (`requeued`/`escalated`). |
| `sorafs_repair_slash_proposals_total` | Counter | `outcome` | عداد OTEL لتحولات مقترحات السلاش. |
| `torii_sorafs_repair_tasks_total` | Counter | `status` | عداد Prometheus لتحولات المهام. |
| `torii_sorafs_repair_latency_minutes_bucket` | Histogram | `outcome` | مدرج Prometheus لزمن دورة حياة الإصلاح. |
| `torii_sorafs_repair_queue_depth` | Gauge | `provider` | مقياس Prometheus لعدد المهام المصطفة لكل مزود. |
| `torii_sorafs_repair_backlog_oldest_age_seconds` | Gauge | — | مقياس Prometheus لعمر أقدم مهمة في الطابور (ثوانٍ). |
| `torii_sorafs_repair_lease_expired_total` | Counter | `outcome` | عداد Prometheus لانتهاء مدد الإيجار. |
| `torii_sorafs_slash_proposals_total` | Counter | `outcome` | عداد Prometheus لتحولات مقترحات السلاش. |

تعكس بيانات JSON الوصفية لتدقيق الحوكمة تسميات تليمترية الإصلاح (`status`, `ticket_id`, `manifest`, `provider` في أحداث الإصلاح؛ و`outcome` في مقترحات السلاش) كي يمكن ربط المقاييس وأثر التدقيق بشكل حتمي.

### Proof of Timely Retrieval (PoTR) وSLA الشرائح

| المقياس | النوع | Labels | المُنتِج | الملاحظات |
|--------|-------|--------|---------|-----------|
| `sorafs_potr_deadline_ms` | Histogram | `tier`, `provider` | منسق PoTR | هامش الموعد النهائي بالميلي ثانية (موجب = محقق). |
| `sorafs_potr_failures_total` | Counter | `tier`, `provider`, `reason` | منسق PoTR | الأسباب: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Counter | `provider`, `manifest_id`, `reason` | مراقب SLA | يطلق عند إخفاق تسليم الشرائح في تحقيق SLO (كمون، معدل نجاح). |
| `sorafs_chunk_sla_violation_active` | Gauge | `provider`, `manifest_id` | مراقب SLA | مقياس بولي (0/1) يتبدل أثناء نافذة الإخفاق النشطة. |

## أهداف SLO

- توفر البوابة بدون ثقة: **99.9%** (استجابات HTTP 2xx/304).
- Trustless TTFB P95: hot tier ≤ 120 ms، warm tier ≤ 300 ms.
- معدل نجاح الأدلة: ≥ 99.5% يوميًا.
- نجاح المُنسِّق (اكتمال الشرائح): ≥ 99%.

## لوحات المتابعة والتنبيهات

1. **Observability للبوابة** (`dashboards/grafana/sorafs_gateway_observability.json`) — تتبع توفر trustless وTTFB P95 وتفصيل الرفض وإخفاقات PoR/PoTR عبر مقاييس OTEL.
2. **صحة المُنسِّق** (`dashboards/grafana/sorafs_fetch_observability.json`) — تغطي الحمل متعدد المصادر وإعادات المحاولة وإخفاقات المزوّدين وموجات stalls.
3. **مقاييس خصوصية SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — ترسم buckets relay المجهولة ونوافذ الكتم وصحة collector عبر `soranet_privacy_last_poll_unixtime` و`soranet_privacy_collector_enabled` و`soranet_privacy_poll_errors_total{provider}`.

حزم التنبيهات:

- `dashboards/alerts/sorafs_gateway_rules.yml` — توفر البوابة وTTFB وارتفاعات فشل الأدلة.
- `dashboards/alerts/sorafs_fetch_rules.yml` — إخفاقات/إعادات المحاولة/stalls للمُنسِّق؛ يتم التحقق عبر `scripts/telemetry/test_sorafs_fetch_alerts.sh` و`dashboards/alerts/tests/sorafs_fetch_rules.test.yml` و`dashboards/alerts/tests/soranet_privacy_rules.test.yml` و`dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — قمم تدهور الخصوصية وإنذارات الكتم ورصد collector الخامل وتنبيهات collector المعطل (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — إنذارات brownout للخصوصية مربوطة بـ `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — إنذارات drift/ingest/CEK lag في Taikai viewer إضافة إلى تنبيهات penalty/cooldown لصحة الأدلة في SoraFS المبنية على `torii_sorafs_proof_health_*`.

## استراتيجية التتبع

- اعتماد OpenTelemetry من الطرف للطرف:
  - تصدر البوابات OTLP spans (HTTP) مع معرفات الطلب وdigests المانيفست وtoken hashes.
  - يستخدم المُنسِّق `tracing` + `opentelemetry` لتصدير spans لمحاولات الجلب.
  - تصدر عقد SoraFS المدمجة spans لتحديات PoR وعمليات التخزين. تشترك كل المكونات في trace ID موحد ينتقل عبر `x-sorafs-trace`.
- يربط `SorafsFetchOtel` مقاييس المُنسِّق بهيستوغرامات OTLP بينما توفر أحداث `telemetry::sorafs.fetch.*` حمولات JSON خفيفة لخلفيات تركّز على السجلات.
- Collectors: شغّل OTEL collectors بجانب Prometheus/Loki/Tempo (Tempo مفضل). ما زالت المُصدِّرات المتوافقة مع Jaeger اختيارية.
- يجب أخذ عينات للعمليات عالية الكاردينالية (10% لمسارات النجاح، 100% للإخفاقات).

## تنسيق تليمترية TLS (SF-5b)

- مواءمة المقاييس:
  - ترسل أتمتة TLS `sorafs_gateway_tls_cert_expiry_seconds` و`sorafs_gateway_tls_renewal_total{result}` و`sorafs_gateway_tls_ech_enabled`.
  - أدرج هذه المقاييس في لوحة Gateway Overview ضمن لوحة TLS/Certificates.
- ربط التنبيهات:
  - عند إطلاق تنبيهات انتهاء TLS (≤ 14 يومًا متبقّيًا) اربطها مع SLO توفر trustless.
  - يؤدي تعطيل ECH إلى تنبيه ثانوي يشير إلى لوحات TLS والتوفر معًا.
- خط الأنابيب: تصدر مهمة أتمتة TLS إلى نفس مكدس Prometheus الذي يستخدمه gateway؛ يضمن التنسيق مع SF-5b إزالة التكرار في القياس.

## اصطلاحات تسمية المقاييس والـLabels

- تتبع أسماء المقاييس بادئات `torii_sorafs_*` أو `sorafs_*` المستخدمة بواسطة Torii والبوابة.
- مجموعات الـLabels موحدة:
  - `result` → مخرجات HTTP (`success`, `refused`, `failed`).
  - `reason` → رمز الرفض/الخطأ (`unsupported_chunker`, `timeout`, إلخ).
  - `provider` → معرف المزوّد مرمّز بالهيكس.
  - `manifest` → digest مانيفست قانوني (يتم تقليمه عند ارتفاع الكاردينالية).
  - `tier` → Labels الطبقات التعريفية (`hot`, `warm`, `archive`).
- نقاط إصدار التليمترية:
  - مقاييس البوابة تعيش تحت `torii_sorafs_*` وتعيد استخدام اصطلاحات `crates/iroha_core/src/telemetry.rs`.
  - يصدر المُنسِّق مقاييس `sorafs_orchestrator_*` وأحداث `telemetry::sorafs.fetch.*` (lifecycle, retry, provider failure, error, stall) بعلامات digest المانيفست وjob ID وregion ومعرفات المزوّد.
  - تعرض العقد `torii_sorafs_storage_*` و`torii_sorafs_capacity_*` و`torii_sorafs_por_*`.
- نسّق مع Observability لتسجيل كتالوج المقاييس في وثيقة أسماء Prometheus المشتركة، بما في ذلك توقعات كاردينالية الـLabels (الحدود العليا للمزوّد/المانيفست).

## خط أنابيب البيانات

- تُنشر collectors بجانب كل مكوّن، وتصدر OTLP إلى Prometheus (مقاييس) وLoki/Tempo (سجلات/تتبعات).
- يثري eBPF الاختياري (Tetragon) التتبع منخفض المستوى للبوابات/العقد.
- استخدم `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` لـ Torii والعقد المدمجة؛ يستمر المُنسِّق في استدعاء `install_sorafs_fetch_otlp_exporter`.

## خطافات التحقق

- شغّل `scripts/telemetry/test_sorafs_fetch_alerts.sh` أثناء CI لضمان بقاء قواعد تنبيه Prometheus متزامنة مع مقاييس stall وفحوصات كتم الخصوصية.
- حافظ على لوحات Grafana ضمن التحكم بالإصدارات (`dashboards/grafana/`) وحدّث اللقطات/الروابط عند تغيير اللوحات.
- تسجل تمارين الفوضى النتائج عبر `scripts/telemetry/log_sorafs_drill.sh`؛ يستخدم التحقق `scripts/telemetry/validate_drill_log.sh` (راجع [دليل العمليات](operations-playbook.md)).
