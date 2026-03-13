---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/observability-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة المراقبة
العنوان: Plan d'observabilité et de SLO de SoraFS
Sidebar_label: إمكانية المراقبة وSLO
الوصف: مخطط عن بعد ولوحات المعلومات وسياسة ميزانية الخطأ للبوابات SoraFS والبيانات والمنسق متعدد المصادر.
---

:::ملاحظة المصدر الكنسي
تعكس هذه الصفحة الخطة الحالية في `docs/source/sorafs_observability_plan.md`. تمت مزامنة النسختين حتى الهجرة الكاملة لفرقة Sphinx القديمة.
:::

## الأهداف
- تحديد المقاييس والأحداث المنظمة للبوابات والبيانات والمنسق متعدد المصادر.
- توفر لوحات المعلومات Grafana ومتابعة التنبيهات وخطافات التحقق من الصحة.
- إعداد الأهداف SLO باستخدام سياسة ميزانية الخطأ وتدريبات الفوضى.

## كتالوج المقاييس

### أسطح دو البوابة| متريك | اكتب | آداب | ملاحظات |
|---------|------|-----------|-------|
| `sorafs_gateway_active` | مقياس (UpDownCounter) | `endpoint`، `method`، `variant`، `chunker`، `profile` | إميس عبر `SorafsGatewayOtel` ; تتناسب مع عمليات HTTP في المجلد من خلال الجمع بين نقطة النهاية/الطريقة. |
| `sorafs_gateway_responses_total` | عداد | `endpoint`، `method`، `variant`، `chunker`، `profile`، `result`، `status`، `error_code` | كل طلب بوابة ينتهي بزيادة مرة واحدة ; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | الرسم البياني | `endpoint`، `method`، `variant`، `chunker`، `profile`، `result`، `status`، `error_code` | زمن الوصول إلى البايت الأول لبوابة الردود ؛ تم تصديره إلى Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | عداد | `profile_version`، `result`، `error_code` | نتائج التحقق من اللقطات الملتقطة في لحظة الطلب (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | الرسم البياني | `profile_version`، `result`، `error_code` | توزيع زمن الوصول للتحقق من صحة النتائج. || `telemetry::sorafs.gateway.request` | هيكل الحدث | `endpoint`، `method`، `variant`، `result`، `status`، `error_code`، `duration_ms` | يتم إنشاء السجل حسب كل طلب للارتباط Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`، `torii_sorafs_gateway_refusals_total` | عداد | العاب آداب موروثة | يتم الاحتفاظ بالمقاييس Prometheus للوحات المعلومات التاريخية؛ يصدر بالتوازي مع المسلسل الجديد OTLP. |

تعكس الأحداث `telemetry::sorafs.gateway.request` حاسبات OTEL مع هياكل الحمولات الصافية، وتعرض `endpoint`، و`method`، و`variant`، و`status`، `error_code` و`duration_ms` للارتباط Loki/Tempo، بينما تستخدم لوحات المعلومات سلسلة OTLP لمتابعة SLO.

### Télémétrie de santé des preuves| متريك | اكتب | آداب | ملاحظات |
|---------|------|-----------|-------|
| `torii_sorafs_proof_health_alerts_total` | عداد | `provider_id`، `trigger`، `penalty` | قم بزيادة كل مرة حتى يكون `RecordCapacityTelemetry` هو `SorafsProofHealthAlert`. `trigger` يميز بين تأثيرات PDP/PoTR/Both، بينما يلتقط `penalty` ما إذا كانت الضمانات قد تم قطعها أو حذفها من خلال فترة التهدئة. |
| `torii_sorafs_proof_health_pdp_failures`، `torii_sorafs_proof_health_potr_breaches` | مقياس | `provider_id` | تم الإبلاغ عن آخر حسابات PDP/PoTR في نافذة القياس عن بعد الخاطئة لكي تقيس المعدات تراجع السياسة من قبل الموردين. |
| `torii_sorafs_proof_health_penalty_nano` | مقياس | `provider_id` | يتم بتر Montant Nano-XOR في التنبيه الأخير (صفر عند إيقاف تشغيل التطبيق). |
| `torii_sorafs_proof_health_cooldown` | مقياس | `provider_id` | مقياس منطقي (`1` = تنبيه محذوف من فترة التهدئة) للإشارة عندما تكون التنبيهات اللاحقة صامتة مؤقتًا. |
| `torii_sorafs_proof_health_window_end_epoch` | مقياس | `provider_id` | تم تسجيل الوقت لنافذة القياس عن بعد للتنبيه حتى يتمكن المشغلون من الاتصال بالعناصر Norito. |

هذا التدفق الغذائي يعطل خط حماية صحة لوحة القيادة Taikai viewer
(`dashboards/grafana/taikai_viewer.json`)، يقدم لمشغلي CDN رؤية مباشرة
على أحجام التنبيهات، ومزيج مشغلات PDP/PoTR، والعقوبات، وحالة التهدئة بالتساوي
com.fourniseur.تساعد المقاييس المماثلة مع الحفاظ على قواعد التنبيه لعارض Taikai:
يتم إغلاق `SorafsProofHealthPenalty` عند إغلاقه
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` زيادة
خلال 15 دقيقة أخرى، ثم `SorafsProofHealthCooldown` يصدر تحذيرًا إذا كان
Fournisseur Reste En Cooldown قلادة لمدة خمس دقائق. ينبض بالحياة في التنبيهات الثنائية
`dashboards/alerts/taikai_viewer_rules.yml` لكي يتم التخلص من SREs في سياق فوري
عندما يتم تكثيف تطبيق PoR/PoTR.

### أسطح الأوركسترا| متريك / حدث | اكتب | آداب | المنتج | ملاحظات |
|----------------------|------|------------|------------|-------|
| `sorafs_orchestrator_active_fetches` | مقياس | `manifest_id`، `region` | `FetchMetricsCtx` | الجلسات الفعلية في المجلد. |
| `sorafs_orchestrator_fetch_duration_ms` | الرسم البياني | `manifest_id`، `region` | `FetchMetricsCtx` | الرسم البياني للمدة بالمللي ثانية ; دلاء 1 مللي ثانية إلى 30 ثانية. |
| `sorafs_orchestrator_fetch_failures_total` | عداد | `manifest_id`، `region`، `reason` | `FetchMetricsCtx` | الأسباب: `no_providers`، `no_healthy_providers`، `no_compatible_providers`، `exhausted_retries`، `observer_failed`، `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | عداد | `manifest_id`، `provider_id`، `reason` | `FetchMetricsCtx` | قم بالتمييز بين أسباب إعادة المحاولة (`retry`، `digest_mismatch`، `length_mismatch`، `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | عداد | `manifest_id`، `provider_id`، `reason` | `FetchMetricsCtx` | التقط عمليات إلغاء التنشيط وحسابات الشيكات على مستوى الجلسة. |
| `sorafs_orchestrator_chunk_latency_ms` | الرسم البياني | `manifest_id`، `provider_id` | `FetchMetricsCtx` | توزيع زمن الوصول للقطعة الاسمية (ملي ثانية) لتحليل الإنتاجية/SLO. |
| `sorafs_orchestrator_bytes_total` | عداد | `manifest_id`، `provider_id` | `FetchMetricsCtx` | ثمانيات كتب حسب البيان/المصدر؛ قم بتقليل الإنتاجية عبر `rate()` في PromQL. || `sorafs_orchestrator_stalls_total` | عداد | `manifest_id`، `provider_id` | `FetchMetricsCtx` | احسب القطع التي تم قطعها `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | هيكل الحدث | `manifest`، `region`، `job_id`، `event`، `status`، `chunk_count`، `total_bytes`، `provider_candidates`، `retry_budget`، `global_parallel_limit` | `FetchTelemetryCtx` | قم بإعادة عرض دورة حياة المهمة (البدء/الإكمال) مع الحمولة النافعة JSON Norito. |
| `telemetry::sorafs.fetch.retry` | هيكل الحدث | `manifest`، `region`، `job_id`، `provider`، `reason`، `attempts` | `FetchTelemetryCtx` | Émis par خط إعادة المحاولة بواسطة المورد ; `attempts` يحسب المحاولات المتزايدة (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | هيكل الحدث | `manifest`، `region`، `job_id`، `provider`، `reason`، `failures` | `FetchTelemetryCtx` | تم النشر عندما قام المورّد بمنح حق الوصول إلى الشيكات. |
| `telemetry::sorafs.fetch.error` | هيكل الحدث | `manifest`، `region`، `job_id`، `reason`، `provider?`، `provider_reason?`، `duration_ms` | `FetchTelemetryCtx` | تسجيل التوقف النهائي، متكيف مع استيعاب Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | هيكل الحدث | `manifest`، `region`، `job_id`، `provider`، `latency_ms`، `bytes` | `FetchTelemetryCtx` | يتم ذلك عندما يتجاوز زمن الاستجابة الحد الذي تم تكوينه (يعكس حاسبات التوقف). |### السطوح nœud / réplication

| متريك | اكتب | آداب | ملاحظات |
|---------|------|-----------|-------|
| `sorafs_node_capacity_utilisation_pct` | الرسم البياني | `provider_id` | الرسم البياني OTEL لنسبة استخدام المخزون (التصدير في `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | عداد | `provider_id` | قم بحساب نغمة الأغاني الرتيبة من خلال إعادة الاستخدام، واشتقاق لقطات الجدولة. |
| `sorafs_node_por_failure_total` | عداد | `provider_id` | Compteur monotone des échantillons PoR échoué. |
| `torii_sorafs_storage_bytes_*`، `torii_sorafs_storage_por_*` | مقياس | `provider` | توجد أجهزة قياس Prometheus للثمانيات المستخدمة، وعمق الملف، وحساب PoR وحجم الصوت. |
| `torii_sorafs_capacity_*`، `torii_sorafs_uptime_bps`، `torii_sorafs_por_bps` | مقياس | `provider` | تظهر البيانات المتعلقة بالسعة/وقت التشغيل للمورد في لوحة معلومات السعة. |
| `torii_sorafs_por_ingest_backlog`، `torii_sorafs_por_ingest_failures_total` | مقياس | `provider`، `manifest` | عمق العمل المتراكم بالإضافة إلى أجهزة الكمبيوتر المتراكمة من الشيكات المصدرة لكل استفسار من `/v2/sorafs/por/ingestion/{manifest}`، مع تشغيل اللوحة/التنبيه "PoR Stalls". |

### منع استعادة الوقت المفيد (PoTR) وSLA للقطع| متريك | اكتب | آداب | المنتج | ملاحظات |
|---------|------|-----------|------------|-------|
| `sorafs_potr_deadline_ms` | الرسم البياني | `tier`، `provider` | منسق PoTR | هامش الموعد النهائي بالمللي ثانية (إيجابي = احترام). |
| `sorafs_potr_failures_total` | عداد | `tier`، `provider`، `reason` | منسق PoTR | الأسباب: `expired`، `missing_proof`، `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | عداد | `provider`، `manifest_id`، `reason` | مراقبة جيش تحرير السودان | قم بإلغاء القفل عند تشغيل القطع بمعدل SLO (زمن الاستجابة، مدة النجاح). |
| `sorafs_chunk_sla_violation_active` | مقياس | `provider`، `manifest_id` | مراقبة جيش تحرير السودان | مقياس منطقي (0/1) نشط أثناء نافذة الانتهاك النشطة. |

## كائنات SLO

- البوابة غير الموثوقة متاحة : **99.9%** (الاستجابات HTTP 2xx/304).
- TTFB P95 Trustless: الطبقة الساخنة ≥ 120 مللي ثانية، الطبقة الدافئة ≥ 300 مللي ثانية.
- مجموع النجاحات السابقة : ≥ 99.5% في اليوم.
- نجاح المنسق (الانتهاء من القطع): ≥ 99%.

## لوحات المعلومات والتنبيهات1. **بوابة المراقبة** (`dashboards/grafana/sorafs_gateway_observability.json`) — تناسب التوفر الموثوق به، TTFB P95، إعادة تقسيم الرفض وفحص PoR/PoTR عبر مقاييس OTEL.
2. **Santé de l'orchesstrateur** (`dashboards/grafana/sorafs_fetch_observability.json`) — يغطي الشحنة متعددة المصادر، والمحاولات، والتجهيزات المجهزة، وطائرات الأكشاك.
3. **مقاييس السرية SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — تتبع مجموعات التتابع المجهولة ونوافذ القمع وصحة المجمعات عبر `soranet_privacy_last_poll_unixtime` و`soranet_privacy_collector_enabled` و`soranet_privacy_poll_errors_total{provider}`.

باقات التنبيه :

- `dashboards/alerts/sorafs_gateway_rules.yml` — بوابة متاحة، TTFB، صور فحص مسبقة.
- `dashboards/alerts/sorafs_fetch_rules.yml` - عمليات التحقق/إعادة المحاولة/أكشاك المدير؛ تم التحقق منه عبر `scripts/telemetry/test_sorafs_fetch_alerts.sh` و`dashboards/alerts/tests/sorafs_fetch_rules.test.yml` و`dashboards/alerts/tests/soranet_privacy_rules.test.yml` و`dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` - صور فقدان السرية، إنذارات القمع، اكتشاف المجمع غير النشط وتنبيهات المجمع المعطل (`soranet_privacy_last_poll_unixtime`، `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — إنذارات انقطاع التيار الكهربائي عن الكابلات المجهولة على `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — إنذارات الانجراف/الدخول/CEK lag Taikai viewer بالإضافة إلى التنبيهات الجديدة للعقوبات/التبريد الصحي من SoraFS المغذية بواسطة `torii_sorafs_proof_health_*`.

## استراتيجية التتبع- اعتماد OpenTelemetry في كل من:
  - تقوم البوابات بتعليق امتدادات OTLP (HTTP) مع معرفات الطلب وملخصات البيان وتجزئة الرمز المميز.
  - يستخدم المُنسق `tracing` + `opentelemetry` لتصدير نطاقات الجلب التجريبية.
  - يتم تصدير الامتدادات SoraFS من المسافات لتحديات PoR وعمليات التخزين. جميع المكونات تشارك في نشر معرف التتبع عبر `x-sorafs-trace`.
- يعتمد `SorafsFetchOtel` على المقاييس المنسقة على الرسوم البيانية OTLP بينما توفر الأحداث `telemetry::sorafs.fetch.*` حمولات JSON خفيفة للسجلات الخلفية المركزية.
- المجمعون : تنفيذ المجمعات OTEL à côté de Prometheus/Loki/Tempo (Tempo préféré). لدى المصدرين API Jaeger خيارات.
- يجب أن تكون العمليات الأساسية عالية الجودة (10% لطرق النجاح، 100% للنتائج).

## تنسيق القياس عن بعد TLS (SF-5b)- محاذاة المقاييس :
  - نشر أتمتة TLS `sorafs_gateway_tls_cert_expiry_seconds` و`sorafs_gateway_tls_renewal_total{result}` و`sorafs_gateway_tls_ech_enabled`.
  - قم بتضمين هذه المقاييس في لوحة المعلومات الخاصة بالبوابة نظرة عامة على لوحة TLS/الشهادات.
- اتصال التنبيهات :
  - عندما تنخفض تنبيهات انتهاء صلاحية TLS (أقل من 14 يومًا متبقية)، يتم الاتصال بـ SLO de disponibilité الموثوق به.
  - يؤدي إلغاء تنشيط ECH إلى تنبيه ثانٍ يشير إلى لوحات TLS المتوفرة.
- خط الأنابيب: يتم تصدير مهمة أتمتة TLS إلى نفس المكدس Prometheus من بوابة المقاييس؛ يضمن التنسيق مع SF-5b استخدام أداة مكررة.

## اتفاقيات تسمية وتقييد المقاييس- أسماء المقاييس تتبع البادئات الموجودة `torii_sorafs_*` أو `sorafs_*` المستخدمة بواسطة Torii والبوابة.
- مجموعات الآداب موحدة :
  - `result` → نتيجة HTTP (`success`، `refused`، `failed`).
  - `reason` → رمز الرفض/الخطأ (`unsupported_chunker`، `timeout`، وما إلى ذلك).
  - `provider` → معرف الموفر المشفر بالسداسي عشري.
  - `manifest` → ملخص الكنسي للبيان (tronqué quand la Cardinalité est élevée).
  - `tier` → تسميات الطبقة المعلنة (`hot`، `warm`، `archive`).
- نقاط البث عن بعد :
  - تعمل بوابة المقاييس على `torii_sorafs_*` وتعيد استخدام اصطلاحات `crates/iroha_core/src/telemetry.rs`.
  - قام المنسق بضبط المقاييس `sorafs_orchestrator_*` والأحداث `telemetry::sorafs.fetch.*` (دورة الحياة، إعادة المحاولة، فشل الموفر، الخطأ، التوقف) مع ملخص البيان ومعرف الوظيفة والمنطقة والمعرفات المقدمة.
  - عرض العناصر `torii_sorafs_storage_*` و`torii_sorafs_capacity_*` و`torii_sorafs_por_*`.
- التنسيق مع إمكانية المراقبة لتسجيل كتالوج القياسات في مستند الاسم Prometheus Partagé، ويتضمن علامات الاهتمام الأساسية للملصقات (المحمولة من قبل الموردين/البيانات).

## خط أنابيب البيانات- يتم نشر المجمعات على كل قطعة مكونة من OTLP إلى Prometheus (المقاييس) وLoki/Tempo (السجلات/الآثار).
- يعمل خيار eBPF (Tetragon) على تحسين التتبع على المستوى الأساسي للبوابات/الأرقام.
-استخدم `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` لـ Torii والأشياء الجديدة ؛ l'orchesstrateur continue d'appeler `install_sorafs_fetch_otlp_exporter`.

## خطافات التحقق من الصحة

- قم بتنفيذ `scripts/telemetry/test_sorafs_fetch_alerts.sh` في CI لضمان محاذاة قواعد التنبيه Prometheus مع مقاييس التوقف وعمليات التحقق من إخفاء السرية.
- قم بحماية لوحات المعلومات Grafana من خلال التحكم في الإصدار (`dashboards/grafana/`) واضغط على اللقطات/الارتباطات عند تغيير اللوحات.
- يتم إرسال تدريبات الفوضى إلى النتائج عبر `scripts/telemetry/log_sorafs_drill.sh` ؛ يتم الضغط على التحقق من الصحة على `scripts/telemetry/validate_drill_log.sh` (يعرض [Playbook d'exploitation](operations-playbook.md)).