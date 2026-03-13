---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/observability-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة المراقبة
العنوان: Plano de observabilidade e SLO da SoraFS
Sidebar_label: إمكانية المراقبة وSLOs
الوصف: مخطط القياس عن بعد ولوحات المعلومات وسياسة ميزانية الخطأ للبوابات SoraFS، ولدينا أو الأوركسترادور متعدد المصادر.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة مخصصة أو مخططة على `docs/source/sorafs_observability_plan.md`. Mantenha ambas as copias sincronzadas.
:::

##الأهداف
- تحديد المقاييس والأحداث التي تم تصميمها للبوابات، سواء كانت أو مديرًا متعدد المصادر.
- لوحات المعلومات Fornecer Grafana، حدود التنبيهات وخطافات التحقق.
- تحديد الأهداف SLO جنبًا إلى جنب مع سياسة ميزانية الخطأ وتدريبات الفوضى.

## كتالوج المقاييس

### السطوح تفعل البوابة| متريكا | تيبو | التسميات | نوتاس |
|---------|------|--------|-------|
| `sorafs_gateway_active` | مقياس (UpDownCounter) | `endpoint`، `method`، `variant`، `chunker`، `profile` | إميتيدو عبر `SorafsGatewayOtel`؛ تقوم بعمليات تشغيل HTTP من خلال الجمع بين نقطة النهاية/الطريقة. |
| `sorafs_gateway_responses_total` | عداد | `endpoint`، `method`، `variant`، `chunker`، `profile`، `result`، `status`، `error_code` | كل طلب كامل يتم زيادة البوابة مرة واحدة; `result` في {`success`،`error`،`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | الرسم البياني | `endpoint`، `method`، `variant`، `chunker`، `profile`، `result`، `status`، `error_code` | زمن الاستجابة من البايت الأول إلى البوابة؛ التصدير مثل Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | عداد | `profile_version`، `result`، `error_code` | تم التقاط نتائج التحقق من التجارب في لحظة الطلب (`result` في {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | الرسم البياني | `profile_version`، `result`، `error_code` | توزيع زمن الوصول للتحقق من استلاماتك. || `telemetry::sorafs.gateway.request` | حدث البناء | `endpoint`، `method`، `variant`، `result`، `status`، `error_code`، `duration_ms` | تم إنشاء السجل بعد الانتهاء من كل طلب للارتباط مع Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`، `torii_sorafs_gateway_refusals_total` | عداد | مجموعات التسميات البديلة | Metricas Prometheus mantidas للوحات المعلومات التاريخية؛ تم إصدارها مع سلسلة OTLP الجديدة. |

تقوم Eventos `telemetry::sorafs.gateway.request` باستعراض حسابات OTEL مع الحمولات النافعة، وتعرض `endpoint`، و`method`، و`variant`، و`status`، و`error_code` e `duration_ms` للارتباط مع Loki/Tempo، بينما تستهلك لوحات المعلومات سلسلة OTLP لمرافقة SLO.

### قياس المسافة عن بعد| متريكا | تيبو | التسميات | نوتاس |
|---------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | عداد | `provider_id`، `trigger`، `penalty` | قم بالزيادة حتى يتم إصدار `RecordCapacityTelemetry` إلى `SorafsProofHealthAlert`. `trigger` يميز بين PDP/PoTR/كلاهما، بينما يتم التقاط `penalty` في حالة قطع الضمانات أو حذفها من خلال فترة التهدئة. |
| `torii_sorafs_proof_health_pdp_failures`، `torii_sorafs_proof_health_potr_breaches` | مقياس | `provider_id` | أحدث عدوى من PDP/PoTR تتعلق بسجلات القياس عن بعد لكي تتمكن المعدات من القياس الكمي أو بقدر ما يتفوق المحللون على السياسة. |
| `torii_sorafs_proof_health_penalty_nano` | مقياس | `provider_id` | تم حذف Valor Nano-XOR من التنبيه الأخير (بدون أي وقت أو فترة تهدئة إضافية للتطبيق). |
| `torii_sorafs_proof_health_cooldown` | مقياس | `provider_id` | مقياس منطقي (`1` = تنبيه متجاوز للتبريد) لعرض تنبيهات المرافقة عندما تكون صامتة مؤقتًا. |
| `torii_sorafs_proof_health_window_end_epoch` | مقياس | `provider_id` | تم تسجيل العصر لتاريخ القياس عن بعد مع تنبيه المشغلين المرتبطين بالمنتجات Norito. |

يغذي Esses alimentam a linha de saude das provas do لوحة القيادة Taikai viewer
(`dashboards/grafana/taikai_viewer.json`)، ومن خلال مشغلي CDN الذين يمكن رؤيتهم بوتيرة حقيقية
حجم التنبيهات ومزيج المشغلات PDP/PoTR والعقوبات وحالة التهدئة
بروفيدور.من خلال قياسات متواصلة، يتم دعم إرشادات تنبيه عارض Taikai:
`SorafsProofHealthPenalty` يختلف عندما
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` التكبير
لدينا آخر 15 دقيقة، أثناء `SorafsProofHealthCooldown` أرسل لنا نصيحة
يثبت الثبات في فترة التهدئة لمدة خمس دقائق. المزيد من التنبيهات الحية
`dashboards/alerts/taikai_viewer_rules.yml` لكي تتمكن SREs من الحصول على السياق فورًا
عند تكثيف تطبيق PoR/PoTR.

### السطوح تفعل orquestrador| متريكا / إيفنتو | تيبو | التسميات | منتج | نوتاس |
|------------------|------|--------|----------|-------|
| `sorafs_orchestrator_active_fetches` | مقياس | `manifest_id`، `region` | `FetchMetricsCtx` | الجلسات مستمرة. |
| `sorafs_orchestrator_fetch_duration_ms` | الرسم البياني | `manifest_id`، `region` | `FetchMetricsCtx` | رسم بياني للمدة على بعد أميال؛ دلاء دي 1 مللي ثانية في 30 ثانية. |
| `sorafs_orchestrator_fetch_failures_total` | عداد | `manifest_id`، `region`، `reason` | `FetchMetricsCtx` | رازوز: `no_providers`، `no_healthy_providers`، `no_compatible_providers`، `exhausted_retries`، `observer_failed`، `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | عداد | `manifest_id`، `provider_id`، `reason` | `FetchMetricsCtx` | التمييز بين أسباب إعادة المحاولة (`retry`، `digest_mismatch`، `length_mismatch`، `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | عداد | `manifest_id`، `provider_id`، `reason` | `FetchMetricsCtx` | قم بالتقاط عدوى الإعاقة والعدوى على مستوى الجلسة. |
| `sorafs_orchestrator_chunk_latency_ms` | الرسم البياني | `manifest_id`، `provider_id` | `FetchMetricsCtx` | توزيع زمن الوصول للقطعة (مللي ثانية) لتحليل الإنتاجية/SLO. |
| `sorafs_orchestrator_bytes_total` | عداد | `manifest_id`، `provider_id` | `FetchMetricsCtx` | وحدات البايت تدخل من خلال البيان/المزود؛ اشتقاق الإنتاجية عبر `rate()` em PromQL. |
| `sorafs_orchestrator_stalls_total` | عداد | `manifest_id`، `provider_id` | `FetchMetricsCtx` | القطع التي تتجاوز `ScoreboardConfig::latency_cap_ms`. || `telemetry::sorafs.fetch.lifecycle` | حدث البناء | `manifest`، `region`، `job_id`، `event`، `status`، `chunk_count`، `total_bytes`، `provider_candidates`، `retry_budget`، `global_parallel_limit` | `FetchTelemetryCtx` | قم بتوضيح دورة الحياة (البدء/الإكمال) مع الحمولة JSON Norito. |
| `telemetry::sorafs.fetch.retry` | حدث البناء | `manifest`، `region`، `job_id`، `provider`، `reason`، `attempts` | `FetchTelemetryCtx` | تم إصداره من خلال سلسلة من المحاولات من قبل المثبت؛ `attempts` تحتوي على عمليات إعادة المحاولة المتزايدة (>= 1). |
| `telemetry::sorafs.fetch.provider_failure` | حدث البناء | `manifest`، `region`، `job_id`، `provider`، `reason`، `failures` | `FetchTelemetryCtx` | تم النشر عندما يتم إثبات الرحلة أو الحد من الخطأ. |
| `telemetry::sorafs.fetch.error` | حدث البناء | `manifest`، `region`، `job_id`، `reason`، `provider?`، `provider_reason?`، `duration_ms` | `FetchTelemetryCtx` | سجل المحطة الطرفية، صديق لاستيعاب Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | حدث البناء | `manifest`، `region`، `job_id`، `provider`، `latency_ms`، `bytes` | `FetchTelemetryCtx` | يتم ذلك عند تأخير القطع بشكل فائق أو تكوين محدود (تستخدم قواطع التوقف). |

### سطحيات دي لا/مكررة| متريكا | تيبو | التسميات | نوتاس |
|---------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | الرسم البياني | `provider_id` | رسم بياني لنسبة استخدام التخزين OTEL (تم تصديره كـ `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | عداد | `provider_id` | Contador monotono de amostras PoR bem-sucedidas, استخلاص اللقطات من الجدولة. |
| `sorafs_node_por_failure_total` | عداد | `provider_id` | Contador monotono de amostras PoR com falha. |
| `torii_sorafs_storage_bytes_*`، `torii_sorafs_storage_por_*` | مقياس | `provider` | توجد مقاييس Prometheus للبايتات المستخدمة وعمق الملف وعدوى PoR في Voo. |
| `torii_sorafs_capacity_*`، `torii_sorafs_uptime_bps`، `torii_sorafs_por_bps` | مقياس | `provider` | توفر بيانات السعة/وقت التشغيل نجاحًا كبيرًا في لوحة معلومات السعة. |
| `torii_sorafs_por_ingest_backlog`، `torii_sorafs_por_ingest_failures_total` | مقياس | `provider`، `manifest` | عمق تراكم المزيد من الحاويات المصدرة باستمرار إلى `/v2/sorafs/por/ingestion/{manifest}` واستشارة أو توصيل أو طلاء/تنبيه "PoR Stalls". |

### إثبات الاسترجاع في الوقت المناسب (PoTR) وSLA للقطع| متريكا | تيبو | التسميات | منتج | نوتاس |
|---------|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | الرسم البياني | `tier`، `provider` | المنسق بووتر | اتبع الموعد النهائي في milissegundos (positivo = atendido). |
| `sorafs_potr_failures_total` | عداد | `tier`، `provider`، `reason` | المنسق بووتر | رازوز: `expired`، `missing_proof`، `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | عداد | `provider`، `manifest_id`، `reason` | مونيتور دي SLA | Disparado quando to entrega de القطع falha no SLO (latentence، فئة النجاح). |
| `sorafs_chunk_sla_violation_active` | مقياس | `provider`، `manifest_id` | مونيتور دي SLA | مقياس منطقي (0/1) بالتناوب أثناء عزف الكمان الجديد. |

## أوبجيتيفوس سلو

- بوابة الوصول غير الموثوق بها: **99.9%** (الاستجابة HTTP 2xx/304).
- TTFB P95 Trustless: الطبقة الساخنة = 99.5% يوميًا.
- نجاح الأوركسترادور (نتيجة القطع): >= 99%.

## لوحات المعلومات والتنبيهات1. **إمكانية مراقبة البوابة** (`dashboards/grafana/sorafs_gateway_observability.json`) - مرافقة لتوفر الثقة، TTFB P95، تفاصيل عمليات إعادة الاستخدام وتصحيح PoR/PoTR عبر مقاييس OTEL.
2. **Saude do orquestrador** (`dashboards/grafana/sorafs_fetch_observability.json`) - شحن كوبري متعدد المصادر، وإعادة المحاولة، وفشل الإجراءات، وإيقاف الأكشاك.
3. **مقاييس الخصوصية SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) - دلاء رسومات الترحيل المجهولة، وتعبئة القمع، وجمع المجمع عبر `soranet_privacy_last_poll_unixtime`، و`soranet_privacy_collector_enabled`، و`soranet_privacy_poll_errors_total{provider}`.

حزم التنبيهات:

- `dashboards/alerts/sorafs_gateway_rules.yml` - إمكانية الوصول إلى البوابة، TTFB، صور الاختبار.
- `dashboards/alerts/sorafs_fetch_rules.yml` - falhas/إعادة المحاولة/الأكشاك التي تقوم بها orquestrador؛ التحقق من الصحة عبر `scripts/telemetry/test_sorafs_fetch_alerts.sh` و`dashboards/alerts/tests/sorafs_fetch_rules.test.yml` و`dashboards/alerts/tests/soranet_privacy_rules.test.yml` و`dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` - صور انخفاض الخصوصية، وإنذارات القمع، واكتشاف أداة التجميع الإضافية، وتنبيهات أداة التجميع المعطلة (`soranet_privacy_last_poll_unixtime`، `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` - إنذارات انقطاع التيار الكهربائي المجهول المرتبطة بـ `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` - تنبيهات الانجراف/الدخول/تأخر CEK لعارض Taikai بالإضافة إلى تنبيهات جديدة للعقوبة/التهدئة من اختبار SoraFS المعزز لـ `torii_sorafs_proof_health_*`.

## استراتيجية التتبع- Adote OpenTelemetry de ponta a ponta:
  - يمتد إصدار البوابات إلى OTLP (HTTP) الموضحة بمعرفات الطلب وملخصات البيان وتجزئة الرمز المميز.
  - Orquestrador USA `tracing` + `opentelemetry` لتصدير نطاقات تجارب الجلب.
  - الرقم SoraFS يمتد للتصدير لتحديات عمليات التخزين وعمليات التخزين. قم بمشاركة جميع المكونات ونشر معرف التتبع عبر `x-sorafs-trace`.
- `SorafsFetchOtel` مقاييس الارتباط للمنسق إلى المدرج التكراري OTLP أثناء الأحداث `telemetry::sorafs.fetch.*` التي توفر حمولات JSON للواجهات الخلفية المركزية في السجلات.
- جامعي: تنفيذ جامعي OTEL ao lado de Prometheus/Loki/Tempo (Tempo Favoritedo). المصدرون متوافقون مع Jaeger الخيارات الدائمة.
- العمليات الكاردينالية العالية يجب أن تكون فعالة (10% لنجاح النجاح، 100% للنجاح).

## تنسيق القياس عن بعد TLS (SF-5b)- تحسين المقاييس:
  - جهاز TLS تلقائي يعمل على `sorafs_gateway_tls_cert_expiry_seconds` و`sorafs_gateway_tls_renewal_total{result}` و`sorafs_gateway_tls_ech_enabled`.
  - تضمين مقاييس لا تحتوي على نظرة عامة على بوابة لوحة القيادة أو طلاء TLS/الشهادات.
- تنبيهات التنبيه:
  - عندما تختفي تنبيهات انتهاء صلاحية TLS (<= 14 يومًا متبقية)، ترتبط بـ SLO للتوفر غير الموثوق به.
  - يصدر إلغاء تنشيط ECH تنبيهًا ثانويًا يشير إلى مقدار TLS المؤلم بقدر ما هو متاح.
- خط الأنابيب: مهمة تصدير TLS التلقائي إلى مكدس Prometheus من خلال مقاييس البوابة؛ تم التنسيق مع SF-5b لضمان إزالة البيانات المكررة.

## مؤتمرات الأسماء وتسميات المقاييس- أسماء المقاييس تتبع البادئات الموجودة `torii_sorafs_*` أو `sorafs_*` المستخدمة بواسطة Torii أو البوابة.
- Conjuntos de labels sao Padronizados:
  - `result` -> نتيجة HTTP (`success`، `refused`، `failed`).
  - `reason` -> كود الاسترجاع/الخطأ (`unsupported_chunker`، `timeout`، وما إلى ذلك).
  - `provider` -> معرف المثبت المشفر بالست عشري.
  - `manifest` -> ملخص الكنسي للقيام بالبيان (القصاص عندما يكون كارديناليدادي والأعلى).
  - `tier` -> التسميات المعلنة للطبقة (`hot`، `warm`، `archive`).
- نقاط إرسال القياس عن بعد:
  - تعمل مقاييس البوابة على `torii_sorafs_*` وإعادة استخدام اجتماعات `crates/iroha_core/src/telemetry.rs`.
  - يقوم المُنسق بإصدار المقاييس `sorafs_orchestrator_*` والأحداث `telemetry::sorafs.fetch.*` (دورة الحياة، إعادة المحاولة، فشل الموفر، الخطأ، التوقف) مع ملخص البيان، معرف الوظيفة، السجل ومعرفات المُحقق.
  - رقم المعرض `torii_sorafs_storage_*`، `torii_sorafs_capacity_*` و`torii_sorafs_por_*`.
- التنسيق مع إمكانية الملاحظة لتسجيل كتالوج المقاييس في المستند الذي تمت مشاركته بالأسماء Prometheus، بما في ذلك التوقعات الأساسية للتسميات (الحدود العليا للإثبات/البيانات).

## خط أنابيب دي دادوس- يتم زرع المجمعات جنبًا إلى جنب مع كل مكون، وتصدير OTLP إلى Prometheus (المقاييس) وLoki/Tempo (السجلات/الآثار).
- يعمل eBPF الاختياري (Tetragon) على زيادة التتبع على مستوى منخفض للبوابات/الأرقام.
- استخدم `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` لـ Torii ورموزنا؛ o orquestrador continua chamando `install_sorafs_fetch_otlp_exporter`.

## خطافات التحقق

- قم بتنفيذ `scripts/telemetry/test_sorafs_fetch_alerts.sh` أثناء CI لضمان إعادة ضبط التنبيه Prometheus بشكل دائم بالتزامن مع مقاييس التوقف وعمليات التحقق من الخصوصية.
- صيانة لوحات المعلومات Grafana للتحكم بالعكس (`dashboards/grafana/`) وتفعيل لقطات الشاشة/الروابط عندما يتم تغييرها.
- تدريبات المساعدة على تسجيل النتائج عبر `scripts/telemetry/log_sorafs_drill.sh`؛ التحقق من صحة الولايات المتحدة الأمريكية `scripts/telemetry/validate_drill_log.sh` (راجع [دليل التشغيل](operations-playbook.md)).