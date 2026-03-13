---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de observabilidade
título: خطة قابلية الملاحظة وأهداف SLO لـ SoraFS
sidebar_label: Nomes de usuário e SLOs
description: مخطط التليمترية ولوحات المتابعة وسياسة ميزانية الخطأ لبوابات SoraFS والعقد والمُنسِّق متعدد المصادر.
---

:::note المصدر المعتمد
A solução de problemas é `docs/source/sorafs_observability_plan.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم ترحيل مجموعة Sphinx القديمة بالكامل.
:::

## الأهداف
- تحديد المقاييس والأحداث المُهيكلة للبوابات والعقد والمُنسِّق متعدد المصادر.
- Verifique o Grafana e verifique a configuração.
- تثبيت أهداف SLO جنبًا إلى جنب مع سياسات ميزانية الخطأ وتمارين الفوضى.

## كتالوج المقاييس

### أسطح البوابة

| المقياس | النوع | Etiquetas | الملاحظات |
|--------|-------|--------|-----------|
| `sorafs_gateway_active` | Medidor (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | `SorafsGatewayOtel`; Use o método HTTP para definir o endpoint/método. |
| `sorafs_gateway_responses_total` | Contador | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | كل طلب مكتمل للبوابة يزيد العداد مرة واحدة؛ `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histograma | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | كمون time-to-first-byte لاستجابات البوابة؛ يُصدَّر كـ Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Contador | `profile_version`, `result`, `error_code` | Verifique o valor do arquivo (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histograma | `profile_version`, `result`, `error_code` | توزيع كمون التحقق لإيصالات PoR. |
| `telemetry::sorafs.gateway.request` | حدث مُهيكل | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Você pode fazer isso com Loki/Tempo. |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Contador | Etiquetas Etiquetas Etiquetas | مقايس Prometheus محفوظة للّوحات التاريخية؛ تُصدر مع سلسلة OTLP الجديدة. |

تعكس أحداث `telemetry::sorafs.gateway.request` عدادات OTEL مع حمولة مُهيكلة, فتُظهر `endpoint` e `method` و`variant` و`status` و`error_code` و`duration_ms` Para Loki/Tempo, بينما تستهلك لوحات المتابعة Use OTLP para SLO.

### تليمترية صحة الأدلة| المقياس | النوع | Etiquetas | الملاحظات |
|--------|-------|--------|-----------|
| `torii_sorafs_proof_health_alerts_total` | Contador | `provider_id`, `trigger`, `penalty` | Você pode usar `RecordCapacityTelemetry` ou `SorafsProofHealthAlert`. O `trigger` é baseado em PDP/PoTR/Both, mas o `penalty` pode ser usado para obter mais informações Isso também reduz o tempo de espera. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Medidor | `provider_id` | Obtenha o PDP/PoTR para obter mais informações sobre o seu negócio Não há nada para fazer. |
| `torii_sorafs_proof_health_penalty_nano` | Medidor | `provider_id` | O recurso Nano-XOR está disponível no modo de espera (o tempo de espera é reduzido). |
| `torii_sorafs_proof_health_cooldown` | Medidor | `provider_id` | مقياس بولي (`1` = Tempo de resfriamento do tempo de espera) Obrigado. |
| `torii_sorafs_proof_health_window_end_epoch` | Medidor | `provider_id` | الحقبة المسجّلة لنافذة التليمترية المرتبطة بالتنبيه لتمكين المشغلين من الربط مع آثار Norito. |

تغذي هذه التدفقات الآن صف prova-saúde في لوحة Taikai viewer
(`dashboards/grafana/taikai_viewer.json`), مما يمنح مشغلي CDN رؤية فورية
لأحجام التنبيهات ومزيج محفزات PDP/PoTR والعقوبات وحالة cooldown لكل مزوّد.

تدعم المقاييس نفسها الآن قاعدتي تنبيه في Visualizador de Taikai:
`SorafsProofHealthPenalty` `SorafsProofHealthPenalty`
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` `torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}`
15 دقيقة, بينما يرفع `SorafsProofHealthCooldown` تحذيرًا إذا بقي مزوّد في
cooldown لمدة خمس دقائق. كلا التنبيهين موجودان في
`dashboards/alerts/taikai_viewer_rules.yml` é um SREs de alta qualidade que não funciona
تطبيق PoR/PoTR.

### أسطح المُنسِّق| المقياس / الحدث | النوع | Etiquetas | المُنتِج | الملاحظات |
|----------------|-------|--------|---------|-----------|
| `sorafs_orchestrator_active_fetches` | Medidor | `manifest_id`, `region` | `FetchMetricsCtx` | الجلسات الجارية حاليًا. |
| `sorafs_orchestrator_fetch_duration_ms` | Histograma | `manifest_id`, `region` | `FetchMetricsCtx` | هيستوغرام المدة بالميلي ثانية؛ Duração de 1 ms a 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Contador | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Nome: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Contador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Você pode usar o código de barras (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Contador | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Não use nenhum produto ou outro produto que não seja adequado para você. |
| `sorafs_orchestrator_chunk_latency_ms` | Histograma | `manifest_id`, `provider_id` | `FetchMetricsCtx` | A taxa de transferência/SLO é definida como (ms). |
| `sorafs_orchestrator_bytes_total` | Contador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | O nome do manifesto/provedor; A taxa de transferência é definida por `rate()` no PromQL. |
| `sorafs_orchestrator_stalls_total` | Contador | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Use o código `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | حدث مُهيكل | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | O valor do arquivo (بدء/اكتمال) é Norito JSON. |
| `telemetry::sorafs.fetch.retry` | حدث مُهيكل | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | يصدر لكل سلسلة إعادة محاولة لمزوّد؛ `attempts` é uma opção de configuração (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | حدث مُهيكل | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Não use nenhum produto para isso. |
| `telemetry::sorafs.fetch.error` | حدث مُهيكل | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Você pode usar o Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | حدث مُهيكل | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Você pode fazer isso sem parar. |

### أسطح العقد / التكرار| المقياس | النوع | Etiquetas | الملاحظات |
|--------|-------|--------|-----------|
| `sorafs_node_capacity_utilisation_pct` | Histograma | `provider_id` | O hotel OTEL está localizado no local (`_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Contador | `provider_id` | عداد أحادي لعينات PoR الناجحة, مشتق من لقطات المجدول. |
| `sorafs_node_por_failure_total` | Contador | `provider_id` | عداد أحادي لعينات PoR الفاشلة. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Medidor | `provider` | مقايس Prometheus حالية للبايتات المستخدمة وعمق الطابور وعدادات PoR الجارية. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Medidor | `provider` | Você pode usar/desativar o código de barras no final do processo. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Medidor | `provider`, `manifest` | Faça o download do seu cartão de crédito em `/v2/sorafs/por/ingestion/{manifest}` لتغذية لوحة/تنبيه "PoR Stalls". |

### Prova de recuperação oportuna (PoTR) e SLA الشرائح

| المقياس | النوع | Etiquetas | المُنتِج | الملاحظات |
|----|-------|--------|---------|-----------|
| `sorafs_potr_deadline_ms` | Histograma | `tier`, `provider` | منسق PoTR | هامش الموعد النهائي بالميلي ثانية (موجب = محقق). |
| `sorafs_potr_failures_total` | Contador | `tier`, `provider`, `reason` | منسق PoTR | Nome: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Contador | `provider`, `manifest_id`, `reason` | Marcenaria SLA | Não há nenhum problema com o SLO (que pode ser usado). |
| `sorafs_chunk_sla_violation_active` | Medidor | `provider`, `manifest_id` | Marcenaria SLA | مقياس بولي (0/1) يتبدل أثناء نافذة الإخفاق النشطة. |

## أهداف SLO

- Taxa de transferência: **99,9%** (HTTP 2xx/304).
- Trustless TTFB P95: camada quente ≤ 120 ms, camada quente ≤ 300 ms.
- Taxa de redução: ≥ 99,5% de desconto.
- نجاح المُنسِّق (اكتمال الشرائح): ≥ 99%.

## لوحات المتابعة والتنبيهات

1. **Observability للبوابة** (`dashboards/grafana/sorafs_gateway_observability.json`) — تتبع توفر trustless وTTFB P95 وتفصيل الرفض وإخفاقات PoR/PoTR عبر مقاييس OTEL.
2. **صحة المُنسِّق** (`dashboards/grafana/sorafs_fetch_observability.json`) — تغطي الحمل متعدد المصادر وإعادات المحاولة وإخفاقات barracas e barracas.
3. **مقاييس خصوصية SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — Transmita o relé de baldes e o coletor de baldes para `soranet_privacy_last_poll_unixtime` e`soranet_privacy_collector_enabled` e`soranet_privacy_poll_errors_total{provider}`.

Veja mais:

- `dashboards/alerts/sorafs_gateway_rules.yml` — O valor do arquivo e o TTFB estão disponíveis.
- `dashboards/alerts/sorafs_fetch_rules.yml` — إخفاقات/إعادات المحاولة/stalls للمُنسِّق؛ Você pode usar `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` e `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — é um coletor de dados e coletor de dados (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — Apagamento de energia para `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — إنذارات drift/ingest/CEK lag no visualizador Taikai إضافة إلى تنبيهات penalidade/cooldown لصحة الأدلة em SoraFS المبنية por `torii_sorafs_proof_health_*`.

## استراتيجية التتبع- Use OpenTelemetry para usar:
  - تصدر البوابات OTLP spans (HTTP) مع معرفات الطلب وdigests مانيفست وtoken hashes.
  - يستخدم المُنسِّق `tracing` + `opentelemetry` لتصدير spans لمحاولات الجلب.
  - تصدر عقد SoraFS المدمجة spans لتحديات PoR وعمليات التخزين. O ID de rastreamento é definido como `x-sorafs-trace`.
- يربط `SorafsFetchOtel` مقاييس المُنسِّق بهيستوغرامات OTLP بينما توفر أحداث `telemetry::sorafs.fetch.*` حمولات JSON Verifique se o produto está funcionando corretamente.
- Coletores: Coletores OTEL بجانب Prometheus/Loki/Tempo (Tempo مفضل). Eu sou o Jaeger اختيارية.
- يجب أخذ عينات للعمليات عالية الكاردينالية (10% para مسارات النجاح, 100% para للإخفاقات).

## تنسيق تليمترية TLS (SF-5b)

- مواءمة المقاييس:
  - Instale TLS `sorafs_gateway_tls_cert_expiry_seconds` e `sorafs_gateway_tls_renewal_total{result}` e `sorafs_gateway_tls_ech_enabled`.
  - أدرج هذه المقاييس في لوحة Gateway Overview ضمن لوحة TLS/Certificates.
- ربط التنبيهات:
  - عند إطلاق تنبيهات انتهاء TLS (≤ 14 يومًا متبقّيًا) اربطها مع SLO é confiável.
  - يؤدي تعطيل ECH إلى تنبيه ثانوي يشير إلى لوحات TLS e معًا.
- خط الأنابيب: تصدر مهمة أتمتة TLS إلى نفس مكدس Prometheus الذي يستخدمه gateway; Você pode usar o SF-5b para obter mais informações.

## اصطلاحات تسمية المقاييس والـLabels

- تتبع أسماء المقاييس بادئات `torii_sorafs_*` ou `sorafs_*` مستخدمة بواسطة Torii Então.
- مجموعات الـLabels موحدة:
  - `result` → Configuração HTTP (`success`, `refused`, `failed`).
  - `reason` → رمز الرفض/الخطأ (`unsupported_chunker`, `timeout`, إلخ).
  - `provider` → معرف المزوّد مرمّز بالهيكس.
  - `manifest` → digest مانيفست قانوني (يتم تقليمه عند ارتفاع الكاردينالية).
  - `tier` → Etiquetas de nomes (`hot`, `warm`, `archive`).
- نقاط إصدار التليمترية:
  - O código de saída é `torii_sorafs_*` e o `crates/iroha_core/src/telemetry.rs`.
  - يصدر المُنسِّق مقاييس `sorafs_orchestrator_*` e `telemetry::sorafs.fetch.*` (ciclo de vida, nova tentativa, falha do provedor, erro, parada) بعلامات resumo do código e ID do trabalho e região ومعرفات المزوّد.
  - Selecione `torii_sorafs_storage_*` e `torii_sorafs_capacity_*` e `torii_sorafs_por_*`.
- نسّق مع Observabilidade لتسجيل كتالوج المقاييس في وثيقة أسماء Prometheus المشتركة, بما في ذلك توقعات كاردينالية الـLabels (الحدود العليا للمزوّد/المانيفست).

## خط أنابيب البيانات

- Coletores de alta qualidade são usados ​​para coletar OTLP ou Prometheus (مقاييس) e Loki/Tempo (سجلات/تتبعات).
- يثري eBPF الاختياري (Tetragon) التتبع منخفض المستوى للبوابات/العقد.
- Altere `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` para Torii e execute o procedimento Use o código `install_sorafs_fetch_otlp_exporter`.

## خطافات التحقق

- شغّل `scripts/telemetry/test_sorafs_fetch_alerts.sh` أثناء CI لضمان بقاء قواعد تنبيه Prometheus متزامنة مع مقاييس stall وفحوصات كتم الخصوصية.
- Você pode usar o Grafana para obter mais informações (`dashboards/grafana/`). تغيير اللوحات.
- تسجل تمارين الفوضى النتائج عبر `scripts/telemetry/log_sorafs_drill.sh`؛ Use o código `scripts/telemetry/validate_drill_log.sh` (راجع [دليل العمليات](operations-playbook.md)).