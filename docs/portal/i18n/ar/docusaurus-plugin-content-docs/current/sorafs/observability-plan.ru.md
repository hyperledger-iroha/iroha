---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/observability-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة المراقبة
العنوان: خطة المراقبة وSLO لـ SoraFS
Sidebar_label: Наблюдаемость وSLO
الوصف: مخطط أجهزة القياس عن بعد ولوحات البيانات والميزانية السياسية للبوابات SoraFS والموزعين الموسيقيين والمتعددين الأصيلين.
---

:::note Канонический источник
هذا الجزء من الخطة ينفذ إلى `docs/source/sorafs_observability_plan.md`. قم بالنسخ المتزامن، حيث أن مجموعة أبو الهول القديمة ليست كاملة الهجرة.
:::

## كيلي
- تحديد المقاييس والتنظيمات الهيكلية للبوابات والأنظمة الموسيقية والمنسقة الموسيقية المتعددة.
- اقتراح لوحة الاتصال Grafana والتنبيهات الأولية وخطافات التحقق من الصحة.
- تثبيت جميع SLO بعيدًا عن الميزانية السياسية والألعاب الفوضوية.

## مقياس الكتالوج

### بوابة شاملة| متريكا | النوع | ميتكي | مساعدة |
|---------|-----|-------|------------|
| `sorafs_gateway_active` | مقياس (UpDownCounter) | `endpoint`، `method`، `variant`، `chunker`، `profile` | يتم التخلص منه من خلال `SorafsGatewayOtel`; تتبع عمليات HTTP من خلال الجمع بين نقطة النهاية/الطريقة. |
| `sorafs_gateway_responses_total` | عداد | `endpoint`، `method`، `variant`، `chunker`، `profile`، `result`، `status`، `error_code` | كل عملية تشغيل البوابة الأخيرة تزيد من ذكاءك مرة واحدة; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | الرسم البياني | `endpoint`، `method`، `variant`، `chunker`، `profile`، `result`، `status`، `error_code` | زمن الوصول إلى البايت الأول لبوابة الإرسال؛ يتم التصدير كـ Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | عداد | `profile_version`، `result`، `error_code` | يتم إصلاح نتائج التحقق من المطابقة في لحظة واحدة (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | الرسم البياني | `profile_version`، `result`، `error_code` | فحوصات النطاق الخفي لنسخ PoR. || `telemetry::sorafs.gateway.request` | البنية الهيكلية | `endpoint`، `method`، `variant`، `result`، `status`، `error_code`، `duration_ms` | السجل الهيكلي متاح عند الانتهاء من عملية المراسلات Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`، `torii_sorafs_gateway_refusals_total` | عداد | طرق متقدمة للمجموعات | المقاييس Prometheus، المحمية بألواح تاريخية؛ تغادر الآن مع سلسلة جديدة من OTLP. |

يتم توصيل `telemetry::sorafs.gateway.request` بشبكات OTEL بالحمولات الهيكلية، مثل `endpoint`، `method`، `variant`، `status` و`error_code` و`duration_ms` للارتباطات في Loki/Tempo، وكيفية استخدام لوحات المفاتيح لسلسلة OTLP مراقبة SLO.

### توصيل أجهزة القياس عن بعد| متريكا | النوع | ميتكي | مساعدة |
|---------|-----|-------|------------|
| `torii_sorafs_proof_health_alerts_total` | عداد | `provider_id`، `trigger`، `penalty` | يتم زيادة عدد المرات عندما يتم حذف `RecordCapacityTelemetry` من `SorafsProofHealthAlert`. يعرض `trigger` كلاً من PDP/PoTR/Both، ويوضح `penalty`، أنه تم تضمينه في قائمة أو دعم فترة التهدئة. |
| `torii_sorafs_proof_health_pdp_failures`، `torii_sorafs_proof_health_potr_breaches` | مقياس | `provider_id` | البداية التالية لـ PDP/PoTR في مشكلة أجهزة القياس عن بعد التي يمكن أن تؤدي الأوامر إلى زيادة عدد مقدمي الخدمة في السياسة. |
| `torii_sorafs_proof_health_penalty_nano` | مقياس | `provider_id` | ملخص Nano-XOR، معروض في التنبيه التالي (مرة أخرى، إذا تم تنفيذ فترة التهدئة). |
| `torii_sorafs_proof_health_cooldown` | مقياس | `provider_id` | مقياس البوليفي (`1` = تنبيه لتهدئة التباطؤ)، للإعلان عن التنبيهات اللاحقة في الوقت المناسب. |
| `torii_sorafs_proof_health_window_end_epoch` | مقياس | `provider_id` | هناك أجهزة قياس عن بعد متصلة بالتنبيه بحيث يمكن للمشغلين الاتصال بالمنتجات Norito. |

هذه العناصر تضغط على دليل الصحة في Taikai viewer
(`dashboards/grafana/taikai_viewer.json`)، يعمل مشغل CDN هذا على قيد الحياة
قم بتنبيهات رائعة، قم بتشغيل PDP/PoTR والعقوبات وشروط التهدئة
مقدم.تدعم هذه المقاييس أيضًا تنبيهين مناسبين لعارض Taikai:
`SorafsProofHealthPenalty` جاهز عندما
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` مرة أخرى
لمدة 15 دقيقة الماضية، سيتم إرسال تحذير `SorafsProofHealthCooldown`، إذا
يبقى الموفر في فترة تهدئة لمدة دقيقة واحدة. أوبا تنبيه للحياة في
`dashboards/alerts/taikai_viewer_rules.yml`، للحصول على سياق SRE
بشكل متقطع عند تصعيد التصعيد PoR/PoTR.

### الأوركيستراتور الشامل| متريكا / سوبيتيي | النوع | ميتكي | منتج | مساعدة |
|-------------------|-----|-------|--------------|------------|
| `sorafs_orchestrator_active_fetches` | مقياس | `manifest_id`، `region` | `FetchMetricsCtx` | سيسيسي، تواجد في متناول اليد. |
| `sorafs_orchestrator_fetch_duration_ms` | الرسم البياني | `manifest_id`، `region` | `FetchMetricsCtx` | تسجيل البيانات في المللي ثانية; دلاء من 1 مللي ثانية إلى 30 ثانية. |
| `sorafs_orchestrator_fetch_failures_total` | عداد | `manifest_id`، `region`، `reason` | `FetchMetricsCtx` | الأسباب: `no_providers`، `no_healthy_providers`، `no_compatible_providers`، `exhausted_retries`، `observer_failed`، `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | عداد | `manifest_id`، `provider_id`، `reason` | `FetchMetricsCtx` | قم بتحليل الأسباب (`retry`، `digest_mismatch`، `length_mismatch`، `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | عداد | `manifest_id`، `provider_id`، `reason` | `FetchMetricsCtx` | أكمل الاستثناءات والإضافات في دورة حياتك. |
| `sorafs_orchestrator_chunk_latency_ms` | الرسم البياني | `manifest_id`، `provider_id` | `FetchMetricsCtx` | جلب اختفاء النطاق على قطعة (مللي ثانية) لتحليل الإنتاجية/SLO. |
| `sorafs_orchestrator_bytes_total` | عداد | `manifest_id`، `provider_id` | `FetchMetricsCtx` | الطعوم التي يتم توصيلها إلى المانيفست/المزود; يتم الوصول إلى الإنتاجية من خلال `rate()` في PromQL. |
| `sorafs_orchestrator_stalls_total` | عداد | `manifest_id`، `provider_id` | `FetchMetricsCtx` | اقرأ القطع السابقة `ScoreboardConfig::latency_cap_ms`. || `telemetry::sorafs.fetch.lifecycle` | البنية الهيكلية | `manifest`، `region`، `job_id`، `event`، `status`، `chunk_count`، `total_bytes`، `provider_candidates`، `retry_budget`، `global_parallel_limit` | `FetchTelemetryCtx` | قم بإنهاء مهمة الدورة الحيوية (البدء/الإكمال) باستخدام حمولة Norito JSON. |
| `telemetry::sorafs.fetch.retry` | البنية الهيكلية | `manifest`، `region`، `job_id`، `provider`، `reason`، `attempts` | `FetchTelemetryCtx` | يتم التخلص منها من خلال مقدم سلسلة الإرجاع; `attempts` يقرأ الإرجاعات التزايدية (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | البنية الهيكلية | `manifest`، `region`، `job_id`، `provider`، `reason`، `failures` | `FetchTelemetryCtx` | يتم النشر عندما يقوم المزود بالإجابة مرة أخرى. |
| `telemetry::sorafs.fetch.error` | البنية الهيكلية | `manifest`، `region`، `job_id`، `reason`، `provider?`، `provider_reason?`، `duration_ms` | `FetchTelemetryCtx` | الرسالة النهائية التي يمكنك استيعابها بسهولة في Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | البنية الهيكلية | `manifest`، `region`، `job_id`، `provider`، `latency_ms`، `bytes` | `FetchTelemetryCtx` | يجب أن يتم ذلك عندما يتم تجاوز الحد الأقصى للجزء المتبقي (إزالة المماطلة). |

### الاستخدام الشامل / النسخ المتماثلة| متريكا | النوع | ميتكي | مساعدة |
|---------|-----|-------|------------|
| `sorafs_node_capacity_utilisation_pct` | الرسم البياني | `provider_id` | يتم استخدام نسبة مئوية من OTEL للنبيذ (يتم تصديره كـ `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | عداد | `provider_id` | مدقق رتيب ناجح PoR-Vыborok، متقن من جدولة اللقطات. |
| `sorafs_node_por_failure_total` | عداد | `provider_id` | monotonnый счетчик неуспезный PoR-выборок. |
| `torii_sorafs_storage_bytes_*`، `torii_sorafs_storage_por_*` | مقياس | `provider` | مقياس Prometheus للإستخدامات المستخدمة في الطعوم والنظارات وPoR أثناء الطيران. |
| `torii_sorafs_capacity_*`، `torii_sorafs_uptime_bps`، `torii_sorafs_por_bps` | مقياس | `provider` | شكرًا لك على توفير السعة/وقت التشغيل، وسيتم عرضها في لوحة الوصول. |
| `torii_sorafs_por_ingest_backlog`، `torii_sorafs_por_ingest_failures_total` | مقياس | `provider`، `manifest` | تراكم الأعمال المتراكمة بالإضافة إلى مجموعة كاملة من أدوات الحماية، التي يتم تصديرها عند إجراء عملية `/v1/sorafs/por/ingestion/{manifest}`، قم بتشغيل اللوحة/التنبيه "PoR Stalls". |

### إثبات الاسترجاع في الوقت المناسب (PoTR) وSLA على أجزاء| متريكا | النوع | ميتكي | منتج | مساعدة |
|---------|-----|--------------|------------|------|--------|
| `sorafs_potr_deadline_ms` | الرسم البياني | `tier`، `provider` | منسق PoTR | Запас по дедлайну в ميلي ثانية (пологительный = выполнен). |
| `sorafs_potr_failures_total` | عداد | `tier`، `provider`، `reason` | منسق PoTR | الأسباب: `expired`، `missing_proof`، `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | عداد | `provider`، `manifest_id`، `reason` | مراقب جيش تحرير السودان | قم بالاتصال عندما لا يتم تجميع القطع في SLO (السرعة الكامنة، معدل النجاح). |
| `sorafs_chunk_sla_violation_active` | مقياس | `provider`، `manifest_id` | مراقب جيش تحرير السودان | مقياس بوليفي (0/1)، يتم إيقافه أثناء الضغط النشط. |

## سيلي سلو

- بوابة موثوقة وموثوقة: **99.9%** (إجابات HTTP 2xx/304).
- TTFB P95 غير الموثوق به: الطبقة الساخنة ≥ 120 مللي ثانية، الطبقة الدافئة ≥ 300 مللي ثانية.
- جودة التوصيل: ≥ 99.5% في اليوم.
- أوركسترا ممتازة (قطع مقطوعة): ≥ 99%.

## لوحات المعلومات والتنبيهات1. **بوابة المراقبة** (`dashboards/grafana/sorafs_gateway_observability.json`) — قم بمراقبة التوافر غير الموثوق به وTTFB P95 وتقسيم البيانات وPoR/PoTR من خلال مقاييس OTEL.
2. **مدير الأوركسترا** (`dashboards/grafana/sorafs_fetch_observability.json`) — يقوم بإنشاء مصادر متعددة ورجعية ومزودين وأكشاك أخرى.
3. **مقاييس الخصوصية SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — دلاء ترحيل الرسومات المجهولة الهوية، ووحدة التجميع ومجمع الطاقة عبر `soranet_privacy_last_poll_unixtime`، `soranet_privacy_collector_enabled` و`soranet_privacy_poll_errors_total{provider}`.

تنبيهات الحزم:

- `dashboards/alerts/sorafs_gateway_rules.yml` — بوابة الوصول، TTFB، جميع التصريحات.
- `dashboards/alerts/sorafs_fetch_rules.yml` — отказы/retраи/stalls окестратора; يتم التحقق من صحته عبر `scripts/telemetry/test_sorafs_fetch_alerts.sh`، و`dashboards/alerts/tests/sorafs_fetch_rules.test.yml`، و`dashboards/alerts/tests/soranet_privacy_rules.test.yml`، و`dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — تدهور الخصوصية بالكامل، وتفعيل الإنذارات، والكشف عن المجمع الخامل، والتنبيهات خارج نطاق المجمع (`soranet_privacy_last_poll_unixtime`، `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — إنذارات انقطاع التيار الكهربائي مجهولة المصدر، متاحة إلى `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` - إنذارات дейфа/inest/CEK lag Taikai viewer بالإضافة إلى تنبيهات جديدة للجزاء/cooldown здоровья доказательств SoraFS، أساسًا `torii_sorafs_proof_health_*`.

## استراتيجية النقل- استخدام OpenTelemetry من البداية إلى النهاية:
  - تحرر البوابات امتدادات OTLP (HTTP) مع معرفات الطلب التوضيحية، وملخصات البيان، وتجزئة الرمز المميز.
  - يستخدم الموزع `tracing` + `opentelemetry` للتصدير الذي يمتد لجلب الجلب.
  - نطاقات التصدير القوية SoraFS لـ PoR وعمليات التخزين. تعمل جميع المكونات على معرف التتبع الرئيسي، والذي يتم تحويله عبر `x-sorafs-trace`.
- `SorafsFetchOtel` يمكنه إدارة المقاييس في سجلات OTLP، و`telemetry::sorafs.fetch.*` من خلال حمولات JSON الخفيفة للواجهات الخلفية الموجهة للسجل.
- هواة الجمع: قم بربط جامعي OTEL مع Prometheus/Loki/Tempo (Tempo preдпочтелен). يتمتع المصدرون من Jaeger بخيارات اختيارية.
- يتم تنفيذ العمليات باستخدام الكاردينالنوستيا التالية (10% للرحلة الناجحة، 100% للمغادرة).

## تنسيق القياس عن بعد TLS (SF-5b)- مقياس التحقق:
  - تنشر أتمتة TLS `sorafs_gateway_tls_cert_expiry_seconds` و`sorafs_gateway_tls_renewal_total{result}` و`sorafs_gateway_tls_ech_enabled`.
  - قم بإدراج هذه المقاييس في نظرة عامة على البوابة بلوحة القيادة في لوحة TLS/الشهادات.
- جميع التنبيهات:
  - عند إنشاء تنبيهات TLS (≥ 14 يومًا متبقية)، تتوافق مع توفر SLO غير الموثوق به.
  - إلغاء حظر ECH يشمل التنبيهات الافتتاحية والملحقات وTLS ولوحات التوفر.
- خط الأنابيب: يتم تصدير وظيفة أتمتة TLS إلى مكدس Prometheus، وهي بوابة المقاييس؛ يضمن التنسيق مع SF-5b إزالة التكرار للأدوات.

## التحسين والطرق- استخدم المقياس التالي للبادئة الصحيحة `torii_sorafs_*` أو `sorafs_*`، باستخدام Torii والبوابة.
- طرق التقييس:
  - `result` → نتيجة HTTP (`success`، `refused`، `failed`).
  - `reason` → رمز الحذف/الرمز (`unsupported_chunker`، `timeout`، وما إلى ذلك).
  - `provider` → معرّف الهوية بالترميز السداسي.
  - `manifest` → بيان الملخص القانوني (obreзaется пи высокой cardinalьности).
  - `tier` → طبقة البيانات المعلنة (`hot`، `warm`، `archive`).
- أجهزة قياس الانبعاث عن بعد:
  - تعيش بوابة المقاييس تحت `torii_sorafs_*` وتستفيد من اتفاقيات النقل من `crates/iroha_core/src/telemetry.rs`.
  - أداة قياس المقاييس `sorafs_orchestrator_*` والتبعية `telemetry::sorafs.fetch.*` (دورة الحياة، إعادة المحاولة، فشل الموفر، الخطأ، المماطلة)، بيان الملخص التمهيدي، معرف الوظيفة، المنطقة، ورموز التعريف مقدم.
  - نشر `torii_sorafs_storage_*` و`torii_sorafs_capacity_*` و`torii_sorafs_por_*`.
- التنسيق مع إمكانية الملاحظة لتسجيل مقياس الكتالوج في مستند رئيسي لتسمية Prometheus، بما في ذلك المتطلبات cardиinalьности меток (مزود/بيانات verkhние границы).

## خط أنابيب البيانات- يقوم هواة الجمع بتنمية حياتهم باستخدام كل المكونات، وتصدير OTLP إلى Prometheus (المقاييس) وLoki/Tempo (السجل/المسار).
- يوفر eBPF (Tetragon) الاختياري اتصالاً غير آمن للبوابات/الدخول.
- استخدم `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` لـ Torii والاستخدامات الأخرى؛ يستمر الأوركسترا في تحديد `install_sorafs_fetch_otlp_exporter`.

## خطافات التحقق

- قم بتوصيل `scripts/telemetry/test_sorafs_fetch_alerts.sh` في CI لضبط التنبيهات Prometheus لإيقاف التزامن مع الأكشاك المترية والتحقق من القمع الخصوصية.
- اضغط على لوحات المفاتيح Grafana من إصدار التحكم (`dashboards/grafana/`) وقم بإلقاء نظرة على الشاشة/الخيارات من خلال لوحة التخصيص.
- تقوم تدريبات الفوضى بتسجيل النتائج من خلال `scripts/telemetry/log_sorafs_drill.sh`؛ يتم التحقق من الصحة باستخدام `scripts/telemetry/validate_drill_log.sh` (باسم [عملية دليل التشغيل](operations-playbook.md)).