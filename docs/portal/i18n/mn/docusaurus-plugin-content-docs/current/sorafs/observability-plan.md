---
id: observability-plan
lang: mn
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

::: Каноник эх сурвалжийг анхаарна уу
:::

## Зорилтууд
- Гарц, зангилаа, олон эх сурвалжийн найруулагчийн хэмжүүр, бүтэцлэгдсэн үйл явдлыг тодорхойлох.
- Grafana хяналтын самбар, дохиоллын босго, баталгаажуулалтын дэгээгээр хангана.
- Алдаа төсөв, эмх замбараагүй өрмийн бодлогын зэрэгцээ SLO зорилтуудыг бий болгох.

## Метрийн каталог

### Гарцын гадаргуу

| Метрик | Төрөл | Шошго | Тэмдэглэл |
|--------|------|--------|-------|
| `sorafs_gateway_active` | Хэмжигч (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | `SorafsGatewayOtel`-ээр дамжуулсан; эцсийн цэг/аргын хослол тус бүрээр нислэгийн HTTP үйлдлүүдийг хянадаг. |
| `sorafs_gateway_responses_total` | Тоолуур | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, I18NI000000040X, I18NI00000041020 | Дууссан гарцын хүсэлт бүр нэг удаа нэмэгддэг; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Гистограм | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, I18NI000000053X, I18NI000000540005 | Гарцын хариултын эхний байт хүртэлх хугацаа; Prometheus `_bucket/_sum/_count` хэлбэрээр экспортолсон. |
| `sorafs_gateway_proof_verifications_total` | Тоолуур | `profile_version`, `result`, `error_code` | Хүсэлтийн үед авсан нотлох баталгаажуулалтын үр дүн (`result` ∈ {`success`, `failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Гистограм | `profile_version`, `result`, `error_code` | PoR баримтын баталгаажуулалтын хоцрогдлын хуваарилалт. |
| `telemetry::sorafs.gateway.request` | Бүтэцлэгдсэн арга хэмжээ | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Локи/Темпо корреляцын хүсэлтийг гүйцээх бүрт гаргадаг бүтэцлэгдсэн бүртгэл. |

`telemetry::sorafs.gateway.request` events mirror the OTEL counters with structured payloads, surfacing `endpoint`, `method`, `variant`, `status`, `error_code`, and `duration_ms` for Хяналтын самбарууд SLO хянахын тулд OTLP цувралыг ашигладаг бол Loki/Tempo хамаарал.

### Эрүүл мэндийн баталгааны телеметр

| Метрик | Төрөл | Шошго | Тэмдэглэл |
|--------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Тоолуур | `provider_id`, `trigger`, `penalty` | `RecordCapacityTelemetry` нь `SorafsProofHealthAlert` ялгарах бүрт нэмэгддэг. `trigger` нь PDP/PoTR/Хоёулаа бүтэлгүйтлийг ялгадаг бол `penalty` нь барьцаа хөрөнгө үнэхээр тасарсан эсвэл хөргөлтийн үед дарагдсан эсэхийг харуулдаг. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | хэмжигч | `provider_id` | Хамгийн сүүлийн үеийн PDP/PoTR тооллогыг зөрчиж буй телеметрийн цонхонд мэдээлсэн тул багууд үйлчилгээ үзүүлэгчдийн бодлогыг хэр хол давсныг тоон үзүүлэлтээр тодорхойлох боломжтой. |
| `torii_sorafs_proof_health_penalty_nano` | хэмжигч | `provider_id` | Сүүлийн сэрэмжлүүлгийн үед Nano-XOR-ын хэмжээг бууруулсан (хөргөх хугацаа хэрэгжих үед тэг болно). |
| `torii_sorafs_proof_health_cooldown` | хэмжигч | `provider_id` | Дагах дохионы дууг түр хаасан үед логик хэмжигч (`1` = сэрүүцэх үед дарагдсан дохиолол). |
| `torii_sorafs_proof_health_window_end_epoch` | хэмжигч | `provider_id` | Операторууд Norito олдворуудтай уялдаатай байхын тулд дохиололд холбогдсон телеметрийн цонхонд бүртгэгдсэн эрин үе. |

Эдгээр хангамж нь одоо Taikai үзэгчийн хяналтын самбарын эрүүл мэндийн баталгааны мөрийг идэвхжүүлдэг
(`dashboards/grafana/taikai_viewer.json`) нь CDN операторуудад шууд харагдах байдлыг өгдөг
дохиоллын хэмжээ, PDP/PoTR триггерийн холимог, торгууль, хөргөлтийн төлөв
үйлчилгээ үзүүлэгч.

Үүнтэй ижил хэмжүүрүүд одоо Taikai үзэгчийн сэрэмжлүүлгийн хоёр дүрмийг буцаана:
`SorafsProofHealthPenalty` хэзээ ч гал асаана
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` нэмэгдэнэ
сүүлийн 15 минут бол `SorafsProofHealthCooldown` анхааруулга өгдөг.
үйлчилгээ үзүүлэгч таван минутын турш хөргөлтийн горимд байна. Хоёр анхааруулга хоёуланд нь амьдардаг
`dashboards/alerts/taikai_viewer_rules.yml` тул SRE нь шууд контекстийг хүлээн авдаг
PoR/PoTR-ийн хэрэгжилт хурцдах бүрд.

### Оркестрийн гадаргуу

| Метрик / Үйл явдал | Төрөл | Шошго | Үйлдвэрлэгч | Тэмдэглэл |
|----------------|------|--------|----------|-------|
| `sorafs_orchestrator_active_fetches` | хэмжигч | `manifest_id`, `region` | `FetchMetricsCtx` | Одоогоор нислэгт байгаа сессүүд. |
| `sorafs_orchestrator_fetch_duration_ms` | Гистограм | `manifest_id`, `region` | `FetchMetricsCtx` | Миллисекундээр үргэлжлэх гистограмм; 1 мс→30 секундын хувин. |
| `sorafs_orchestrator_fetch_failures_total` | Тоолуур | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Шалтгаан: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Тоолуур | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Дахин оролдох шалтгааныг ялгадаг (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Тоолуур | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Сеансын түвшний идэвхгүй байдал / бүтэлгүйтлийн тоог бичнэ. |
| `sorafs_orchestrator_chunk_latency_ms` | Гистограм | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Дамжуулах хүчин чадал/SLO шинжилгээнд зориулж нэг хэсэг болгон татах хоцрогдлын хуваарилалт (ms). |
| `sorafs_orchestrator_bytes_total` | Тоолуур | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Манифест/үйлүүлэгч тус бүрээр илгээсэн байт; PromQL дээрх `rate()`-ээр дамжуулан дамжуулах чадварыг олж авах. |
| `sorafs_orchestrator_stalls_total` | Тоолуур | `manifest_id`, `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms`-ээс хэтэрсэн хэсгүүдийг тоолно. |
| `telemetry::sorafs.fetch.lifecycle` | Бүтэцлэгдсэн арга хэмжээ | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, I18NI0000016010, I18NI0000016010, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Norito JSON ачаалалтай ажлын амьдралын мөчлөгийг (эхлэх/дуусгах) толин тусгал. |
| `telemetry::sorafs.fetch.retry` | Бүтэцлэгдсэн арга хэмжээ | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Үйлчилгээ үзүүлэгчийн дахин оролдлого тутамд ялгарсан; `attempts` нь өсөн нэмэгдэж буй дахин оролдлогуудыг тоолдог (≥1). |
| `telemetry::sorafs.fetch.provider_failure` | Бүтэцлэгдсэн арга хэмжээ | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Үйлчилгээ үзүүлэгч бүтэлгүйтлийн босгыг давах үед гарч ирдэг. |
| `telemetry::sorafs.fetch.error` | Бүтэцлэгдсэн арга хэмжээ | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Терминал гэмтлийн бүртгэл, Loki/Splunk залгихад тохиромжтой. |
| `telemetry::sorafs.fetch.stall` | Бүтэцлэгдсэн арга хэмжээ | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Хэмжээний хоцролт нь тохируулсан таглааг зөрчих үед нэмэгддэг (толин тусгалууд лангууны тоолуур). |

### Зангилаа / хуулбарлах гадаргуу

| Метрик | Төрөл | Шошго | Тэмдэглэл |
|--------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Гистограм | `provider_id` | Хадгалах ашиглалтын хувийн OTEL гистограмм (`_bucket/_sum/_count` гэж экспортолсон). |
| `sorafs_node_por_success_total` | Тоолуур | `provider_id` | Хуваарьлагчийн агшин зуурын зургуудаас авсан амжилттай PoR дээжүүдэд зориулсан монотон тоолуур. |
| `sorafs_node_por_failure_total` | Тоолуур | `provider_id` | Амжилтгүй PoR дээжийн монотон тоолуур. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | хэмжигч | `provider` | Одоо байгаа Prometheus хэмжигч ашигласан байт, дарааллын гүн, PoR нислэгийн тоо. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | хэмжигч | `provider` | Хүчин чадлын хяналтын самбарт үйлчилгээ үзүүлэгчийн хүчин чадал/ажиллах хугацааны амжилтын өгөгдөл гарч ирэв. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | хэмжигч | `provider`, `manifest` | `/v2/sorafs/por/ingestion/{manifest}`-г санал асуулга авах бүрд экспортын хуримтлагдсан алдааны тоолуур дээр нэмээд "PoR лангуу" самбар/сануулгыг тэжээдэг. |

### Засвар ба SLA

| Метрик | Төрөл | Шошго | Тэмдэглэл |
|--------|------|--------|-------|
| `sorafs_repair_tasks_total` | Тоолуур | `status` | Засварын ажлын шилжилтийн OTEL тоолуур. |
| `sorafs_repair_latency_minutes` | Гистограм | `outcome` | Засварын амьдралын мөчлөгийн хоцрогдолд зориулсан OTEL гистограм. |
| `sorafs_repair_queue_depth` | Гистограм | `provider` | Үйлчилгээ үзүүлэгч бүрийн дараалалд байгаа ажлуудын OTEL гистограмм (хормын хувилбар). |
| `sorafs_repair_backlog_oldest_age_seconds` | Гистограм | — | Хамгийн эртний дараалалд байсан ажлын насны OTEL гистограмм (секунд). |
| `sorafs_repair_lease_expired_total` | Тоолуур | `outcome` | Түрээсийн хугацаа дууссан OTEL тоолуур (`requeued`/`escalated`). |
| `sorafs_repair_slash_proposals_total` | Тоолуур | `outcome` | OTEL тоологч нь ташуу саналын шилжилтийн. |
| `torii_sorafs_repair_tasks_total` | Тоолуур | `status` | Даалгаврын шилжилтийн Prometheus тоолуур. |
| `torii_sorafs_repair_latency_minutes_bucket` | Гистограм | `outcome` | Засварын амьдралын мөчлөгийн хоцрогдолд зориулсан Prometheus гистограм. |
| `torii_sorafs_repair_queue_depth` | хэмжигч | `provider` | Нэг үйлчилгээ үзүүлэгчийн дараалалд байгаа ажлуудын Prometheus хэмжигч. |
| `torii_sorafs_repair_backlog_oldest_age_seconds` | хэмжигч | — | Prometheus хамгийн эртний дараалалд байгаа ажлын насны хэмжигч (секунд). |
| `torii_sorafs_repair_lease_expired_total` | Тоолуур | `outcome` | Түрээсийн хугацаа дуусах Prometheus тоолуур. |
| `torii_sorafs_slash_proposals_total` | Тоолуур | `outcome` | Prometheus налуу саналын шилжилтийн тоолуур. |

Засаглалын аудитын JSON мета өгөгдөл нь засварын телеметрийн шошгыг тусгадаг (`status`, `ticket_id`, `manifest`, засварын үйл явдал дээрх `provider`; `outcome` slash correlics болон slash correlics дээр аудит хийх) шийдэмгий байдлаар.

### Хадгалах ба GC| Метрик | Төрөл | Шошго | Тэмдэглэл |
|--------|------|--------|-------|
| `sorafs_gc_runs_total` | Тоолуур | `result` | GC шүүрэлтийн OTEL тоолуур, суулгагдсан зангилаанаас ялгардаг. |
| `sorafs_gc_evictions_total` | Тоолуур | `reason` | Шалтгаанаар нь бүлэглэсэн нүүлгэгдсэн манифестийн OTEL тоолуур. |
| `sorafs_gc_bytes_freed_total` | Тоолуур | `reason` | Шалтгаанаар бүлэглэсэн суллагдсан байтуудын OTEL тоолуур. |
| `sorafs_gc_blocked_total` | Тоолуур | `reason` | Идэвхтэй засвар эсвэл бодлогоор хаасан нүүлгэн шилжүүлэх OTEL тоолуур. |
| `torii_sorafs_gc_runs_total` | Тоолуур | `result` | GC шүүрэлтийн Prometheus тоолуур (амжилт/алдаа). |
| `torii_sorafs_gc_evictions_total` | Тоолуур | `reason` | Prometheus Шалтгаанаар бүлэглэсэн хөөгдсөн манифестийн тоолуур. |
| `torii_sorafs_gc_bytes_freed_total` | Тоолуур | `reason` | Prometheus шалтгаанаар бүлэглэсэн суллагдсан байтын тоологч. |
| `torii_sorafs_gc_blocked_total` | Тоолуур | `reason` | Prometheus шалтгаанаар бүлэглэсэн хаагдсан нүүлгэлтийн тоолуур. |
| `torii_sorafs_gc_expired_manifests` | хэмжигч | — | Хугацаа нь дууссан манифестийн одоогийн тоо GC шүүрэлтийг ажиглаж байна. |
| `torii_sorafs_gc_oldest_expired_age_seconds` | хэмжигч | — | Хугацаа нь дууссан хамгийн эртний манифестийн хэдэн секундын нас (нигүүлсэл хадгалсны дараа). |

### Эвлэрэл

| Метрик | Төрөл | Шошго | Тэмдэглэл |
|--------|------|--------|-------|
| `sorafs.reconciliation.runs_total` | Тоолуур | `result` | Эвлэрүүлэн зуучлах агшин зуурын зураг авах OTEL тоолуур. |
| `sorafs.reconciliation.divergence_total` | Тоолуур | — | Нэг гүйлтийн зөрүүний тоолох OTEL тоолуур. |
| `torii_sorafs_reconciliation_runs_total` | Тоолуур | `result` | Prometheus Тоолуур гүйлгээг нэгтгэх. |
| `torii_sorafs_reconciliation_divergence_count` | хэмжигч | — | Эвлэрлийн тайланд ажиглагдсан хамгийн сүүлийн зөрүү. |

### Цаг тухайд нь олж авсаны баталгаа (PoTR) болон хэсэгчилсэн SLA

| Метрик | Төрөл | Шошго | Үйлдвэрлэгч | Тэмдэглэл |
|--------|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | Гистограм | `tier`, `provider` | PoTR зохицуулагч | Эцсийн хугацаа миллисекундээр суларсан (эерэг = хангагдсан). |
| `sorafs_potr_failures_total` | Тоолуур | `tier`, `provider`, `reason` | PoTR зохицуулагч | Шалтгаан: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Тоолуур | `provider`, `manifest_id`, `reason` | SLA монитор | Хэсэгчилсэн хүргэлт SLO (хоцролт, амжилтын хувь)-аас хоцрох үед галладаг. |
| `sorafs_chunk_sla_violation_active` | хэмжигч | `provider`, `manifest_id` | SLA монитор | Булийн хэмжигч (0/1) идэвхтэй зөрчлийн цонхны үед шилжсэн. |

## SLO зорилтууд

- Гарцын найдвартай байдал: **99.9%** (HTTP 2xx/304 хариулт).
- Итгэлгүй TTFB P95: халуун түвшин ≤120ms, дулаан давхарга ≤300ms.
- Баталгаажуулах амжилтын түвшин: Өдөрт ≥99.5%.
- Оркестрийн амжилт (хэсэг дуусгах): ≥99%.

## Хяналтын самбар ба анхааруулга

1. **Gateway Observability** (`dashboards/grafana/sorafs_gateway_observability.json`) — найдвартай байдал, TTFB P95, татгалзсан задаргаа болон PoR/PoTR алдааг OTEL хэмжигдэхүүнээр хянадаг.
2. **Оркестрийн эрүүл мэнд** (`dashboards/grafana/sorafs_fetch_observability.json`) — олон эх сурвалжийн ачаалал, дахин оролдлого, үйлчилгээ үзүүлэгчийн алдаа, зогсолтын тасалдлыг хамарна.
3. **SoraNet Privacy Metrics** (`dashboards/grafana/soranet_privacy_metrics.json`) — `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`, I18NI00000002-ээр дамжуулан нэрээ нууцалсан реле хувин, дарангуйлах цонх, коллекторын эрүүл мэндийн графикийг харуулдаг.
4. **Capacity Health** (`dashboards/grafana/sorafs_capacity_health.json`) — үйлчилгээ үзүүлэгчийн зай болон засварын SLA нэмэгдэл, үйлчилгээ үзүүлэгчээр засварлах дарааллын гүн, GC шүүрүүлэх/нүүлгэх/байтын чөлөөлөгдсөн/хаагдсан шалтгаан/хугацаа дууссан-манифестийн нас болон эвлэрлийн зөрүүг snap-г хянадаг.

Анхааруулгын багцууд:

- `dashboards/alerts/sorafs_gateway_rules.yml` — гарцын олдоц, TTFB, алдааны огцом өсөлт.
- `dashboards/alerts/sorafs_fetch_rules.yml` — найруулагчийн алдаа/дахин оролдох/лангуу; `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml`, `dashboards/alerts/tests/soranet_policy_rules.test.yml`-ээр баталгаажуулсан.
- `dashboards/alerts/sorafs_capacity_rules.yml` — хүчин чадлын даралтыг нэмээд засварын SLA/хоцрогдол/түрээсийн хугацаа дууссаны анхааруулга болон хадгалалтын цэвэрлэгээнд зориулсан GC зогсолт/хоригдсон/алдааны дохиолол.
- `dashboards/alerts/soranet_privacy_rules.yml` — нууцлалын зэрэглэл буурах, дарах дохиолол, коллекторын сул зогсолтыг илрүүлэх, идэвхгүй болсон коллекторын дохиолол (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — нэрээ нууцлах дохиолол `sorafs_orchestrator_brownouts_total` руу холбогдсон.
- `dashboards/alerts/taikai_viewer_rules.yml` — Taikai үзэгчийн drift/ingest/CEK хоцрогдол дохиолол, мөн `torii_sorafs_proof_health_*`-ээр ажилладаг шинэ SoraFS эрүүл мэндийн баталгааны торгууль/хөргөлтийн дохио.

## Мөшгих стратеги

- OpenTelemetry-г эцэс төгсгөлгүй нэвтрүүлэх:
  - Гарцууд нь хүсэлтийн ID, манифест дижест, токен хэш бүхий тайлбар бүхий OTLP spans (HTTP) ялгаруулдаг.
  - Оркестр нь дуудах оролдлогын зайг экспортлохдоо `tracing` + `opentelemetry` ашигладаг.
  - Embedded SoraFS зангилаа нь PoR сорилтууд болон хадгалах үйл ажиллагаанд зориулагдсан экспортын зай. Бүх бүрэлдэхүүн хэсгүүд нь `x-sorafs-trace`-ээр тархсан нийтлэг ул мөр ID-г хуваалцдаг.
- `SorafsFetchOtel` оркестрийн хэмжигдэхүүнийг OTLP гистограмм руу холбодог бол `telemetry::sorafs.fetch.*` үйл явдлууд нь лог төвтэй арын хэсэгт JSON-ийн хөнгөн ачааллыг өгдөг.
- Цуглуулагчид: OTEL цуглуулагчдыг Prometheus/Loki/Tempo-той зэрэгцүүлэн ажиллуул (Темпо илүүд үздэг). Jaeger API экспортлогчид сонголттой хэвээр байна.
- Өндөр кардиналтай үйл ажиллагаанаас дээж авах хэрэгтэй (амжилтын замд 10%, бүтэлгүйтэл 100%).

## TLS телеметрийн зохицуулалт (SF-5b)

- Метрийн зэрэглэл:
  - TLS автоматжуулалт нь `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}`, `sorafs_gateway_tls_ech_enabled` зэргийг нийлүүлдэг.
  - Эдгээр хэмжигчийг TLS/Certificates самбарын доорх Gateway тойм хяналтын самбарт оруулна уу.
- Анхааруулга холболт:
  - TLS-ийн хугацаа дуусах үед гал гарах үед (≤14 хоног үлдсэн) найдвартай SLO-той холбоотой.
  - ECH-ийн идэвхгүй байдал нь TLS болон бэлэн байдлын самбарын аль алиныг нь харуулсан хоёрдогч анхааруулга гаргадаг.
- Дамжуулах хоолой: TLS автоматжуулалтын ажил нь гарцын хэмжигдэхүүнтэй ижил Prometheus стек рүү экспортлодог; SF-5b-тай зохицуулалт хийснээр давхардсан багаж хэрэгслийг баталгаажуулна.

## Метрийн нэршил ба шошгоны конвенци

- Метрийн нэрс нь Torii болон гарцын ашигладаг `torii_sorafs_*` эсвэл `sorafs_*` угтваруудыг дагадаг.
- Шошгоны багцыг стандартчилсан:
  - `result` → HTTP үр дүн (`success`, `refused`, `failed`).
  - `reason` → татгалзах/алдааны код (`unsupported_chunker`, `timeout` гэх мэт).
  - `status` → засварын ажлын төлөв (`queued`, `in_progress`, `completed`, `failed`, `escalated`).
  - `outcome` → засварын түрээс эсвэл саатлын үр дүн (`requeued`, `escalated`, `completed`, `failed`).
  - `provider` → hex кодлогдсон үйлчилгээ үзүүлэгчийн танигч.
  - `manifest` → каноник манифест дижест (кардинал өндөртэй үед тайрсан).
  - `tier` → тунхаглалын түвшний шошгууд (`hot`, `warm`, `archive`).
- Телеметрийн цацрагийн цэгүүд:
  - Гарцын хэмжигдэхүүн нь `torii_sorafs_*`-ийн дагуу ажилладаг бөгөөд `crates/iroha_core/src/telemetry.rs`-ийн конвенцуудыг дахин ашигладаг.
  - Оркестратор нь `sorafs_orchestrator_*` хэмжүүр болон `telemetry::sorafs.fetch.*` үйл явдлуудыг (амьдралын мөчлөг, дахин оролдох, үйлчилгээ үзүүлэгчийн алдаа, алдаа, зогсолт) ялгаруулж, манифест дижест, ажлын ID, бүс нутаг, үйлчилгээ үзүүлэгчийн танигчтай тэмдэглэдэг.
  - Зангилааны гадаргуу `torii_sorafs_storage_*`, `torii_sorafs_capacity_*`, `torii_sorafs_por_*`.
- Шошгоны үндсэн байдлын хүлээлт (хангагч/манифестийн дээд хязгаар) зэрэг хуваалцсан Prometheus нэршлийн баримт бичигт хэмжигдэхүүн каталогийг бүртгэхийн тулд Observability-тэй хамтран ажиллана.

## Мэдээллийн дамжуулах хоолой

- Цуглуулагчид бүрэлдэхүүн хэсэг бүрийн хажууд байрлуулж, OTLP-ийг Prometheus (хэмжээ) болон Loki/Tempo (лог/мөр) рүү экспортлодог.
- Нэмэлт eBPF (Тетрагон) нь гарц/зангилааны доод түвшний хайлтыг баяжуулдаг.
- Torii болон суулгагдсан зангилааны хувьд `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` ашиглах; Оркестр `install_sorafs_fetch_otlp_exporter` руу залгасаар байна.

## Баталгаажуулах дэгээ

- `scripts/telemetry/test_sorafs_fetch_alerts.sh`-г CI-ийн үед ажиллуулж, Prometheus дохиоллын дүрмүүд зогсолтын хэмжигдэхүүн болон нууцлалыг дарах шалгалтын тусламжтайгаар цоожтой хэвээр байх болно.
- Grafana хяналтын самбарыг хувилбарын хяналтан дор (`dashboards/grafana/`) байлгаж, самбар өөрчлөгдөх үед дэлгэцийн агшин/холбоосыг шинэчил.
- Chaos нь `scripts/telemetry/log_sorafs_drill.sh`-ээр дамжуулан үр дүнг бүртгэдэг; баталгаажуулалт нь `scripts/telemetry/validate_drill_log.sh` хөшүүрэг болдог ([Үйл ажиллагааны номыг](operations-playbook.md) үзнэ үү).