---
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ab50f11b7d9ed6af7d0b1d14f756921ed8cb0798e32a7ea0b109b564c859bdba
source_last_modified: "2026-01-21T19:17:13.231946+00:00"
translation_last_reviewed: 2026-02-07
id: observability-plan
title: SoraFS Observability & SLO Plan
sidebar_label: Observability & SLOs
description: Telemetry schema, dashboards, and error-budget policy for SoraFS gateways, nodes, and the multi-source orchestrator.
translator: machine-google-reviewed
---

:::иҫкәртергә канонлы сығанаҡ
::: 1990 й.

## Маҡсаттар
- Шлюздар, төйөндәр һәм күп сығанаҡлы оркестр өсөн метрика һәм структуралы ваҡиғалар билдәләү.
- I18NT0000000018X приборҙар таҡталары, иҫкәртмә сиктәре һәм валидация ҡармаҡтары менән тәьмин итеү.
- SLO-ны хаталар һәм хаос-быраулы сәйәсәт менән бер рәттән маҡсат итеп ҡуйығыҙ.

## метрик каталог

### Ҡапҡа өҫтө

| Метрика | Тип | Ярлыҡтар | Иҫкәрмәләр |
|-------|-------|---------|-------|
| `sorafs_gateway_active` | Гаж (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, I18NI000000032Х | I18NI000000033X аша сығарылған; осош эсендә HTTP операцияларын күҙәтә, бер ос нөктәһе/метод комбинацияһы. |
| `sorafs_gateway_responses_total` | Ҡаршы | I18NI0000000035X, I18NI00000000036X, I18NI000000037X, I18NI0000000038X, I18NI000000039X X, I18NI0000000040X, I18NI0000000411, `error_code` | Һәр тамамланған шлюз запрос өҫтәүҙәр бер тапҡыр; `result` ∈ {`success`, I18NI000000045X, I18NI000000046XX. |
| `sorafs_gateway_ttfb_ms_bucket` | Гистограмма | I18NI000000048X, I18NI0000000049X, I18NI000000050X, I18NI0000000051X, I18NI00000000052X, I18NI0000000053, I18NI00000000054X, `error_code` | Ваҡыт-беренсе-байтлы латентлыҡ өсөн шлюз яуаптары; экспорты менән I18NT000000000X `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Ҡаршы | `profile_version`, `result`, `error_code` | Һорау ваҡытында алынған иҫбатлау тикшерелеүе һөҙөмтәләре (`result` ∈ {I18NI000000062X,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Гистограмма | `profile_version`, `result`, `error_code` | Тикшеренеүҙең латентлыҡ бүлелеше өсөн PoR квитанциялары. |
| `telemetry::sorafs.gateway.request` | Структуралы ваҡиға | I18NI0000000069X, I18NI000000070X, `variant`, I18NI0000000072X, I18NI000000000073X, I18NI000000074X, I18NI0000000000075 X | Структуралы журнал Loki/Tempo корреляцияһы өсөн һәр үтенесте тамамлау буйынса сығарылған. |

I18NI0000000076X ваҡиғалар көҙгө OTEL счетчиктары менән структуралы файҙалы йөктәр, өҫкө йөҙөү I18NI000000000077X, I18NI0000000078X, I18NI0000000079X, `status`, I18NI0000000081X, һәм `duration_ms` өсөн Loki/Tempo корреляцияһы, ә приборҙар таҡталары SLO күҙәтеү өсөн OTLP серияһын ҡуллана.

### Һаулыҡ һаҡлау телеметрияһы

| Метрика | Тип | Ярлыҡтар | Иҫкәрмәләр |
|-------|-------|---------|-------|
| `torii_sorafs_proof_health_alerts_total` | Ҡаршы | `provider_id`, `trigger`, `penalty` | Өҫтәүҙәр һәр тапҡыр I18NI000000087X I18NI000000088XX сығара. I18NI000000089X PDP/PoTR/Икеһе лә етешһеҙлектәрҙе айыра, ә I18NI000000000X0X contlant ысынында ҡыҫҡартылған йәки һыуытыу ярҙамында баҫтырылғанмы, юҡмы. |
| `torii_sorafs_proof_health_pdp_failures`, I18NI000000092X | Гаж | `provider_id` | Һуңғы PDP/PoTR иҫәптәре тураһында хәбәр итеү эсендә рәнйеткән телеметрия тәҙрәһе, шулай итеп, командалар һанлы ни тиклем алыҫ провайдерҙар артыҡ атыу сәйәсәте. |
| `torii_sorafs_proof_health_penalty_nano` | Гаж | `provider_id` | Нано-XOR суммаһы ҡырҡылған һуңғы иҫкәртмә (нуль ҡасан courcewin баҫтырылған үтәү). |
| `torii_sorafs_proof_health_cooldown` | Гаж | `provider_id` | Булет датчигы (`1` = һыуытҡыс менән иҫкәртмә) ер өҫтөнә, ҡасан эҙмә-эҙлекле иҫкәртмәләр ваҡытлыса тын алыу. |
| `torii_sorafs_proof_health_window_end_epoch` | Гаж | `provider_id` | Эпоха телеметрия тәҙрәһе өсөн теркәлгән иҫкәртмәгә бәйле, шуға күрә операторҙар I18NT0000000020X артефакттарына ҡаршы корреляциялана ала. |

Был каналдар хәҙер ҡөҙрәт тайкай тамашасы приборҙар таҡтаһы’s дәлил-һаулыҡ рәт
(`dashboards/grafana/taikai_viewer.json`), CDN операторҙарына йәшәү күренеше бирә
уяу күләмдәренә, PDP/PoTR триггер ҡатнашмаһы, штрафтар, һәм һыуытҡыс хәле бер .
провайдер.

Шул уҡ метрика хәҙер кире ике Тайкай тамашасы иҫкәртмә ҡағиҙәләре:
I18NI000000102X 1990 йылда утҡа инә.
I18NI000000103X 2012 йылда арта.
һуңғы 15 минут, шул уҡ ваҡытта I18NI000000104X иҫкәртмә күтәрә, әгәр ҙә
провайдер биш минутта һыуытыуҙа ҡала. Ике уяу ла 2019 йылда йәшәй.
I18NI000000105X шулай SREs тиҙ арала контекст ала
ҡасан да булһа PoR/PoTR үтәү эскала.

### Оркестратор өҫтө

| Метрика / Ваҡиға | Тип | Ярлыҡтар | Продюсер | Иҫкәрмәләр |
|--------------|--------|-------------------|--------|
| `sorafs_orchestrator_active_fetches` | Гаж | `manifest_id`, `region` X | `FetchMetricsCtx` | Сессиялар әлеге ваҡытта осоуҙа. |
| `sorafs_orchestrator_fetch_duration_ms` | Гистограмма | `manifest_id`, `region` | `FetchMetricsCtx` | Миллисекундтарҙа Дюритет гистограммаһы; 1мс→30s биҙрә. |
| `sorafs_orchestrator_fetch_failures_total` | Ҡаршы | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Сәбәптәре: `no_providers`, I18NI000000120X, `no_compatible_providers`, I18NI0000000122X, I18NI000000123X, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Ҡаршы | `manifest_id`, I18NI000000127X, `reason` | `FetchMetricsCtx` | 18NI0000130X, `digest_mismatch`, I18NI000000132X, I18NI000000133X). |
| `sorafs_orchestrator_provider_failures_total` | Ҡаршы | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Ҡабул итеү сессия кимәлендә инвалидлыҡ / етешһеҙлектәр иҫәптәре. |
| `sorafs_orchestrator_chunk_latency_ms` | Гистограмма | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Пер-перчатка fetch латентлыҡ бүленә (мс) өсөн үткәреүсәнлек/SLO анализ. |
| `sorafs_orchestrator_bytes_total` | Ҡаршы | I18NI0000014X, I18NI000000145X | `FetchMetricsCtx` | Байтс тапшырылған бер асыҡ/провайдер; пропускной үткәреү аша I18NI000000147X PromQL. |
| `sorafs_orchestrator_stalls_total` | Ҡаршы | `manifest_id`, I18NI000000150X | `FetchMetricsCtx` | Һандар `ScoreboardConfig::latency_cap_ms`-тан ашыу өлөштәр. |
| `telemetry::sorafs.fetch.lifecycle` | Структуралы ваҡиға | I18NI000000154X, I18NI0000001555 X, I18NI000000156X, I18NI0000000157X, `status`, I18NI000000159X, I18NI0000000160X, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Көҙгөләр эш циклы (старт/тулы) менән I18NT000000021X JSON файҙалы йөк. |
| `telemetry::sorafs.fetch.retry` | Структуралы ваҡиға | `manifest`, `region`, `job_id`, I18NI000000169X, `reason`, I18NI000000171X | `FetchTelemetryCtx` | Эмитированный бер провайдер ҡабаттан тырышып һыҙат; I18NI000000173X һандары өҫтәмә ретристар (≥1). |
| `telemetry::sorafs.fetch.provider_failure` | Структуралы ваҡиға | `manifest`, `region`, `job_id`, I18NI000000178X, I18NI000000179X, I18NI0000000180X | `FetchTelemetryCtx` | Өҫтөрәк, ҡасан провайдер уңышһыҙлыҡ сиге аша үткән. |
| `telemetry::sorafs.fetch.error` | Структуралы ваҡиға | I18NI000000183X, I18NI000000184X, I18NI000000185X, I18NI000000186X, `provider?`, I18NI0000000188X, I18NI0000000189 X | `FetchTelemetryCtx` | Терминал етешһеҙлектәре рекорды, дуҫ Локи/Splunk ашау. |
| `telemetry::sorafs.fetch.stall` | Структуралы ваҡиға | I18NI000000192X, `region`, `job_id`, I18NI000000195X, I18NI000000196X, I18NI000000197X | `FetchTelemetryCtx` | Ҡасан үҫтерелгән ҡасан өлөшө латентность боҙа конфигурацияланған ҡапҡас (көҙгөләр стойка счетчиктар). |

### Төйөн / репликация өҫтө

| Метрика | Тип | Ярлыҡтар | Иҫкәрмәләр |
|-------|-------|---------|-------|
| `sorafs_node_capacity_utilisation_pct` | Гистограмма | `provider_id` | OTEL гистограмма һаҡлау утилләштереү проценты (экспортланған `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Ҡаршы | `provider_id` | Монотонный счетчик өсөн уңышлы PoR өлгөләре, график снимоктарҙан алынған. |
| `sorafs_node_por_failure_total` | Ҡаршы | `provider_id` | Монотонный счетчик өсөн уңышһыҙ PoR өлгөләре. |
| `torii_sorafs_storage_bytes_*`, I18NI000000207X | Гаж | `provider` | Ҡулланылған байттар өсөн булған I18NT000000001X датчигы, сират тәрәнлеге, PoR осоу һандары. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, I18NI000002111X | Гаж | `provider` | Провайдер ҡәҙерле/өҫтөндә уңыш мәғлүмәттәре ҡөҙрәтле приборҙар панелендә өҫкә сыҡты. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Гаж | `provider`, `manifest` | Арка тәрәнлеге плюс кумулятив етешһеҙлектәр иҫәпләүселәр экспортҡа ҡасан I18NI000000217X һорау алыу, туҡландырыу “PoR Stalls” панель/иҫкәртмә. |

### Ремонт & СЛА

| Метрика | Тип | Ярлыҡтар | Иҫкәрмәләр |
|-------|-------|---------|-------|
| `sorafs_repair_tasks_total` | Ҡаршы | `status` | OTEL счетчик өсөн ремонт бурыстарын күсеү. |
| `sorafs_repair_latency_minutes` | Гистограмма | `outcome` | OTEL гистограммаһы өсөн ремонт йәшәү циклы латентлығы. |
| `sorafs_repair_queue_depth` | Гистограмма | `provider` | OTEL гистограммаһы сиратлы бурыстарҙы бер провайдер (снимок стилендә). |
| `sorafs_repair_backlog_oldest_age_seconds` | Гистограмма | — | ОТЕЛ гистограммаһы иң боронғо сиратлы бурыс йәшенең (икенсе). |
| `sorafs_repair_lease_expired_total` | Ҡаршы | `outcome` | Ҡуртымға алыу ваҡыты өсөн OTEL счетчигы (I18NI000000227X/`escalated`X). |
| `sorafs_repair_slash_proposals_total` | Ҡаршы | `outcome` | OTEL счетчик өсөн слэш тәҡдим күсеүҙәре. |
| `torii_sorafs_repair_tasks_total` | Ҡаршы | `status` | I18NT000000002Х счетчик өсөн бурыстар күсеүҙәре. |
| `torii_sorafs_repair_latency_minutes_bucket` | Гистограмма | `outcome` | I18NT000000003X гистограммаһы өсөн ремонт тормош циклы латентлығы. |
| `torii_sorafs_repair_queue_depth` | Гаж | `provider` | I18NT000000004X датчигы өсөн сират буйынса бурыстар өсөн бер провайдер. |
| `torii_sorafs_repair_backlog_oldest_age_seconds` | Гаж | — | I18NT000000005X габарит өсөн иң боронғо сиратлы бурыс йәше (секундтар). |
| `torii_sorafs_repair_lease_expired_total` | Ҡаршы | `outcome` | I18NT000000006X счетчик өсөн ҡуртымға срогы срогы. |
| `torii_sorafs_slash_proposals_total` | Ҡаршы | `outcome` | Prometheus счетчик өсөн ҡыҫҡартыу тәҡдим күсеүҙәре. |

Governance audit JSON metadata mirrors the repair telemetry labels (`status`, `ticket_id`, `manifest`, `provider` on repair events; `outcome` on slash proposals) so metrics and audit artefacts can be корреляциялы детерминистик.

### Һаҡлау & Г.К.| Метрика | Тип | Ярлыҡтар | Иҫкәрмәләр |
|-------|-------|---------|-------|
| `sorafs_gc_runs_total` | Ҡаршы | `result` | OTEL счетчик өсөн ГК һыпыртыуҙар, встроенный төйөн сығарып сығарылған. |
| `sorafs_gc_evictions_total` | Ҡаршы | `reason` | OTEL счетчик өсөн ҡыуып сығарылған манифесттар аҡыл менән төркөмләнгән. |
| `sorafs_gc_bytes_freed_total` | Ҡаршы | `reason` | OTEL счетчик өсөн байттар азат ителгән төркөмләнгән аҡыл менән. |
| `sorafs_gc_blocked_total` | Ҡаршы | `reason` | OTEL счетчик өсөн сығарыу әүҙем ремонт йәки сәйәсәт блокировка. |
| `torii_sorafs_gc_runs_total` | Ҡаршы | `result` | Prometheus өсөн счетчик өсөн ГК һыпыртыуҙар (уңыш/хата). |
| `torii_sorafs_gc_evictions_total` | Ҡаршы | `reason` | I18NT000000009X счетчик өсөн ҡыуып сығарылған манифесттары аҡыл менән төркөмләнгән. |
| `torii_sorafs_gc_bytes_freed_total` | Ҡаршы | `reason` | I18NT000000010X счетчик өсөн байттар азат ителгән төркөмләнгән сәбәп буйынса. |
| `torii_sorafs_gc_blocked_total` | Ҡаршы | `reason` | I18NT000000011X счетчик өсөн блокировкаланған күсерелгәндәр өсөн төркөмләнгән сәбәптәр буйынса. |
| `torii_sorafs_gc_expired_manifests` | Гаж | — | ГК һыпыртыуҙары күҙәтелгән ағымдағы сроклы нәфистәр һаны. |
| `torii_sorafs_gc_oldest_expired_age_seconds` | Гаж | — | Иң боронғо срогы үткән асыҡлыҡтың бер нисә секундында (һаҡлау рәхмәтенән һуң). |

### Яраштырыу

| Метрика | Тип | Ярлыҡтар | Иҫкәрмәләр |
|-------|-------|---------|-------|
| `sorafs.reconciliation.runs_total` | Ҡаршы | `result` | OTEL счетчик өсөн ярашыу снимоктары. |
| `sorafs.reconciliation.divergence_total` | Ҡаршы | — | OTEL счетчик дивергенция иҫәптәре бер йүгерә. |
| `torii_sorafs_reconciliation_runs_total` | Ҡаршы | `result` | I18NT000000012X счетчик өсөн ярашыу йүгерә. |
| `torii_sorafs_reconciliation_divergence_count` | Гаж | — | Һуңғы дивергенция һаны күҙәтелгән ярашыу тураһында отчет. |

### Ваҡытында эҙләү тураһында иҫбатлау (PoTR) & өлөшө SLA

| Метрика | Тип | Ярлыҡтар | Продюсер | Иҫкәрмәләр |
|-------|------|---------|----------|------- |
| `sorafs_potr_deadline_ms` | Гистограмма | `tier`, I18NI000000273X | PoTR координаторы | Миллисекундтарҙа сроклы ялҡау (ыңғай = осрашты). |
| `sorafs_potr_failures_total` | Ҡаршы | `tier`, I18NI000000276X, I18NI0000002777X | PoTR координаторы | Сәбәптәре: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Ҡаршы | `provider`, `manifest_id`, I18NI000000284X | SLA мониторы | Атылғанда, өлөшләтә тапшырыу SLO һағынып (латентлыҡ, уңыш ставкаһы). |
| `sorafs_chunk_sla_violation_active` | Гаж | `provider`, I18NI000000287X | SLA мониторы | Бул датчигы (0/1) әүҙем боҙоу тәҙрәһе ваҡытында ҡырҡылған. |

## СЛО Маҡсаттар

- Ҡапҡа ышаныслы доступность: **99.9%** (HTP 2xx/304 яуаптар).
- Ышанысһыҙ ТТФБ Р95: эҫе ярус ≤120мс, йылы ярус ≤300мс.
- Дәлилдәр уңыш кимәле: көнөнә ≥99,5%.
- Оркестратор уңыш (урын тамамлау): ≥99%.

## Приборҙар таҡталары һәм иҫкәртеү

1. **Шлюз күҙәтеүсәнлеге** (`dashboards/grafana/sorafs_gateway_observability.json`) — ышаныслы мөмкинлектәрҙе күҙәтә, TTFB P95, баш тартыу өҙөлгән, һәм PoR/PoTR етешһеҙлектәре аша OTEL метрикаһы.
2. **Оркестратор Һаулыҡ** (`dashboards/grafana/sorafs_fetch_observability.json`) — күп сығанаҡлы йөкләмәне, ретристар, провайдер етешһеҙлектәре һәм стойка ярсыҡтарын ҡаплай.
**СораНет хосуси метрикаһы** (`dashboards/grafana/soranet_privacy_metrics.json`) — диаграммалар аноним эстафета, баҫтырыу тәҙрәләре, һәм коллектор һаулығы аша `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`, һәм I18NI0000000293.
4. **Ҡөмлөлөк Һаулыҡ ** (I18NI0000294X) — күҙәтә провайдер башы плюс ремонт SLA эскалациялар, ремонт сират тәрәнлеге провайдер, һәм GC һыпыртыу/байте азат ителгән/блокированный сәбәптәр/ваҡытыла-сикле йәш һәм ярашыу дивергенция снимоктар.

Иҫкәртергә өйөмдәр:

- `dashboards/alerts/sorafs_gateway_rules.yml` — шлюздың булыуы, ТТФБ, иҫбатлау етешһеҙлектәре шпицтары.
- `dashboards/alerts/sorafs_fetch_rules.yml` — оркестрҙа етешһеҙлектәр/ҡаршылыҡтар/стойкалар; `scripts/telemetry/test_sorafs_fetch_alerts.sh` аша раҫланған, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml`, һәм I18NI000000300X.
- `dashboards/alerts/sorafs_capacity_rules.yml` — ҡөҙрәт баҫымы плюс ремонтлау SLA/артҡы/арендаторлыҡ-ваҡыт иҫкәртмәләр һәм GC ларек/блокированный/хаталар өсөн иҫкәртмәләр һаҡлау өсөн һыпыртыу.
- I18NI000000302X — хосусилыҡты түбәнәйткән шпицтар, баҫтырыу сигнализацияһы, коллекционер-аллы асыҡлау һәм инвалидтар коллекторы иҫкәртмәләре (`soranet_privacy_last_poll_unixtime`, I18NI0000004X).
- I18NI000000305X — анонимлыҡ браунут сигналдары `sorafs_orchestrator_brownouts_total` тиклем проводной.
- I18NI000000307X — Тайкай тамашасыһы дрейф/эш/CEK лаг сигнализацияһы плюс яңы I18NT000000002X иҫбатлаусы штраф/һыуытҡыс иҫкәртмәләре Prometheus ярҙамында эшләй.

## Эҙләү стратегияһы

- Аңлашма OpenTelemetries осона тиклем:
  - шлюздар OTLP spans (HTTP) сығара, аннотацияланған запрос идентификаторҙары, асыҡ һеңдерелгән, һәм жетон хеш.
  - Оркестрҙа `tracing` + I18NI0000000310X ҡулланыла, был осраҡтар өсөн экспортлау өсөн тырышлыҡтар.
  - Imedded I18NT000000023X төйөндәр экспорт spans өсөн PoR проблемалары һәм һаҡлау операциялары. Бөтә компоненттар ҙа I18NI000000311X аша таралған дөйөм эҙ идентификаторы менән бүлешә.
- I18NI000000312X күперҙәр оркестр метрикаһы OTLP гистограммаларына, ә I18NI000000313X ваҡиғалар лог-үҙәк бэкэндтары өсөн еңел JSON файҙалы йөктәр бирә.
- Коллекторҙар: I18NT000000013X/Локи/Темпо менән бер рәттән OTEL коллекционерҙарын эшләтә (Темпо өҫтөнлөк бирә). Jaere API экспортерҙары теләк буйынса ҡала.
- Юғары кардинальлыҡ операциялары өлгөләр алырға тейеш (уңыш юлдары өсөн 10%, 100% етешһеҙлектәр өсөн).

## TLS Телеметрия координацияһы (SF-5b)

- Метрика тура килтереп:
  - TLS автоматлаштырыу суднолары I18NI000000314X, I18NI000000315X, һәм I18NI000000316X.
  - Был датчиктарҙы TLS/Сертификаттар панелендә шлюз дөйөм приборҙар таҡтаһына индерегеҙ.
- Хәүефһеҙлек бәйләнеше:
  - Ҡасан TLS срогы иҫкәрткән янғын (≤14 көн ҡалған) ышаныслы булыуы менән корреляция SLO.
  - ЕЧ-ты өҙөү TLS һәм доступность панелдәрен дә икенсел иҫкәртмәгә һылтанма яһай.
- Торба: TLS автоматлаштырыу эш экспорты шул уҡ I18NT000000000014X стека шлюз метрикаһы булараҡ; SF-5b менән координациялау өҙөлгән приборҙар тәьмин итә.

## метрик исем биреүҙең & Ярлыҡ конвенциялары

- метрик исемдәр I18NI000000318X I18NT0000000000024X һәм шлюз ярҙамында ҡулланылған `torii_sorafs_*` йәки `sorafs_*` префикстарына эйәреп килә.
- Ярлыҡ комплекттары стандартлаштырыла:
  - `result` → HTTP һөҙөмтәһе (`success`, I18NI000000321X, I18NI000000322Х).
  - I18NI000000323X → баш тартыу/хаталар коды (I18NI000000324X, I18NI000000325X, һ.б.).
  - I18NI000000326X → ремонтлау бурысы дәүләте (I18NI00000000327X, I18NI0000000328X X, I18NI000000329X, I18NI000000330X, I18NI0000000031X).
  - I18NI000000332X → ремонтлау йәки латентлыҡ һөҙөмтәһе (I18NI000000333X, I18NI000000034X, `endpoint`, I18NI0000000000336X).
  - `provider` → гекс-кодланған провайдер идентификаторы.
  - I18NI0000038X → канонлы асыҡ һеңдерелгән (юғары кардинальлыҡ ҡасан ҡырҡылған).
  - I18NI000000339X → декларатив ярус ярлыҡтары (`hot`, `warm`, I18NI000000342X).
- Телеметрия эмиссия нөктәләре:
  - Ҡапҡа метрикаһы `torii_sorafs_*` аҫтында йәшәй һәм `crates/iroha_core/src/telemetry.rs`X-тан конвенцияларҙы ҡабаттан ҡуллана.
  - Оркестратор I18NI000000345X метрикаһы һәм `telemetry::sorafs.fetch.*` ваҡиғалар (йәшәү циклы, ҡабаттан тырышып, провайдер етешһеҙлеге, хатаһы, стойка) асыҡ һеңдерелгән, эш идентификаторы, төбәк һәм провайдер идентификаторҙары менән билдәләнгән.
  - төйөндәр өҫтөндә I18NI000000347X, `torii_sorafs_capacity_*`, һәм I18NI000000349X.
- Координация менән күҙәтеү өсөн теркәү метрик каталог дөйөм I18NT0000000015X исем биреүҙе doc, шул иҫәптән ярлыҡ кардиналь өмөттәр (провайдер/манифест өҫкө сиктәре).

## Мәғлүмәттәр торбаһы

- Коллекторҙар һәр компонент менән бер рәттән йәйелдерелә, OTLP экспортлау I18NT0000000016X (метрика) һәм Локи/Темпо (логтар/эҙҙәр).
- Опциональ eBPF (Тетрагон) шлюздар/төйөндәр өсөн түбән кимәлдәге эҙләүҙе байыта.
- `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` ҡулланыу өсөн I18NT0000000025X һәм индерелгән төйөндәр; оркестрсы I18NI000000351X шылтыратыуын дауам итә.

## Валидация ҡармаҡтары

- CI ваҡытында I18NI0000000352X Run I18NT00000000017X иҫкәртмә ҡағиҙәләрен тәьмин итеү өсөн стойкала стойкала ҡала метрикаһы һәм хосусилыҡты баҫтырыу тикшерелгән.
- I18NT000000019X X версия контроле аҫтында һаҡлау (I18NI000000353X) һәм панелдәр үҙгәргәндә скриншоттар/һылтанмаларҙы яңыртыу.
- Хаос буралар лог һөҙөмтәләре аша I18NI000000354X; валидация рычагтары I18NI000000355X (ҡара: [Операциялар плейбук] (I18NU0000026X)).