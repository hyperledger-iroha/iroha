---
id: observability-plan
lang: az
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Observability & SLO Plan
sidebar_label: Observability & SLOs
description: Telemetry schema, dashboards, and error-budget policy for SoraFS gateways, nodes, and the multi-source orchestrator.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
:::

## Məqsədlər
- Şlüzlər, qovşaqlar və çoxmənbəli orkestr üçün ölçüləri və strukturlaşdırılmış hadisələri müəyyənləşdirin.
- Grafana tablosunu, xəbərdarlıq hədlərini və doğrulama qarmaqlarını təmin edin.
- Səhv-büdcə və xaos-qazma siyasətləri ilə yanaşı SLO hədəflərini təyin edin.

## Metrik Kataloq

### Gateway səthləri

| Metrik | Növ | Etiketlər | Qeydlər |
|--------|------|--------|-------|
| `sorafs_gateway_active` | Ölçer (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | `SorafsGatewayOtel` vasitəsilə yayılır; son nöqtə/metod kombinasiyası üzrə uçuşda HTTP əməliyyatlarını izləyir. |
| `sorafs_gateway_responses_total` | Sayğac | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, I18NI000000040X, I18NI00001041002 | Hər tamamlanmış şlüz sorğusu bir dəfə artır; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Histoqram | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, I18NI000000053X, I18NI00001054005 | Şlüz cavabları üçün birinci bayt gecikmə vaxtı; Prometheus `_bucket/_sum/_count` kimi ixrac edilir. |
| `sorafs_gateway_proof_verifications_total` | Sayğac | `profile_version`, `result`, `error_code` | Sorğu zamanı əldə edilən sübut yoxlama nəticələri (`result` ∈ {`success`, `failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Histoqram | `profile_version`, `result`, `error_code` | PoR qəbzləri üçün doğrulama gecikmə paylanması. |
| `telemetry::sorafs.gateway.request` | Strukturlaşdırılmış hadisə | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Loki/Tempo korrelyasiyası üçün hər sorğunun tamamlanmasında yayımlanan strukturlaşdırılmış jurnal. |

`telemetry::sorafs.gateway.request` hadisələri, `endpoint`, `method`, `variant`, `status`, `status`, I18NI000000000 və I18NI00000000000001080000000000000000002 və `endpoint` səthli strukturlaşdırılmış faydalı yüklərlə OTEL sayğaclarını əks etdirir. Panellər SLO izləmə üçün OTLP seriyasını istehlak edərkən Loki/Tempo korrelyasiyası.

### Sağlamlığı sübut edən telemetriya

| Metrik | Növ | Etiketlər | Qeydlər |
|--------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Sayğac | `provider_id`, `trigger`, `penalty` | `RecordCapacityTelemetry` hər dəfə `SorafsProofHealthAlert` yaydıqda artır. `trigger` PDP/PoTR/Hər iki uğursuzluğu fərqləndirir, `penalty` isə girovun həqiqətən kəsildiyini və ya soyudulması ilə sıxışdırıldığını göstərir. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Ölçü | `provider_id` | Qüsurlu telemetriya pəncərəsində bildirilən son PDP/PoTR sayları, beləliklə, komandalar provayderlərin siyasəti nə qədər aşdığını ölçə bilsinlər. |
| `torii_sorafs_proof_health_penalty_nano` | Ölçü | `provider_id` | Nano-XOR məbləği son xəbərdarlıqda azaldıldı (soyutma müddəti tətbiq edilmədikdə sıfır). |
| `torii_sorafs_proof_health_cooldown` | Ölçü | `provider_id` | Məntiqi ölçü (`1` = siqnal soyutma ilə dayandırılır) təqib siqnalları müvəqqəti olaraq səssizləşdirildikdə səthə çıxır. |
| `torii_sorafs_proof_health_window_end_epoch` | Ölçü | `provider_id` | Operatorların Norito artefaktları ilə əlaqə qura bilməsi üçün xəbərdarlığa bağlı telemetriya pəncərəsi üçün qeydə alınmış dövr. |

Bu lentlər indi Taikai izləyicisinin tablosunun sağlamlıq vəziyyətini yoxlamaq sırasını gücləndirir
(`dashboards/grafana/taikai_viewer.json`), CDN operatorlarına canlı görünürlük verir
xəbərdarlıq həcmlərinə, PDP/PoTR trigger qarışığına, cəzalara və soyutma vəziyyətinə
provayder.

Eyni ölçülər indi iki Taikai izləyicisi xəbərdarlığı qaydasını dəstəkləyir:
`SorafsProofHealthPenalty` hər an yanır
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}`-də artır
son 15 dəqiqə, `SorafsProofHealthCooldown` isə xəbərdarlıq edərsə
provayder beş dəqiqə soyutma rejimində qalır. Hər iki xəbərdarlıq yaşayır
`dashboards/alerts/taikai_viewer_rules.yml` beləliklə, SRE-lər dərhal kontekst əldə edirlər
PoR/PoTR tətbiqi gücləndikdə.

### Orkestr səthləri

| Metrik / Hadisə | Növ | Etiketlər | İstehsalçı | Qeydlər |
|----------------|------|--------|----------|-------|
| `sorafs_orchestrator_active_fetches` | Ölçü | `manifest_id`, `region` | `FetchMetricsCtx` | Hazırda uçuşda olan seanslar. |
| `sorafs_orchestrator_fetch_duration_ms` | Histoqram | `manifest_id`, `region` | `FetchMetricsCtx` | Millisaniyələrdə müddət histoqramı; 1ms→30s vedrələr. |
| `sorafs_orchestrator_fetch_failures_total` | Sayğac | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Səbəblər: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Sayğac | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Yenidən cəhd səbəblərini fərqləndirir (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Sayğac | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Sessiya səviyyəsində əlillik/uğursuzluq göstəricilərini çəkir. |
| `sorafs_orchestrator_chunk_latency_ms` | Histoqram | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Keçirmə qabiliyyəti/SLO təhlili üçün hər bir hissənin alınması gecikmə paylanması (ms). |
| `sorafs_orchestrator_bytes_total` | Sayğac | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Hər manifest/provayderə çatdırılan baytlar; PromQL-də `rate()` vasitəsilə ötürmə qabiliyyəti əldə edin. |
| `sorafs_orchestrator_stalls_total` | Sayğac | `manifest_id`, `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms`-dən çox olan parçaları sayır. |
| `telemetry::sorafs.fetch.lifecycle` | Strukturlaşdırılmış hadisə | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, I18NI0000016010, I18NI0000016010 `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Norito JSON yükü ilə işin həyat dövrünü (başlanğıc/tamamla) güzgülər. |
| `telemetry::sorafs.fetch.retry` | Strukturlaşdırılmış hadisə | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Provayderin təkrar cəhd seriyası üzrə yayılır; `attempts` artımlı təkrar cəhdləri hesablayır (≥1). |
| `telemetry::sorafs.fetch.provider_failure` | Strukturlaşdırılmış hadisə | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Provayder uğursuzluq həddini keçdikdə görünür. |
| `telemetry::sorafs.fetch.error` | Strukturlaşdırılmış hadisə | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Terminal nasazlığı rekordu, Loki/Splunk qəbuluna uyğundur. |
| `telemetry::sorafs.fetch.stall` | Strukturlaşdırılmış hadisə | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Parça gecikməsi konfiqurasiya edilmiş qapağı pozduqda yüksəlir (güzgülər sayğacların dayanması). |

### Node/replikasiya səthləri

| Metrik | Növ | Etiketlər | Qeydlər |
|--------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Histoqram | `provider_id` | Yaddaşdan istifadə faizinin OTEL histoqramı (`_bucket/_sum/_count` olaraq ixrac edilmişdir). |
| `sorafs_node_por_success_total` | Sayğac | `provider_id` | Planlayıcı anlıq görüntülərindən əldə edilən uğurlu PoR nümunələri üçün monotonik sayğac. |
| `sorafs_node_por_failure_total` | Sayğac | `provider_id` | Uğursuz PoR nümunələri üçün monotonik sayğac. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Ölçü | `provider` | Mövcud Prometheus istifadə olunan baytlar, növbə dərinliyi, PoR uçuş sayları üçün ölçülər. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Ölçü | `provider` | Provayderin tutumu/iş vaxtı uğuru məlumatları tutum panelində üzə çıxdı. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Ölçü | `provider`, `manifest` | `/v2/sorafs/por/ingestion/{manifest}` sorğusu keçirildikdə, “PoR Stalls” panelini/xəbərdarlığını qidalandıraraq, ehtiyat işlərinin dərinliyi üstəgəl ixrac edilən məcmu uğursuzluq sayğacları. |

### Təmir və SLA

| Metrik | Növ | Etiketlər | Qeydlər |
|--------|------|--------|-------|
| `sorafs_repair_tasks_total` | Sayğac | `status` | Təmir tapşırığı keçidləri üçün OTEL sayğacı. |
| `sorafs_repair_latency_minutes` | Histoqram | `outcome` | Təmir dövrünün gecikməsi üçün OTEL histoqramı. |
| `sorafs_repair_queue_depth` | Histoqram | `provider` | Hər provayder üçün növbəyə qoyulmuş tapşırıqların OTEL histoqramı (snapshot tərzi). |
| `sorafs_repair_backlog_oldest_age_seconds` | Histoqram | — | Ən köhnə növbəli tapşırıq yaşının OTEL histoqramı (saniyələr). |
| `sorafs_repair_lease_expired_total` | Sayğac | `outcome` | İcarə müddətinin bitməsi üçün OTEL sayğacı (`requeued`/`escalated`). |
| `sorafs_repair_slash_proposals_total` | Sayğac | `outcome` | Slash təklif keçidləri üçün OTEL sayğacı. |
| `torii_sorafs_repair_tasks_total` | Sayğac | `status` | Tapşırıq keçidləri üçün Prometheus sayğacı. |
| `torii_sorafs_repair_latency_minutes_bucket` | Histoqram | `outcome` | Təmir ömrünün gecikməsi üçün Prometheus histoqramı. |
| `torii_sorafs_repair_queue_depth` | Ölçü | `provider` | Hər provayder üçün növbəli tapşırıqlar üçün Prometheus göstəricisi. |
| `torii_sorafs_repair_backlog_oldest_age_seconds` | Ölçü | — | Prometheus ən köhnə növbəli tapşırıq yaşı (saniyələr) üçün ölçü. |
| `torii_sorafs_repair_lease_expired_total` | Sayğac | `outcome` | İcarə müddətinin bitməsi üçün Prometheus sayğacı. |
| `torii_sorafs_slash_proposals_total` | Sayğac | `outcome` | Slash təklif keçidləri üçün Prometheus sayğacı. |

İdarəetmə auditi JSON metaməlumatları təmir telemetriya etiketlərini əks etdirir (`status`, `ticket_id`, `manifest`, təmir hadisələri üzrə `provider`; `outcome` slash-professional ölçülər üzrə audit və yoxlamalar) deterministik olaraq.

### Saxlama və GC| Metrik | Növ | Etiketlər | Qeydlər |
|--------|------|--------|-------|
| `sorafs_gc_runs_total` | Sayğac | `result` | Daxili qovşaq tərəfindən yayılan GC süpürgələri üçün OTEL sayğacı. |
| `sorafs_gc_evictions_total` | Sayğac | `reason` | Səbəb üzrə qruplaşdırılmış çıxarılan manifestlər üçün OTEL sayğacı. |
| `sorafs_gc_bytes_freed_total` | Sayğac | `reason` | Səbəb üzrə qruplaşdırılmış sərbəst buraxılmış baytlar üçün OTEL sayğacı. |
| `sorafs_gc_blocked_total` | Sayğac | `reason` | Aktiv təmir və ya siyasətlə bloklanmış evakuasiyalar üçün OTEL sayğacı. |
| `torii_sorafs_gc_runs_total` | Sayğac | `result` | GC taramaları üçün Prometheus sayğacı (uğur/səhv). |
| `torii_sorafs_gc_evictions_total` | Sayğac | `reason` | Prometheus səbəbə görə qruplaşdırılmış çıxarılan manifestlər üçün sayğac. |
| `torii_sorafs_gc_bytes_freed_total` | Sayğac | `reason` | Prometheus səbəbə görə qruplaşdırılmış baytlar üçün sayğac. |
| `torii_sorafs_gc_blocked_total` | Sayğac | `reason` | Prometheus səbəbə görə qruplaşdırılmış bloklanmış çıxarılmalar üçün sayğac. |
| `torii_sorafs_gc_expired_manifests` | Ölçü | — | GC taramaları tərəfindən müşahidə edilən vaxtı keçmiş manifestlərin cari sayı. |
| `torii_sorafs_gc_oldest_expired_age_seconds` | Ölçü | — | Ən köhnə müddəti bitmiş manifestin saniyələri ilə yaş (tutma lütfündən sonra). |

### Barışıq

| Metrik | Növ | Etiketlər | Qeydlər |
|--------|------|--------|-------|
| `sorafs.reconciliation.runs_total` | Sayğac | `result` | Razılaşma snapshotları üçün OTEL sayğacı. |
| `sorafs.reconciliation.divergence_total` | Sayğac | — | OTEL hər qaçışda fərq sayma sayğacı. |
| `torii_sorafs_reconciliation_runs_total` | Sayğac | `result` | Üzləşdirmə əməliyyatları üçün Prometheus sayğacı. |
| `torii_sorafs_reconciliation_divergence_count` | Ölçü | — | Üzləşdirmə hesabatında müşahidə edilən son fərqlərin sayı. |

### Vaxtında Axtarış (PoTR) və yığın SLA sübutu

| Metrik | Növ | Etiketlər | İstehsalçı | Qeydlər |
|--------|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | Histoqram | `tier`, `provider` | PoTR koordinatoru | Son tarix millisaniyələrdə boşaldı (müsbət = qarşılandı). |
| `sorafs_potr_failures_total` | Sayğac | `tier`, `provider`, `reason` | PoTR koordinatoru | Səbəblər: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Sayğac | `provider`, `manifest_id`, `reason` | SLA monitor | Parça çatdırılması SLO (gecikmə, müvəffəqiyyət dərəcəsi) əldən verdikdə işə salınır. |
| `sorafs_chunk_sla_violation_active` | Ölçü | `provider`, `manifest_id` | SLA monitor | Boolean ölçü (0/1) aktiv pozuntu pəncərəsi zamanı dəyişdirildi. |

## SLO Hədəfləri

- Gateway etibarsız mövcudluğu: **99.9%** (HTTP 2xx/304 cavab).
- Etibarsız TTFB P95: isti səviyyə ≤120ms, isti səviyyə ≤300ms.
- Proof müvəffəqiyyət dərəcəsi: gündə ≥99.5%.
- Orkestrin uğuru (parçaların tamamlanması): ≥99%.

## İdarə Paneli və Xəbərdarlıq

1. **Gateway Observability** (`dashboards/grafana/sorafs_gateway_observability.json`) — OTEL göstəriciləri vasitəsilə etibarsız əlçatanlığı, TTFB P95, imtinanın pozulmasını və PoR/PoTR xətalarını izləyir.
2. **Orchestrator Sağlamlığı** (`dashboards/grafana/sorafs_fetch_observability.json`) — çoxmənbəli yükü, təkrar cəhdləri, provayder nasazlıqlarını və dayanma partlamalarını əhatə edir.
3. **SoraNet Məxfilik Metrikləri** (`dashboards/grafana/soranet_privacy_metrics.json`) — `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` və I18NI0000002 vasitəsilə anonimləşdirilmiş rele vedrələri, bastırma pəncərələri və kollektor sağlamlığını qrafiklər.
4. **Capacity Health** (`dashboards/grafana/sorafs_capacity_health.json`) — provayderin boşluq sahəsi və təmir SLA yüksəlişlərini, provayder tərəfindən təmir növbəsinin dərinliyini və GC süpürmələri/çıxışları/bloklanmış baytları/müddəti bitmiş açıq-aşkar yaş və uzlaşma divergensiyasını izləyir.

Xəbərdarlıq paketləri:

- `dashboards/alerts/sorafs_gateway_rules.yml` — şlüz mövcudluğu, TTFB, uğursuzluq sıçrayışlarını sübut edir.
- `dashboards/alerts/sorafs_fetch_rules.yml` — orkestrator uğursuzluqları/yenidən cəhdlər/stalls; `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` və `dashboards/alerts/tests/soranet_policy_rules.test.yml` vasitəsilə təsdiq edilmişdir.
- `dashboards/alerts/sorafs_capacity_rules.yml` — tutumun təzyiqi üstəgəl təmir SLA/ardıcıllıq/icarə müddəti bitməsi xəbərdarlığı və saxlama süpürmələri üçün GC dayanma/bloklanmış/xəta xəbərdarlıqları.
- `dashboards/alerts/soranet_privacy_rules.yml` — məxfilik səviyyəsinin aşağı salınması, susdurma siqnalları, kollektorun boş işləməsinin aşkarlanması və kollektorun qeyri-aktivliyi barədə xəbərdarlıqlar (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — `sorafs_orchestrator_brownouts_total`-ə qoşulmuş anonimlik söndürmə siqnalları.
- `dashboards/alerts/taikai_viewer_rules.yml` — Taikai tamaşaçısının sürüşməsi/indirilməsi/CEK gecikmə həyəcan siqnalları və `torii_sorafs_proof_health_*` tərəfindən dəstəklənən yeni SoraFS sağlamlığa nəzarət cəzası/soyutma siqnalları.

## İzləmə Strategiyası

- OpenTelemetry-ni uçdan uca qəbul edin:
  - Şlüzlər sorğu identifikatorları, manifest həzmləri və token heşləri ilə qeyd edilmiş OTLP aralıqlarını (HTTP) yayır.
  - Orkestr götürmə cəhdləri üçün aralıqları ixrac etmək üçün `tracing` + `opentelemetry` istifadə edir.
  - Daxil edilmiş SoraFS qovşaqlarının PoR problemləri və saxlama əməliyyatları üçün ixrac aralığı. Bütün komponentlər `x-sorafs-trace` vasitəsilə yayılan ümumi iz identifikatorunu paylaşır.
- `SorafsFetchOtel` orkestr göstəricilərini OTLP histoqramlarında körpüləşdirir, `telemetry::sorafs.fetch.*` hadisələri log-merkezli arxa uçlar üçün yüngül JSON yüklərini təmin edir.
- Kollektorlar: OTEL kollektorlarını Prometheus/Loki/Tempo ilə birlikdə idarə edin (Tempo üstünlük verilir). Jaeger API ixracatçıları isteğe bağlı olaraq qalır.
- Yüksək kardinallıq əməliyyatları nümunə götürülməlidir (uğur yolları üçün 10%, uğursuzluqlar üçün 100%).

## TLS Telemetriya Koordinasiyası (SF-5b)

- Metrik uyğunlaşdırma:
  - TLS avtomatlaşdırma gəmiləri `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` və `sorafs_gateway_tls_ech_enabled`.
  - Bu ölçüləri TLS/Sertifikatlar panelinin altındakı Gateway Baxış panelinə daxil edin.
- Xəbərdarlıq əlaqəsi:
  - TLS-in istifadə müddətinin bitməsi ilə bağlı xəbərdarlıqlar (≤14 gün qalır) etibarsız mövcudluq SLO ilə əlaqələndirilir.
  - ECH deaktiv edilməsi həm TLS, həm də əlçatanlıq panellərinə istinad edən ikinci dərəcəli xəbərdarlıq verir.
- Boru kəməri: TLS avtomatlaşdırma işi şlüz ölçüləri ilə eyni Prometheus yığınına ixrac edir; SF-5b ilə koordinasiya deduplikasiya edilmiş cihazları təmin edir.

## Metrik Adlandırma və Etiket Konvensiyaları

- Metrik adlar Torii və şlüz tərəfindən istifadə edilən mövcud `torii_sorafs_*` və ya `sorafs_*` prefikslərini izləyir.
- Etiket dəstləri standartlaşdırılıb:
  - `result` → HTTP nəticəsi (`success`, `refused`, `failed`).
  - `reason` → imtina/səhv kodu (`unsupported_chunker`, `timeout` və s.).
  - `status` → təmir tapşırığı vəziyyəti (`queued`, `in_progress`, `completed`, `failed`, `escalated`).
  - `outcome` → təmir icarəsi və ya gecikmə nəticəsi (`requeued`, `escalated`, `completed`, `failed`).
  - `provider` → hex kodlu provayder identifikatoru.
  - `manifest` → kanonik manifest həzm (yüksək kardinallıq olduqda kəsilir).
  - `tier` → deklarativ səviyyəli etiketlər (`hot`, `warm`, `archive`).
- Telemetriya emissiya nöqtələri:
  - Gateway ölçüləri `torii_sorafs_*` altında yaşayır və `crates/iroha_core/src/telemetry.rs` konvensiyalarından təkrar istifadə edin.
  - Orkestrator `sorafs_orchestrator_*` ölçülərini və manifest həzm, iş identifikatoru, region və provayder identifikatorları ilə işarələnmiş `telemetry::sorafs.fetch.*` hadisələrini (həyat dövrü, təkrar cəhd, provayder uğursuzluğu, xəta, dayanma) yayır.
  - Düyünlərin səthi `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` və `torii_sorafs_por_*`.
- Etiketin əsaslılıq gözləntiləri daxil olmaqla, paylaşılan Prometheus adlandırma sənədində metrik kataloqu qeydiyyatdan keçirmək üçün Müşahidə Edilənlik ilə əlaqələndirin (provayder/yuxarı hədləri göstərir).

## Məlumat Boru Kəməri

- Kollektorlar OTLP-ni Prometheus (metrikalar) və Loki/Tempo (loglar/izlər)-ə ixrac edərək hər bir komponentlə yanaşı yerləşdirirlər.
- Əlavə eBPF (Tetraqon) şlüzlər/qovşaqlar üçün aşağı səviyyəli izləməni zənginləşdirir.
- Torii və daxili qovşaqlar üçün `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` istifadə edin; orkestr `install_sorafs_fetch_otlp_exporter` zəng etməyə davam edir.

## Doğrulama qarmaqları

- CI zamanı `scripts/telemetry/test_sorafs_fetch_alerts.sh`-i işə salın, Prometheus xəbərdarlığı qaydalarının dayanma metrikləri və məxfiliyin qarşısının alınması yoxlamaları ilə kilidləndiyini təmin edin.
- Grafana tablosunu versiya nəzarəti altında saxlayın (`dashboards/grafana/`) və panellər dəyişdikdə ekran görüntülərini/linkləri yeniləyin.
- Xaos `scripts/telemetry/log_sorafs_drill.sh` vasitəsilə nəticələri qeyd edir; doğrulama `scripts/telemetry/validate_drill_log.sh` imkanlarından istifadə edir ([Əməliyyat Kitabına](operations-playbook.md) baxın).