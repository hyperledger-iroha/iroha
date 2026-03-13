---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/observability-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة المراقبة
العنوان: خطة مستقبلية وأهداف SLO لـ SoraFS
Sidebar_label: المزايا وSLOs
الوصف: مخطط التليميترية ولوحات المتابعة وسياسة التعرض للمسؤولية لبوابات SoraFS والعقد والمُنسِّق متعدد المصادر.
---

:::ملحوظة المصدر مؤهل
هذه الاعتبارات الصفحة البناءة ضمن `docs/source/sorafs_observability_plan.md`. احرص على أن يتم رحيل مجموعة أبو الهول القديمة بالكامل.
:::

##عجز
- تحديد المعايير والأحداث الهيكلة للبوابات والعقد والمنسِّق متعدد المصادر.
- توفير لوحات Grafana وحدود التنبيهات والإشعارات.
- تثبيت أهداف SLO جنبًا إلى جنب مع خيارات مقترحة للحد من الفوضى وممارسة الفوضى.

##كتالوج المعايير

### أسطح البوابة| المقياس | النوع | التسميات | مذكرة |
|--------|-------|--------|-----------|
| `sorafs_gateway_active` | مقياس (UpDownCounter) | `endpoint`، `method`، `variant`، `chunker`، `profile` | يُصدر عبر `SorafsGatewayOtel`؛ يتتبع عمليات HTTP للوصول إلى نقطة النهاية/الطريقة. |
| `sorafs_gateway_responses_total` | عداد | `endpoint`، `method`، `variant`، `chunker`، `profile`، `result`، `status`، `error_code` | كل طلب مكتمل للبوابة يزيد العداد مرة واحدة؛ `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | الرسم البياني | `endpoint`، `method`، `variant`، `chunker`، `profile`، `result`، `status`، `error_code` | كمون الوقت لأول بايت لاستجابات البوابة؛ يُصَدَّر كـ Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | عداد | `profile_version`، `result`، `error_code` | نتائج التحقق من أدلة الملتقطة الفورية (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | الرسم البياني | `profile_version`، `result`، `error_code` | توزيع كمون التحقق من إيصالات PoR. |
| `telemetry::sorafs.gateway.request` | حدث مُهيكل | `endpoint`، `method`، `variant`، `result`، `status`، `error_code`، `duration_ms` | سجل مُهيكل يُصدر عند القانون كل طلب للتنسيق مع Loki/Tempo. || `torii_sorafs_chunk_range_requests_total`، `torii_sorafs_gateway_refusals_total` | عداد | مجموعات التسميات القديمة | معايير Prometheus محفوظة للوحات التاريخية؛ تُصدر مع سلسلة OTLP الجديدة. |

الاعتبار `telemetry::sorafs.gateway.request` عدادات OTEL مع هيكل خفيف الوزن، فتُظهر `endpoint` و`method` و`variant` و`status` و`error_code` و`duration_ms` للتنسيق Loki/Tempo، أثناء استهلاكك لسلسلة OTLP لتتبع SLO.

### تلترية صحة الأدلة

| المقياس | النوع | التسميات | مذكرة |
|--------|-------|--------|-----------|
| `torii_sorafs_proof_health_alerts_total` | عداد | `provider_id`، `trigger`، `penalty` | يزيد كلما صدر `RecordCapacityTelemetry` حدث `SorafsProofHealthAlert`. دعوى `trigger` بين إخفاقات PDP/PoTR/Both، في حين يتم اختيار `penalty` ما إذا كان الضمان قد خُصم فعلياً أو تم كتمه عبر Cooldown. |
| `torii_sorafs_proof_health_pdp_failures`، `torii_sorafs_proof_health_potr_breaches` | مقياس | `provider_id` | سجلت PDP/PoTR داخل النافذة التلمترية المخالِفة حتى بداية ظهور فرق من قياس مقدار تجاوز المقياسين للسياسة. |
| `torii_sorafs_proof_health_penalty_nano` | مقياس | `provider_id` | كمية Nano-XOR المخصوم في آخر تنبيه (صفر عند كتم التطبيق بسبب Cooldown). |
| `torii_sorafs_proof_health_cooldown` | مقياس | `provider_id` | مقياس بولي (`1` = تنبيه مكبوت بسبب Cooldown) لكي متى تكون تنبيهات متابعة مكتومة مؤقتًا. |
| `torii_sorafs_proof_health_window_end_epoch` | مقياس | `provider_id` | الحقبة المسجلة لنافذة التليمترية التي بالتنبيه تعمل مباشرة من الضرب مع الإصابة Norito. |تغذي هذه العاصمة الآن صف إثبات الصحة في لوحة Taikai viewer
(`dashboards/grafana/taikai_viewer.json`)، مما يمنح مشغلي CDN رؤية فورية
لأحجام التنبيهات ومزيج محفزات PDP/PoTR والعقوبات وحالة Cooldown لكل متحكم.

تدعم المقاييس الخاصة الآن قاعدتي تنبيه في Taikai viewer:
فور `SorafsProofHealthPenalty` عندما
وطن `torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` خلال
آخر 15 دقيقة، بينما يسترشد `SorafsProofHealthCooldown` تحذيرًا إذا بقي مضبطًا في
فترة تهدئة لمدة خمس دقائق. كلا التنبيهين موجود في
`dashboards/alerts/taikai_viewer_rules.yml` ليحصل على SREs على تركيبة فورية عند التقييم
تطبيق PoR/PoTR.

### أسطح المُنسِّق| المقياس / الحدث | النوع | التسميات | المنتِج | مذكرة |
|----------------|-------|--------|--------|-----------|
| `sorafs_orchestrator_active_fetches` | مقياس | `manifest_id`، `region` | `FetchMetricsCtx` | الجلسات الحالية. |
| `sorafs_orchestrator_fetch_duration_ms` | الرسم البياني | `manifest_id`، `region` | `FetchMetricsCtx` | هيستوغرام المدة بالميلي ثانية؛ نطاقات 1 مللي ثانية إلى 30 ثانية. |
| `sorafs_orchestrator_fetch_failures_total` | عداد | `manifest_id`، `region`، `reason` | `FetchMetricsCtx` | لذلك: `no_providers`، `no_healthy_providers`، `no_compatible_providers`، `exhausted_retries`، `observer_failed`، `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | عداد | `manifest_id`، `provider_id`، `reason` | `FetchMetricsCtx` | أسباب أسباب إعادة المحاولة (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | عداد | `manifest_id`، `provider_id`، `reason` | `FetchMetricsCtx` | يمسك مسجل الجلسة أو تعداد الإخفاقات على مستوى النظر. |
| `sorafs_orchestrator_chunk_latency_ms` | الرسم البياني | `manifest_id`، `provider_id` | `FetchMetricsCtx` | توزيع كمون المقاعد (ms) لتحليل الإنتاجية/SLO. |
| `sorafs_orchestrator_bytes_total` | عداد | `manifest_id`، `provider_id` | `FetchMetricsCtx` | البايتات المسلّمة لكل مانيفست/مزوّد؛ استخرج الإنتاجية عبر `rate()` في PromQL. |
| `sorafs_orchestrator_stalls_total` | عداد | `manifest_id`، `provider_id` | `FetchMetricsCtx` | يحصي الطبقات التي تتجاوز `ScoreboardConfig::latency_cap_ms`. || `telemetry::sorafs.fetch.lifecycle` | حدث مُهيكل | `manifest`، `region`، `job_id`، `event`، `status`، `chunk_count`، `total_bytes`، `provider_candidates`، `retry_budget`، `global_parallel_limit` | `FetchTelemetryCtx` | الإنعكاس حياة أهم (بدء/اكتمال) بحافلة Norito JSON. |
| `telemetry::sorafs.fetch.retry` | حدث مُهيكل | `manifest`، `region`، `job_id`، `provider`، `reason`، `attempts` | `FetchTelemetryCtx` | نماذج لكل سلسلة إعادة محاولة للمقياس؛ `attempts` تحصي المحاولات التراكمية (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | حدث مُهيكل | `manifest`، `region`، `job_id`، `provider`، `reason`، `failures` | `FetchTelemetryCtx` | يُظهر عند تجاوز المتحكم لعتبة الإخفاق. |
| `telemetry::sorafs.fetch.error` | حدث مُهيكل | `manifest`، `region`، `job_id`، `reason`، `provider?`، `provider_reason?`، `duration_ms` | `FetchTelemetryCtx` | سجل فشل نهائي مناسب لابتلاع Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | حدث مُهيكل | `manifest`، `region`، `job_id`، `provider`، `latency_ms`، `bytes` | `FetchTelemetryCtx` | يُطلق عند تجاوز كمون المضبوط (يعكس عدادات المماطلة). |

### أسطح العقد / التكرار| المقياس | النوع | التسميات | مذكرة |
|--------|-------|--------|-----------|
| `sorafs_node_capacity_utilisation_pct` | الرسم البياني | `provider_id` | هيستوغرام OTEL لنسبة استخدام التخزين (يُصدر كـ `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | عداد | `provider_id` | عداد أحادي لعينات PoR الناجحة، مقطع من لقطات المجدول. |
| `sorafs_node_por_failure_total` | عداد | `provider_id` | عداد أحادي لعينات PoR الفاشلة. |
| `torii_sorafs_storage_bytes_*`، `torii_sorafs_storage_por_*` | مقياس | `provider` | معايير Prometheus الحالية للبايتات المستعملة وعمق الطابور وعدادات PoR الحالية. |
| `torii_sorafs_capacity_*`، `torii_sorafs_uptime_bps`، `torii_sorafs_por_bps` | مقياس | `provider` | بيانات النجاح السعة/الجاهزية للمتحكمين في لوحة السعة. |
| `torii_sorafs_por_ingest_backlog`، `torii_sorafs_por_ingest_failures_total` | مقياس | `provider`، `manifest` | عمق التراكم بالإضافة إلى عدادات بالتالي التراكمية المصدّرة عند `/v2/sorafs/por/ingestion/{manifest}` لتغذية لوحة/تنبيه "PoR Stalls". |

### إثبات الاسترجاع في الوقت المناسب (PoTR) وSLA الركاب| المقياس | النوع | التسميات | المنتِج | مذكرة |
|--------|-------|--------|---------|-----------|
| `sorafs_potr_deadline_ms` | الرسم البياني | `tier`، `provider` | منسق بوتر | النهاية النهائية بالميلي ثانية (موجب = محقق). |
| `sorafs_potr_failures_total` | عداد | `tier`، `provider`، `reason` | منسق بوتر | لذلك: `expired`، `missing_proof`، `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | عداد | `provider`، `manifest_id`، `reason` | مراقب جيش تحرير السودان | أطلق عند إخفاق القيادة في تحقيق SLO (كمون، نجاح ناجح). |
| `sorafs_chunk_sla_violation_active` | مقياس | `provider`، `manifest_id` | مراقب جيش تحرير السودان | مقياس بولي (0/1) يتبدل أثناء نافذة الإقفال العضوي. |

## أهداف SLO

- توفر البوابة بدون ثقة: **99.9%** (استجابات HTTP 2xx/304).
- Trustless TTFB P95: الطبقة الساخنة ≥ 120 مللي ثانية، الطبقة الدافئة ≥ 300 مللي ثانية.
- مدى نجاح الأدلة: ≥ 99.5% يومياً.
- نجاح المُنسِّق (اكتمال القادة): ≥ 99%.

## لوحات المتابعة والتنبيهات

1. **قابلية الملاحظة للبوابة** (`dashboards/grafana/sorafs_gateway_observability.json`) — تتبع توفر Trustless وTTFB P95 وتفصيل الرفض وخفاقات PoR/PoTR عبر معايير OTEL.
2. **صحة المُنسِّق** (`dashboards/grafana/sorafs_fetch_observability.json`) — تغطية متعدد الصلاحيات وصلاحيات المحاولة وإخفاقات المنظمين وموجات الأكشاك.
3. **مقاييس خصوصية SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — ترسم دلاء التتابع المجهولة ونوافذ الكتمة جامع عبر `soranet_privacy_last_poll_unixtime` و`soranet_privacy_collector_enabled` و`soranet_privacy_poll_errors_total{provider}`.

حزم التنبيهات:- `dashboards/alerts/sorafs_gateway_rules.yml` — توفر البوابة وTTFB لعدم وجود الأدلة.
- `dashboards/alerts/sorafs_fetch_rules.yml` — إخفاقات/إعادات المحاولة/stalls للمُنسِّق؛ يتم التحقق من خلال `scripts/telemetry/test_sorafs_fetch_alerts.sh` و`dashboards/alerts/tests/sorafs_fetch_rules.test.yml` و`dashboards/alerts/tests/soranet_privacy_rules.test.yml` و`dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — قمم تنوع الخصوصية وانذارات الكتم ورصد Collector براكل وتنبيهات Collector المعطل (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — إنذارات Brownout للخصوصية مربوطة بـ `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — إنذارات الانجراف/التناول/CEK lag في Taikai viewer إضافة إلى تنبيهات عقوبة/cooldown تثبت الأدلة في SoraFS المبنية على `torii_sorafs_proof_health_*`.

## استراتيجية التتبع

- اعتماد OpenTelemetry من الطرف للطرف:
  - أصبحت بوابات OTLP تمتد (HTTP) ذات معرفات الطلب وملخصات المانيفيست والرموز المميزة.
  - يستخدم المُنسِّق `tracing` + `opentelemetry` لتصدير المسافات لمحاولات الجلب.
  - تزايد عقد SoraFS يمتد لتحديات PoR بداية التشغيل. تشترك في كل المكونات في معرف التتبع المتحرك عبر `x-sorafs-trace`.
- ربط `SorafsFetchOtel` مقاييس المُنسِّق بهيستوغرامات OTLP أثناء أحداث `telemetry::sorafs.fetch.*` حمولات JSON سريعة لخلفيات تركّز على تسجيل.
- الجامعون: شغّل OTEL هواة الجمع Bebe Prometheus/Loki/Tempo (Tempo مفضل). ما بعد المُصدِّرات المتوافقة مع جايجر اختيارية.
- يجب أخذ عينات من السيارات الكاردينالية العالية (10% تورات النجاح، 100% للإخفاقات).

## التنسيق التلمتري TLS (SF-5b)- مواءمة المعايير:
  - يستخدم لـ TLS `sorafs_gateway_tls_cert_expiry_seconds` و`sorafs_gateway_tls_renewal_total{result}` و`sorafs_gateway_tls_ech_enabled`.
  - أدرج هذه المعايير في لوحة البوابة نظرة عامة ضمن لوحة TLS/Certificates.
- ربط التنبيهات:
  - عند انتهاء تنبيهات انتهاء TLS (≥ 14 يومًا متبقّيًا) ربطها مع SLO توفر Trustless.
  - يقود ظهور ECH إلى تنبيه ثانوي يشير إلى لوحات TLS والتوفر للجميع.
- خط الأنابيب: مهمة محاسبة TLS إلى نفس مكدس Prometheus الذي يستخدمه gate؛ يضمن وجود SF-5b في إزالة التكرار في القياس.

## اصطلاحات معايير المعايير والـLabels- تتبع أسماء المقاييس في مبادئ `torii_sorafs_*` أو `sorafs_*` المستخدمة بواسطة Torii والبوابة.
- مجموعات الـLabels الموحدة:
  - `result` → مخرجات HTTP (`success`, `refused`, `failed`).
  - `reason` → رمز الرفض/الخطأ (`unsupported_chunker`, `timeout`, إلخ).
  - `provider` → معرف المتحكم مرمز بالهيكس.
  - `manifest` → ملخص مانيفيست قانوني (يتم سليمه عند ارتفاع الكاردينالية).
  - `tier` → التسميات الطبقية التعريفية (`hot`، `warm`، `archive`).
- نقاط الإصدار التليميترية:
  - معايير الشبكة الحية تحت `torii_sorafs_*` وتعيد استخدام اصطلاحات `crates/iroha_core/src/telemetry.rs`.
  - يصدر المُنسِّق مقاييس `sorafs_orchestrator_*` وأحداث `telemetry::sorafs.fetch.*` (دورة الحياة، إعادة المحاولة، فشل الموفر، الخطأ، المماطلة) بعلامات ملخص المانيفست ومعرف الوظيفة والمنطقة ومعرفات المحاسبين.
  - تم التعرض للعقد `torii_sorafs_storage_*` و`torii_sorafs_capacity_*` و`torii_sorafs_por_*`.
- نسّق مع إمكانية الملاحظة لتسجيل كتالوج المعايير في الاتصال بأسماء Prometheus المشتركة، بما في ذلك توقعات كاردينالية الـLabels (الحدود العليا للمتحكم/المانيفست).

## خط بيانات الأنابيب

- تُنشر هواة جمع العملات Become كل مكوّن، وتصدر OTLP إلى Prometheus (معايير) وLoki/Tempo (سجلات/تتبعات).
- يثري eBPF الاختياري (Tetragon) التتبع المستوى المنخفض للبوابات/العقد.
- استخدم `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` لـ Torii والعقدة المدمجة؛ استمر المُنسِّق في الاتصال `install_sorafs_fetch_otlp_exporter`.

##خطافات التحقق- شغّل `scripts/telemetry/test_sorafs_fetch_alerts.sh` أثناء CI وأدام بقاء متطلبات التنبيه Prometheus متزامنة مع معايير المماطلة وفحوصات كتم الخصوصية.
- حافظ على اللوحات Grafana ضمن التحكم بالإصدارات (`dashboards/grafana/`) وتعديل الصورة/الروابط عند تغيير اللوحات.
-كامل عمليات الفوضى النتائج عبر `scripts/telemetry/log_sorafs_drill.sh`؛ يستخدم شهادة `scripts/telemetry/validate_drill_log.sh` (راجع [دليل العمليات](operations-playbook.md)).