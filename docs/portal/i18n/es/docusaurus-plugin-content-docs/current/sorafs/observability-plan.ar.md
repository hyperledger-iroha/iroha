---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan de observabilidad
título: خطة قابلية الملاحظة وأهداف SLO لـ SoraFS
sidebar_label: قابلية الملاحظة وSLO
descripción: مخطط التليمترية ولوحات المتابعة وسياسة ميزانية الخطأ لبوابات SoraFS والعقد والمُنسِّق متعدد المصادر.
---

:::nota المصدر المعتمد
Utilice el software `docs/source/sorafs_observability_plan.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم ترحيل مجموعة Sphinx القديمة بالكامل.
:::

## الأهداف
- تحديد المقاييس والأحداث المُهيكلة للبوابات والعقد والمُنسِّق متعدد المصادر.
- توفير لوحات Grafana وحدود التنبيه وخطافات التحقق.
- تثبيت أهداف SLO جنبًا إلى جنب مع سياسات ميزانية الخطأ وتمارين الفوضى.

## كتالوج المقاييس

### أسطح البوابة| المقياس | النوع | Etiquetas | الملاحظات |
|--------|-------|--------|-----------|
| `sorafs_gateway_active` | Medidor (ContadorArribaAbajo) | `endpoint`, `method`, `variant`, `chunker`, `profile` | يُصدر عبر `SorafsGatewayOtel`؛ Utilice el punto final/método HTTP. |
| `sorafs_gateway_responses_total` | Mostrador | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | كل طلب مكتمل للبوابة يزيد العداد مرة واحدة؛ `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histograma | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | كمون tiempo hasta el primer byte لاستجابات البوابة؛ يُصدَّر كـ Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Mostrador | `profile_version`, `result`, `error_code` | نتائج تحقق الأدلة الملتقطة لحظة الطلب (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histograma | `profile_version`, `result`, `error_code` | توزيع كمون التحقق لإيصالات PoR. |
| `telemetry::sorafs.gateway.request` | حدث مُهيكل | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | سجل مُهيكل يُصدر عند اكتمال كل طلب للتنسيق مع Loki/Tempo. || `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Mostrador | مجموعات Etiquetas قديمة | مقاييس Prometheus محفوظة للّوحات التاريخية؛ تُصدر مع سلسلة OTLP الجديدة. |

تعكس أحداث `telemetry::sorafs.gateway.request` عدادات OTEL مع حمولة مُهيكلة, فتُظهر `endpoint` و`method` و`variant` و`status` و`error_code` و`duration_ms` Para Loki/Tempo, está conectado a OTLP y SLO.

### تليمترية صحة الأدلة

| المقياس | النوع | Etiquetas | الملاحظات |
|--------|-------|--------|-----------|
| `torii_sorafs_proof_health_alerts_total` | Mostrador | `provider_id`, `trigger`, `penalty` | Aquí está el `RecordCapacityTelemetry` y el `SorafsProofHealthAlert`. يميز `trigger` بين إخفاقات PDP/PoTR/Both, بينما يلتقط `penalty` ما إذا كان الضمان قد خُصم فعليًا أو تم كتمه عبر tiempo de reutilización. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Calibre | `provider_id` | أحدث أعداد PDP/PoTR داخل نافذة التليمترية المخالِفة حتى تتمكن الفرق من قياس مقدار تجاوز المزوّدين للسياسة. |
| `torii_sorafs_proof_health_penalty_nano` | Calibre | `provider_id` | مقدار Nano-XOR المخصوم في آخر تنبيه (صفر عند كتم التطبيق بسبب cooldown). |
| `torii_sorafs_proof_health_cooldown` | Calibre | `provider_id` | مقياس بولي (`1` = تنبيه مكبوت بسبب cooldown) لإظهار متى تكون تنبيهات المتابعة مكتومة مؤقتًا. |
| `torii_sorafs_proof_health_window_end_epoch` | Calibre | `provider_id` | El cable de alimentación del cable de alimentación se puede conectar al cable de alimentación Norito. |تغذي هذه التدفقات الآن صف prueba de salud في لوحة Taikai espectador
(`dashboards/grafana/taikai_viewer.json`) مما يمنح مشغلي CDN رؤية فورية
Las aplicaciones PDP/PoTR, el tiempo de reutilización y el tiempo de reutilización están disponibles.

تدعم المقاييس نفسها الآن قاعدتي تنبيه في Taikai espectador:
تطلق `SorafsProofHealthPenalty` عندما
يزداد `torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` خلال
آخر 15 دقيقة, بينما يرفع `SorafsProofHealthCooldown` تحذيرًا إذا بقي مزوّد في
tiempo de reutilización لمدة خمس دقائق. كلا التنبيهين موجودان في
`dashboards/alerts/taikai_viewer_rules.yml` لكي يحصل SRes على سياق فوري عند تصاعد
تطبيق PoR/PoTR.

### أسطح المُنسِّق| المقياس / الحدث | النوع | Etiquetas | المُنتِج | الملاحظات |
|----------------|-------|--------|---------|-----------|
| `sorafs_orchestrator_active_fetches` | Calibre | `manifest_id`, `region` | `FetchMetricsCtx` | الجلسات الجارية حاليًا. |
| `sorafs_orchestrator_fetch_duration_ms` | Histograma | `manifest_id`, `region` | `FetchMetricsCtx` | هيستوغرام المدة بالميلي ثانية؛ Duración de 1 ms a 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Mostrador | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Nombre: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Mostrador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | يميز أسباب إعادة المحاولة (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Mostrador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | يلتقط تعطيل الجلسة أو تعداد الإخفاقات على مستوى الجلسة. |
| `sorafs_orchestrator_chunk_latency_ms` | Histograma | `manifest_id`, `provider_id` | `FetchMetricsCtx` | توزيع كمون جلب الشرائح (ms) para el rendimiento/SLO. |
| `sorafs_orchestrator_bytes_total` | Mostrador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | البايتات المسلّمة لكل manifiesto/proveedor؛ El rendimiento del archivo es `rate()` en PromQL. |
| `sorafs_orchestrator_stalls_total` | Mostrador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | يحصي الشرائح التي تتجاوز `ScoreboardConfig::latency_cap_ms`. || `telemetry::sorafs.fetch.lifecycle` | حدث مُهيكل | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Este archivo de configuración (بدء/اكتمال) contiene Norito JSON. |
| `telemetry::sorafs.fetch.retry` | حدث مُهيكل | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | يصدر لكل سلسلة إعادة محاولة لمزوّد؛ `attempts` تحصي المحاولات التراكمية (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | حدث مُهيكل | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | يُظهر عند تجاوز المزوّد لعتبة الإخفاق. |
| `telemetry::sorafs.fetch.error` | حدث مُهيكل | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Esta es la versión de Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | حدث مُهيكل | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | يُطلق عند تجاوز كمون الشريحة للحد المضبوط (يعكس عدادات puesto). |

### أسطح العقد / التكرار| المقياس | النوع | Etiquetas | الملاحظات |
|--------|-------|--------|-----------|
| `sorafs_node_capacity_utilisation_pct` | Histograma | `provider_id` | Este OTEL لنسبة استخدام التخزين (يُصدر كـ `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Mostrador | `provider_id` | عداد أحادي لعينات PoR الناجحة، مشتق من لقطات المجدول. |
| `sorafs_node_por_failure_total` | Mostrador | `provider_id` | عداد أحادي لعينات PoR الفاشلة. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Calibre | `provider` | مقاييس Prometheus الحالية للبايتات المستخدمة وعمق الطابور وعدادات PoR الجارية. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Calibre | `provider` | بيانات نجاح السعة/الجاهزية للمزوّد المعروضة في لوحة السعة. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Calibre | `provider`, `manifest` | Para obtener más información, consulte el documento "PoR" Puestos". |

### Prueba de recuperación oportuna (PoTR) y SLA الشرائح| المقياس | النوع | Etiquetas | المُنتِج | الملاحظات |
|--------|-------|--------|---------|-----------|
| `sorafs_potr_deadline_ms` | Histograma | `tier`, `provider` | Más PoTR | هامش الموعد النهائي بالميلي ثانية (موجب = محقق). |
| `sorafs_potr_failures_total` | Mostrador | `tier`, `provider`, `reason` | Más PoTR | Nombre: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Mostrador | `provider`, `manifest_id`, `reason` | مراقب SLA | يطلق عند إخفاق تسليم الشرائح في تحقيق SLO (كمون، معدل نجاح). |
| `sorafs_chunk_sla_violation_active` | Calibre | `provider`, `manifest_id` | مراقب SLA | مقياس بولي (0/1) يتبدل أثناء نافذة الإخفاق النشطة. |

## أهداف SLO

- توفر البوابة بدون ثقة: **99,9%** (استجابات HTTP 2xx/304).
- Trustless TTFB P95: nivel activo ≤ 120 ms, nivel activo ≤ 300 ms.
- معدل نجاح الأدلة: ≥ 99,5% يوميًا.
- نجاح المُنسِّق (اكتمال الشرائح): ≥ 99%.

## لوحات المتابعة والتنبيهات

1. **Observabilidad للبوابة** (`dashboards/grafana/sorafs_gateway_observability.json`) — تتبع توفر trustless وTTFB P95 وتفصيل الرفض وإخفاقات PoR/PoTR عبر مقاييس OTEL.
2. **صحة المُنسِّق** (`dashboards/grafana/sorafs_fetch_observability.json`) — تغطي الحمل متعدد المصادر وإعادات المحاولة وإخفاقات المزوّدين وموجات puestos.
3. **مقاييس خصوصية SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — Relé de cubos de ترسم المجهولة ونوافذ الكتم وصحة عبر `soranet_privacy_last_poll_unixtime` و`soranet_privacy_collector_enabled` Y `soranet_privacy_poll_errors_total{provider}`.

حزم التنبيهات:- `dashboards/alerts/sorafs_gateway_rules.yml` — توفر البوابة وTTFB وارتفاعات فشل الأدلة.
- `dashboards/alerts/sorafs_fetch_rules.yml` — إخفاقات/إعادات المحاولة/puestos للمُنسِّق؛ Utilice `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` y `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — قمم تدهور الخصوصية وإنذارات الكتم ورصد colector الخامل وتنبيهات colector المعطل (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml`: se produjo una caída de tensión en el dispositivo de `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — Deriva/ingesta/retraso CEK en el visor Taikai إضافة إلى تنبيهات penalización/enfriamiento لصحة الأدلة في SoraFS المبنية على `torii_sorafs_proof_health_*`.

## استراتيجية التتبع

- Aquí está OpenTelemetry:
  - تصدر البوابات OTLP spans (HTTP) مع معرفات الطلب وdigests المانيفست وtoken hashes.
  - El cable de alimentación `tracing` + `opentelemetry` abarca todos los límites.
  - El código SoraFS abarca todos los PoR y los enlaces. Para obtener un ID de seguimiento, consulte `x-sorafs-trace`.
- يربط `SorafsFetchOtel` Archivos de computadora OTLP بينما توفر أحداث `telemetry::sorafs.fetch.*` Archivos JSON لخلفيات تركّز على السجلات.
- Coleccionistas: شغّل Coleccionistas de OTEL بجانب Prometheus/Loki/Tempo (Tempo مفضل). ما زالت المُصدِّرات المتوافقة مع Jaeger اختيارية.
- يجب أخذ عينات للعمليات عالية الكاردينالية (10% لمسارات النجاح، 100% للإخفاقات).

## Actualización de TLS (SF-5b)- مواءمة المقاييس:
  - Utilice TLS `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` y `sorafs_gateway_tls_ech_enabled`.
  - أدرج هذه المقاييس في لوحة Gateway Overview ضمن لوحة TLS/Certificados.
- ربط التنبيهات:
  - عند إطلاق تنبيهات انتهاء TLS (≤ 14 يومًا متبقّيًا) اربطها مع SLO توفر sin confianza.
  - يؤدي تعطيل ECH إلى تنبيه ثانوي يشير إلى لوحات TLS y معًا.
- Nombre del usuario: conexión TLS para la conexión de puerta de enlace Prometheus يضمن التنسيق مع SF-5b إزالة التكرار في القياس.

## اصطلاحات تسمية المقاييس والـEtiquetas- Coloque los botones `torii_sorafs_*`, `sorafs_*` y Torii.
- مجموعات الـEtiquetas موحدة:
  - `result` → Enlace HTTP (`success`, `refused`, `failed`).
  - `reason` → رمز الرفض/الخطأ (`unsupported_chunker`, `timeout`, إلخ).
  - `provider` → معرف المزوّد مرمّز بالهيكس.
  - `manifest` → resumen مانيفست قانوني (يتم تقليمه عند ارتفاع الكاردينالية).
  - `tier` → Etiquetas الطبقات التعريفية (`hot`, `warm`, `archive`).
- نقاط إصدار التليمترية:
  - مقاييس البوابة تعيش تحت `torii_sorafs_*` y تعيد استخدام اصطلاحات `crates/iroha_core/src/telemetry.rs`.
  - يصدر المُنسِّق مقاييس `sorafs_orchestrator_*` y `telemetry::sorafs.fetch.*` (ciclo de vida, reintento, falla del proveedor, error, bloqueo) بعلامات resumen de datos, ID de trabajo, región y المزوّد.
  - Utilice `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` y `torii_sorafs_por_*`.
- نسّق مع Observabilidad لتسجيل كتالوج المقاييس في وثيقة أسماء Prometheus المشتركة, بما في ذلك توقعات كاردينالية الـEtiquetas (الحدود العليا للمزوّد/المانيفست).

## خط أنابيب البيانات

- Los coleccionistas de تُنشر incluyen Prometheus (مقاييس) y Loki/Tempo (سجلات/تتبعات).
- يثري eBPF الاختياري (Tetragon) التتبع منخفض المستوى للبوابات/العقد.
- استخدم `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` لـ Torii y المدمجة؛ يستمر المُنسِّق في استدعاء `install_sorafs_fetch_otlp_exporter`.

## خطافات التحقق- شغّل `scripts/telemetry/test_sorafs_fetch_alerts.sh` أثناء CI لضمان بقاء قواعد تنبيه Prometheus متزامنة مع مقاييس puesto y فحوصات كتم الخصوصية.
- حافظ على لوحات Grafana ضمن التحكم بالإصدارات (`dashboards/grafana/`) وحدّث اللقطات/الروابط عند تغيير اللوحات.
- تسجل تمارين الفوضى النتائج عبر `scripts/telemetry/log_sorafs_drill.sh`؛ يستخدم التحقق `scripts/telemetry/validate_drill_log.sh` (راجع [دليل العمليات](operations-playbook.md)).