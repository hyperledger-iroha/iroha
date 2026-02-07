---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id : plan d'observabilité
titre : خطة قابلية الملاحظة وأهداف SLO لـ SoraFS
sidebar_label : قابلية الملاحظة et SLO
description: مخطط التليمترية ولوحات المتابعة وسياسة ميزانية الخطأ لبوابات SoraFS والعقد والمُنسِّق متعدد المصادر.
---

:::note المصدر المعتمد
تعكس هذه الصفحة الخطة الموجودة ضمن `docs/source/sorafs_observability_plan.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم ترحيل مجموعة Sphinx القديمة بالكامل.
:::

## الأهداف
- تحديد المقاييس والأحداث المُهيكلة للبوابات والعقد والمُنسِّق متعدد المصادر.
- توفير لوحات Grafana وحدود التنبيه وخطافات التحقق.
- تثبيت أهداف SLO جنبًا إلى جنب مع سياسات ميزانية الخطأ وتمارين الفوضى.

## كتالوج المقاييس

### أسطح البوابة| المقياس | النوع | Étiquettes | الملاحظات |
|--------|-------|--------|---------------|
| `sorafs_gateway_active` | Jauge (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | يُصدر عبر `SorafsGatewayOtel`؛ Il s'agit d'un point de terminaison/méthode HTTP. |
| `sorafs_gateway_responses_total` | Compteur | | كل طلب مكتمل للبوابة يزيد العداد مرة واحدة؛ `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histogramme | | كمون time-to-first-byte لاستجابات البوابة؛ Il s'agit de Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Compteur | `profile_version`, `result`, `error_code` | نتائج تحقق الأدلة الملتقطة لحظة الطلب (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histogramme | `profile_version`, `result`, `error_code` | Il s'agit du PoR. |
| `telemetry::sorafs.gateway.request` | حدث مُهيكل | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Il s'agit d'une histoire de Loki/Tempo. || `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Compteur | مجموعات Étiquettes قديمة | مقاييس Prometheus محفوظة للّوحات التاريخية؛ تُصدر مع سلسلة OTLP الجديدة. |

Voir `telemetry::sorafs.gateway.request` pour OTEL avec le numéro `endpoint` et `method` و`variant` و`status` و`error_code` و`duration_ms` pour Loki/Tempo, pour que vous puissiez le faire OTLP est un SLO.

### تليمترية صحة الأدلة

| المقياس | النوع | Étiquettes | الملاحظات |
|--------|-------|--------|---------------|
| `torii_sorafs_proof_health_alerts_total` | Compteur | `provider_id`, `trigger`, `penalty` | Il s'agit de `RecordCapacityTelemetry` ou `SorafsProofHealthAlert`. يميز `trigger` pour PDP/PoTR/Both, pour `penalty` ما إذا كان قد خُصم فعليًا C'est également le temps de recharge. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Jauge | `provider_id` | أحدث أعداد PDP/PoTR داخل نافذة التليمترية المخالِفة حتى تتمكن الفرق من قياس مقدار تجاوز المزوّدين للسياسة. |
| `torii_sorafs_proof_health_penalty_nano` | Jauge | `provider_id` | Vous pouvez utiliser Nano-XOR pour utiliser le temps de recharge (vous devez utiliser le temps de recharge). |
| `torii_sorafs_proof_health_cooldown` | Jauge | `provider_id` | مقياس بولي (`1` = تنبيه مكبوت بسبب cooldown) لإظهار متى تكون تنبيهات المتابعة مكتومة مؤقتًا. |
| `torii_sorafs_proof_health_window_end_epoch` | Jauge | `provider_id` | La description de l'article correspond à la description de l'article Norito. |تغذي هذه التدفقات الآن صف proof-health في لوحة Taikai viewer
(`dashboards/grafana/taikai_viewer.json`), مما يمنح مشغلي CDN رؤية فورية
Vous avez également accès au temps de recharge PDP/PoTR et au temps de recharge.

تدعم المقاييس نفسها الآن قاعدتي تنبيه في Taikai viewer :
تطلق `SorafsProofHealthPenalty` عندما
يزداد `torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` خلال
15 jours pour `SorafsProofHealthCooldown` تحذيرًا إذا بقي مزوّد في
temps de recharge لمدة خمس دقائق. كلا التنبيهين موجودان في
`dashboards/alerts/taikai_viewer_rules.yml` pour les SRE pour votre projet
Utiliser PoR/PoTR.

### أسطح المُنسِّق| المقياس / الحدث | النوع | Étiquettes | المُنتِج | الملاحظات |
|----------------|-------|--------|---------|-----------|
| `sorafs_orchestrator_active_fetches` | Jauge | `manifest_id`, `region` | `FetchMetricsCtx` | الجلسات الجارية حاليًا. |
| `sorafs_orchestrator_fetch_duration_ms` | Histogramme | `manifest_id`, `region` | `FetchMetricsCtx` | هيستوغرام المدة بالميلي ثانية؛ Durée 1 ms à 30 s. |
| `sorafs_orchestrator_fetch_failures_total` | Compteur | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Noms : `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Compteur | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | يميز أسباب إعادة المحاولة (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Compteur | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | يلتقط تعطيل الجلسة أو تعداد الإخفاقات على مستوى الجلسة. |
| `sorafs_orchestrator_chunk_latency_ms` | Histogramme | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Il s'agit d'une valeur de débit/SLO (ms). |
| `sorafs_orchestrator_bytes_total` | Compteur | `manifest_id`, `provider_id` | `FetchMetricsCtx` | البايتات المسلّمة لكل manifeste/fournisseur؛ Le débit est défini par `rate()` pour PromQL. |
| `sorafs_orchestrator_stalls_total` | Compteur | `manifest_id`, `provider_id` | `FetchMetricsCtx` | يحصي الشرائح التي تتجاوز `ScoreboardConfig::latency_cap_ms`. || `telemetry::sorafs.fetch.lifecycle` | حدث مُهيكل | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Vous avez besoin d'un fichier Norito JSON. |
| `telemetry::sorafs.fetch.retry` | حدث مُهيكل | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | يصدر لكل سلسلة إعادة محاولة لمزوّد؛ `attempts` تحصي المحاولات التراكمية (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | حدث مُهيكل | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | يُظهر عند تجاوز المزوّد لعتبة الإخفاق. |
| `telemetry::sorafs.fetch.error` | حدث مُهيكل | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Il s'agit d'un personnage de Loki/Splunk. |
| `telemetry::sorafs.fetch.stall` | حدث مُهيكل | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | يُطلق عند تجاوز كمون الشريحة للحد المضبوط (يعكس عدادات stall). |

### أسطح العقد / التكرار| المقياس | النوع | Étiquettes | الملاحظات |
|--------|-------|--------|---------------|
| `sorafs_node_capacity_utilisation_pct` | Histogramme | `provider_id` | L'OTEL est situé dans le centre commercial (`_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Compteur | `provider_id` | عداد أحادي لعينات PoR الناجحة، مشتق من لقطات المجدول. |
| `sorafs_node_por_failure_total` | Compteur | `provider_id` | عداد أحادي لعينات PoR الفاشلة. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Jauge | `provider` | مقاييس Prometheus الحالية للبايتات المستخدمة وعمق الطابور وعدادات PoR الجارية. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Jauge | `provider` | بيانات نجاح السعة/الجاهزية للمزوّد المعروضة في لوحة السعة. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Jauge | `provider`, `manifest` | عمق التراكم بالإضافة إلى عدادات الفشل التراكمية المصدّرة عند الاستعلام عن `/v1/sorafs/por/ingestion/{manifest}` لتغذية Voir "PoR Stalls". |

### Preuve de récupération en temps opportun (PoTR) et SLA الشرائح| المقياس | النوع | Étiquettes | المُنتِج | الملاحظات |
|--------|-------|--------|---------|---------------|
| `sorafs_potr_deadline_ms` | Histogramme | `tier`, `provider` | Voir PoTR | هامش الموعد النهائي بالميلي ثانية (موجب = محقق). |
| `sorafs_potr_failures_total` | Compteur | `tier`, `provider`, `reason` | Voir PoTR | Titres : `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Compteur | `provider`, `manifest_id`, `reason` | مراقب SLA | يطلق عند إخفاق تسليم الشرائح في تحقيق SLO (كمون، معدل نجاح). |
| `sorafs_chunk_sla_violation_active` | Jauge | `provider`, `manifest_id` | مراقب SLA | مقياس بولي (0/1) يتبدل أثناء نافذة الإخفاق النشطة. |

## أهداف SLO

- Taux de conversion : **99,9 %** (HTTP 2xx/304).
- TTFB P95 sans confiance : niveau chaud ≤ 120 ms, niveau chaud ≤ 300 ms.
- معدل نجاح الأدلة: ≥ 99,5% يوميًا.
- نجاح المُنسِّق (اكتمال الشرائح) : ≥ 99 %.

## لوحات المتابعة والتنبيهات

1. **Observabilité للبوابة** (`dashboards/grafana/sorafs_gateway_observability.json`) — Il s'agit d'un système sans confiance et d'un TTFB P95 et d'un système PoR/PoTR pour un OTEL.
2. **صحة المُنسِّق** (`dashboards/grafana/sorafs_fetch_observability.json`) — تغطي الحمل متعدد المصادر وإعادات المحاولة وإخفاقات المزوّدين وموجات des stands.
3. **Modèle SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — Relais de seaux de relais et collecteur de collecteur `soranet_privacy_last_poll_unixtime` و`soranet_privacy_collector_enabled` و`soranet_privacy_poll_errors_total{provider}`.

حزم التنبيهات:- `dashboards/alerts/sorafs_gateway_rules.yml` — توفر البوابة وTTFB وارتفاعات فشل الأدلة.
- `dashboards/alerts/sorafs_fetch_rules.yml` — إخفاقات/إعادات المحاولة/stalls للمُنسِّق؛ يتم التحقق عبر `scripts/telemetry/test_sorafs_fetch_alerts.sh` et `dashboards/alerts/tests/sorafs_fetch_rules.test.yml` et `dashboards/alerts/tests/soranet_privacy_rules.test.yml` et `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — Collecteur de collecteurs de collecteurs de déchets (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — Réduction des baisses de tension en cas de coupure de courant par `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — Dérive/ingest/retard CEK pour Taikai Viewer et pénalité/rechargement pour SoraFS. `torii_sorafs_proof_health_*`.

## استراتيجية التتبع

- Utiliser OpenTelemetry pour plus de détails :
  - Les étendues OTLP (HTTP) et les hachages de jetons.
  - يستخدم المُنسِّق `tracing` + `opentelemetry` لتصدير s'étend sur لمحاولات الجلب.
  - Le module SoraFS s'étend sur les PoR et les réseaux sociaux. La recherche de l'ID de trace est effectuée par `x-sorafs-trace`.
- يربط `SorafsFetchOtel` مقاييس المُنسِّق OTLP بينما توفر أحداث `telemetry::sorafs.fetch.*` حمولات JSON خفيفة لخلفيات تركّز على السجلات.
- Collecteurs : شغّل Collecteurs OTEL بجانب Prometheus/Loki/Tempo (Tempo مفضل). Il s'agit de Jaeger اختيارية.
- يجب أخذ عينات للعمليات عالية الكاردينالية (10% لمسارات النجاح، 100% للإخفاقات).

## Application TLS (SF-5b)- مواءمة المقاييس :
  - Utilisez TLS `sorafs_gateway_tls_cert_expiry_seconds` et `sorafs_gateway_tls_renewal_total{result}` et `sorafs_gateway_tls_ech_enabled`.
  - Consultez la section Aperçu de la passerelle pour TLS/Certificats.
- ربط التنبيهات :
  - عند إطلاق تنبيهات انتهاء TLS (≤ 14 يومًا متبقّيًا) اربطها مع SLO توفر trustless.
  - يؤدي تعطيل ECH إلى تنبيه ثانوي يشير إلى لوحات TLS والتوفر معًا.
- Option : Utiliser TLS comme passerelle Prometheus. Il s'agit du SF-5b qui est en cours de réalisation.

## اصطلاحات تسمية المقاييس والـLabels- Utilisez les paramètres `torii_sorafs_*` et `sorafs_*` pour Torii.
- مجموعات الـLabels موحدة :
  - `result` → Paramètres HTTP (`success`, `refused`, `failed`).
  - `reason` → رمز الرفض/الخطأ (`unsupported_chunker`, `timeout`, ici).
  - `provider` → معرف المزوّد مرمّز بالهيكس.
  - `manifest` → digest مانيفست قانوني (يتم تقليمه عند ارتفاع الكاردينالية).
  - `tier` → Étiquettes de sécurité (`hot`, `warm`, `archive`).
- نقاط إصدار التليمترية:
  - مقاييس البوابة تعيش تحت `torii_sorafs_*` et استخدام اصطلاحات `crates/iroha_core/src/telemetry.rs`.
  - يصدر المُنسِّق مقاييس `sorafs_orchestrator_*` et `telemetry::sorafs.fetch.*` (cycle de vie, nouvelle tentative, échec du fournisseur, erreur, blocage) pour le résumé des informations, l'ID de travail et la région. المزوّد.
  - تعرض العقد `torii_sorafs_storage_*` et `torii_sorafs_capacity_*` et `torii_sorafs_por_*`.
- نسّق مع Observability لتسجيل كتالوج المقاييس في وثيقة أسماء Prometheus المشتركة، بما في ذلك توقعات كاردينالية الـLabels (الحدود العليا للمزوّد/المانيفست).

## خط أنابيب البيانات

- Les collectionneurs sont des collectionneurs d'OTLP et Prometheus (مقاييس) et Loki/Tempo (سجلات/تتبعات).
- يثري eBPF الاختياري (Tetragon) التتبع منخفض المستوى للبوابات/العقد.
- `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` pour Torii et Torii يستمر المُنسِّق في استدعاء `install_sorafs_fetch_otlp_exporter`.

## خطافات التحقق- شغّل `scripts/telemetry/test_sorafs_fetch_alerts.sh` أثناء CI لضمان بقاء قواعد تنبيه Prometheus متزامنة مع مقاييس stall وفحوصات كتم الخصوصية.
- حافظ على لوحات Grafana ضمن التحكم بالإصدارات (`dashboards/grafana/`) et حدّث اللقطات/الروابط عند تغيير اللوحات.
- تسجل تمارين الفوضى النتائج عبر `scripts/telemetry/log_sorafs_drill.sh`؛ يستخدم التحقق `scripts/telemetry/validate_drill_log.sh` (راجع [دليل العمليات](operations-playbook.md)).