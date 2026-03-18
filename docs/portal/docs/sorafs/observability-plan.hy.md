---
lang: hy
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

:::note Կանոնական աղբյուր
:::

## Նպատակներ
- Սահմանեք չափումներ և կառուցվածքային իրադարձություններ դարպասների, հանգույցների և բազմաղբյուր նվագախմբի համար:
- Տրամադրեք Grafana վահանակներ, զգուշացման շեմեր և վավերացման կեռիկներ:
- Սահմանել SLO թիրախները սխալների բյուջետային և քաոսային վարժանքների քաղաքականության հետ մեկտեղ:

## Մետրիկ կատալոգ

### Դարպասների մակերեսներ

| Մետրական | Տեսակ | Պիտակներ | Ծանոթագրություններ |
|--------|------|--------|-------|
| `sorafs_gateway_active` | Չափիչ (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | Թողարկվել է `SorafsGatewayOtel`-ի միջոցով; հետևում է թռիչքի ընթացքում HTTP գործողություններին՝ ըստ վերջնակետի/մեթոդի համակցության: |
| `sorafs_gateway_responses_total` | Հաշվիչ | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, I18NI0000000040X, I18NI0000000040X | Դարպասի յուրաքանչյուր ավարտված հարցումն ավելանում է մեկ անգամ; `result` ∈ {`success`,`error`,`dropped`}: |
| `sorafs_gateway_ttfb_ms_bucket` | Հիստոգրամ | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, I18NI0000000050X, I18NI0000000050X, I18NI0000000050X | Ժամանակից մինչև առաջին բայթ ուշացում դարպասների պատասխանների համար; արտահանվում է որպես Prometheus `_bucket/_sum/_count`: |
| `sorafs_gateway_proof_verifications_total` | Հաշվիչ | `profile_version`, `result`, `error_code` | Ապացույցի ստուգման արդյունքները ֆիքսված են հարցման ժամանակ (`result` ∈ {`success`,`failure`}): |
| `sorafs_gateway_proof_duration_ms_bucket` | Հիստոգրամ | `profile_version`, `result`, `error_code` | Ստուգման հետաձգման բաշխում PoR ստացականների համար: |
| `telemetry::sorafs.gateway.request` | Կառուցվածքային միջոցառում | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Կառուցվածքային մատյան, որը թողարկվում է Loki/Tempo հարաբերակցության յուրաքանչյուր հարցման ավարտի դեպքում: |

`telemetry::sorafs.gateway.request` իրադարձությունները արտացոլում են OTEL հաշվիչները՝ կառուցվածքային օգտակար բեռներով՝ երեսապատելով `endpoint`, `method`, `variant`, I18NI000000080X, I18018NI0000, I18NI000000080X, I1801800000, I18NI000000079X, I18NI000000080X, I1800000000 Loki/Tempo հարաբերակցությունը, մինչդեռ վահանակները սպառում են OTLP շարքը SLO-ի հետևելու համար:

### Ապացուցողական-առողջության հեռաչափություն

| Մետրական | Տեսակ | Պիտակներ | Ծանոթագրություններ |
|--------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Հաշվիչ | `provider_id`, `trigger`, `penalty` | Աճում է ամեն անգամ, երբ `RecordCapacityTelemetry`-ն արտանետում է `SorafsProofHealthAlert`: `trigger`-ը տարբերակում է PDP/PoTR/Երկու ձախողումները, մինչդեռ `penalty`-ը ցույց է տալիս, թե արդյոք գրավն իրականում կրճատվել է, թե ճնշվել է սառեցման արդյունքում: |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Չափիչ | `provider_id` | Վերջին PDP/PoTR հաշվումները, որոնք հաղորդվել են վիրավորական հեռաչափության պատուհանի ներսում, որպեսզի թիմերը կարողանան քանակականացնել, թե որքանով են մատակարարները գերազանցում քաղաքականությունը: |
| `torii_sorafs_proof_health_penalty_nano` | Չափիչ | `provider_id` | Նանո-XOR-ի քանակությունը կրճատվել է վերջին ահազանգի դեպքում (զրո, երբ սառեցումը ճնշել է կիրառումը): |
| `torii_sorafs_proof_health_cooldown` | Չափիչ | `provider_id` | Բուլյան չափիչ (`1` = ծանուցումը ճնշված է սառեցման միջոցով) մակերևույթի վրա, երբ հաջորդող ազդանշանները ժամանակավորապես անջատված են: |
| `torii_sorafs_proof_health_window_end_epoch` | Չափիչ | `provider_id` | Դարաշրջանը գրանցված է հեռուստատեսության պատուհանի համար, որը կապված է ահազանգին, որպեսզի օպերատորները կարողանան փոխկապակցվել Norito արտեֆակտների հետ: |

Այս լրահոսերն այժմ ապահովում են Taikai-ի դիտողների վահանակի առողջության ապացույցների շարքը
(`dashboards/grafana/taikai_viewer.json`)՝ տալով CDN օպերատորներին ուղիղ տեսանելիություն
ազդանշանների ծավալների, PDP/PoTR գործարկման խառնուրդի, տույժերի և սառեցման վիճակի մեջ
մատակարար.

Նույն չափումները այժմ հաստատում են Taikai-ի դիտողների նախազգուշացման երկու կանոնները.
`SorafsProofHealthPenalty`-ը կրակում է ամեն անգամ
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` ավելանում է
վերջին 15 րոպեն, մինչդեռ `SorafsProofHealthCooldown`-ը նախազգուշացում է տալիս, եթե
մատակարարը մնում է սառեցման մեջ հինգ րոպե: Երկու ահազանգերն էլ ապրում են
`dashboards/alerts/taikai_viewer_rules.yml`, որպեսզի SRE-ները ստանան անմիջական համատեքստ
ամեն անգամ, երբ PoR/PoTR-ի կիրառումը սրվում է:

### Նվագախմբի մակերեսներ

| Մետրիկա / Իրադարձություն | Տեսակ | Պիտակներ | Արտադրող | Ծանոթագրություններ |
|----------------|------|--------|----------|-------|
| `sorafs_orchestrator_active_fetches` | Չափիչ | `manifest_id`, `region` | `FetchMetricsCtx` | Նիստերը ներկայումս թռիչքի ընթացքում: |
| `sorafs_orchestrator_fetch_duration_ms` | Հիստոգրամ | `manifest_id`, `region` | `FetchMetricsCtx` | Տևողությունը հիստոգրամը միլիվայրկյաններով; 1ms→30s դույլեր. |
| `sorafs_orchestrator_fetch_failures_total` | Հաշվիչ | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Պատճառները՝ `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`: |
| `sorafs_orchestrator_retries_total` | Հաշվիչ | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Տարբերում է կրկնակի պատճառները (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`): |
| `sorafs_orchestrator_provider_failures_total` | Հաշվիչ | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Նկարում է նիստի մակարդակի հաշմանդամության / ձախողման թվերը: |
| `sorafs_orchestrator_chunk_latency_ms` | Հիստոգրամ | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Մեկ կտորով բեռնման հետաձգման բաշխում (ms) թողունակության/SLO վերլուծության համար: |
| `sorafs_orchestrator_bytes_total` | Հաշվիչ | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Յուրաքանչյուր մանիֆեստի/մատակարարի առաքված բայթեր; ստացեք թողունակությունը `rate()`-ի միջոցով PromQL-ում: |
| `sorafs_orchestrator_stalls_total` | Հաշվիչ | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Հաշվում է `ScoreboardConfig::latency_cap_ms`-ը գերազանցող կտորները: |
| `telemetry::sorafs.fetch.lifecycle` | Կառուցվածքային միջոցառում | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, I18NI000001860X, I18NI000001860X `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Հայելիներ աշխատանքային կյանքի ցիկլը (սկսել/ավարտել) Norito JSON ծանրաբեռնվածությամբ: |
| `telemetry::sorafs.fetch.retry` | Կառուցվածքային միջոցառում | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Թողարկվել է յուրաքանչյուր մատակարարի կրկնակի փորձի շերտի համար; `attempts`-ը հաշվում է լրացուցիչ կրկնվող փորձերը (≥1): |
| `telemetry::sorafs.fetch.provider_failure` | Կառուցվածքային միջոցառում | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Հայտնվել է, երբ մատակարարը հատում է ձախողման շեմը: |
| `telemetry::sorafs.fetch.error` | Կառուցվածքային միջոցառում | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Տերմինալի ձախողման գրառում, որը հարմար է Loki/Splunk-ի կլանման համար: |
| `telemetry::sorafs.fetch.stall` | Կառուցվածքային միջոցառում | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Բարձրացվում է, երբ կտորի հետաձգումը խախտում է կազմաձևված գլխարկը (հայելիները փակցման հաշվիչներն են): |

### Հանգույց / կրկնօրինակման մակերեսներ

| Մետրական | Տեսակ | Պիտակներ | Ծանոթագրություններ |
|--------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Հիստոգրամ | `provider_id` | Պահեստավորման օգտագործման տոկոսի OTEL հիստոգրամ (արտահանված է որպես `_bucket/_sum/_count`): |
| `sorafs_node_por_success_total` | Հաշվիչ | `provider_id` | Միապաղաղ հաշվիչը հաջող PoR նմուշների համար՝ ստացված ժամանակացույցի նկարներից: |
| `sorafs_node_por_failure_total` | Հաշվիչ | `provider_id` | Միապաղաղ հաշվիչը ձախողված PoR նմուշների համար: |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Չափիչ | `provider` | Օգտագործված բայթերի համար գոյություն ունեցող Prometheus չափիչներ, հերթի խորություն, PoR թռիչքների քանակ: |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Չափիչ | `provider` | Մատակարարի հզորության/ժամանակի հաջողության տվյալները հայտնվել են հզորության վահանակում: |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Չափիչ | `provider`, `manifest` | Հետագնացության խորությունը գումարած կուտակային ձախողումների հաշվիչները, որոնք արտահանվում են ամեն անգամ, երբ `/v1/sorafs/por/ingestion/{manifest}` հարցում է կատարվում՝ սնուցելով «PoR Stalls» վահանակը/զգուշացումը: |

### Վերանորոգում և SLA

| Մետրական | Տեսակ | Պիտակներ | Ծանոթագրություններ |
|--------|------|--------|-------|
| `sorafs_repair_tasks_total` | Հաշվիչ | `status` | OTEL հաշվիչ վերանորոգման առաջադրանքների անցումների համար: |
| `sorafs_repair_latency_minutes` | Հիստոգրամ | `outcome` | OTEL հիստոգրամ վերանորոգման կյանքի ցիկլի հետաձգման համար: |
| `sorafs_repair_queue_depth` | Հիստոգրամ | `provider` | OTEL-ի հերթագրված առաջադրանքների հիստոգրամ յուրաքանչյուր մատակարարի համար (snapshot-style): |
| `sorafs_repair_backlog_oldest_age_seconds` | Հիստոգրամ | — | Ամենահին հերթագրված առաջադրանքի տարիքի (վայրկյան) OTEL հիստոգրամ: |
| `sorafs_repair_lease_expired_total` | Հաշվիչ | `outcome` | OTEL հաշվիչ վարձակալության ժամկետի ավարտի համար (`requeued`/`escalated`): |
| `sorafs_repair_slash_proposals_total` | Հաշվիչ | `outcome` | OTEL հաշվիչը կտրատած առաջարկի անցումների համար: |
| `torii_sorafs_repair_tasks_total` | Հաշվիչ | `status` | Prometheus հաշվիչ առաջադրանքների անցման համար: |
| `torii_sorafs_repair_latency_minutes_bucket` | Հիստոգրամ | `outcome` | Prometheus հիստոգրամ՝ վերանորոգման կյանքի ցիկլի հետաձգման համար: |
| `torii_sorafs_repair_queue_depth` | Չափիչ | `provider` | Prometheus չափիչ՝ յուրաքանչյուր մատակարարի համար հերթագրված առաջադրանքների համար: |
| `torii_sorafs_repair_backlog_oldest_age_seconds` | Չափիչ | — | Prometheus չափիչ՝ ամենահին հերթագրված առաջադրանքի տարիքի համար (վայրկյան): |
| `torii_sorafs_repair_lease_expired_total` | Հաշվիչ | `outcome` | Prometheus հաշվիչ վարձակալության ժամկետի ավարտի համար: |
| `torii_sorafs_slash_proposals_total` | Հաշվիչ | `outcome` | Prometheus հաշվիչը կտրատած առաջարկի անցումների համար: |

Կառավարման աուդիտ JSON մետատվյալները արտացոլում են վերանորոգման հեռաչափության պիտակները (`status`, `ticket_id`, `manifest`, `provider` վերանորոգման իրադարձությունների վրա; I18NI000000246X; դետերմինիստորեն.

### Պահպանում և GC| Մետրական | Տեսակ | Պիտակներ | Ծանոթագրություններ |
|--------|------|--------|-------|
| `sorafs_gc_runs_total` | Հաշվիչ | `result` | OTEL հաշվիչ GC ավլումների համար, որոնք արտանետվում են ներկառուցված հանգույցից: |
| `sorafs_gc_evictions_total` | Հաշվիչ | `reason` | OTEL հաշվիչ վտարված մանիֆեստների համար՝ խմբավորված ըստ պատճառի: |
| `sorafs_gc_bytes_freed_total` | Հաշվիչ | `reason` | OTEL հաշվիչ ազատված բայթերի համար՝ խմբավորված ըստ պատճառի: |
| `sorafs_gc_blocked_total` | Հաշվիչ | `reason` | OTEL հաշվիչ վտարումների համար, որոնք արգելափակված են ակտիվ վերանորոգման կամ քաղաքականության պատճառով: |
| `torii_sorafs_gc_runs_total` | Հաշվիչ | `result` | Prometheus հաշվիչ GC ավլումների համար (հաջողություն/սխալ): |
| `torii_sorafs_gc_evictions_total` | Հաշվիչ | `reason` | Prometheus հաշվիչ վտարված մանիֆեստների համար՝ խմբավորված ըստ պատճառի: |
| `torii_sorafs_gc_bytes_freed_total` | Հաշվիչ | `reason` | Prometheus հաշվիչ՝ ըստ պատճառի խմբավորված ազատված բայթերի: |
| `torii_sorafs_gc_blocked_total` | Հաշվիչ | `reason` | Prometheus հաշվիչ արգելափակված վտարումների համար՝ խմբավորված ըստ պատճառի: |
| `torii_sorafs_gc_expired_manifests` | Չափիչ | — | Ժամկետանց մանիֆեստների ընթացիկ քանակությունը դիտվել է GC-ի մաքրման միջոցով: |
| `torii_sorafs_gc_oldest_expired_age_seconds` | Չափիչ | — | Տարիքը ամենահին ժամկետանց մանիֆեստի վայրկյաններով (պահպանման շնորհից հետո): |

### Հաշտեցում

| Մետրական | Տեսակ | Պիտակներ | Ծանոթագրություններ |
|--------|------|--------|-------|
| `sorafs.reconciliation.runs_total` | Հաշվիչ | `result` | OTEL հաշվիչ հաշտեցման լուսանկարների համար: |
| `sorafs.reconciliation.divergence_total` | Հաշվիչ | — | OTEL-ի տարաձայնությունների հաշվիչը մեկ վազքի համար: |
| `torii_sorafs_reconciliation_runs_total` | Հաշվիչ | `result` | Prometheus հաշվիչ հաշտեցման գործարկումների համար: |
| `torii_sorafs_reconciliation_divergence_count` | Չափիչ | — | Հաշտեցման հաշվետվության մեջ նկատված տարաձայնությունների վերջին ցուցանիշը: |

### Ժամանակին առբերման ապացույց (PoTR) և կտոր SLA

| Մետրական | Տեսակ | Պիտակներ | Արտադրող | Ծանոթագրություններ |
|--------|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | Հիստոգրամ | `tier`, `provider` | PoTR համակարգող | Վերջնաժամկետի թուլացում միլիվայրկյաններով (դրական = բավարարված): |
| `sorafs_potr_failures_total` | Հաշվիչ | `tier`, `provider`, `reason` | PoTR համակարգող | Պատճառները՝ `expired`, `missing_proof`, `corrupt_proof`: |
| `sorafs_chunk_sla_violation_total` | Հաշվիչ | `provider`, `manifest_id`, `reason` | SLA մոնիտոր | Գործարկվում է, երբ կտորների առաքումը բաց է թողնում SLO-ն (ուշացում, հաջողության մակարդակ): |
| `sorafs_chunk_sla_violation_active` | Չափիչ | `provider`, `manifest_id` | SLA մոնիտոր | Բուլյան չափիչ (0/1) միացվեց ակտիվ խախտման պատուհանի ժամանակ: |

## SLO թիրախներ

- Դարպասի անվստահության հասանելիություն՝ **99.9%** (HTTP 2xx/304 պատասխաններ):
- Անվստահելի TTFB P95. տաք մակարդակ ≤120 մս, տաք մակարդակ ≤300 մս:
- Ապացույցների հաջողության մակարդակ՝ օրական ≥99,5%:
- Նվագախմբի հաջողությունը (հատվածի ավարտը). ≥99%:

## Վահանակներ և ահազանգեր

1. **Gateway Observability** (`dashboards/grafana/sorafs_gateway_observability.json`) — հետևում է անվստահելի հասանելիությանը, TTFB P95-ին, մերժման խափանումներին և PoR/PoTR խափանումներին OTEL-ի չափումների միջոցով:
2. **Orchestrator Health** (`dashboards/grafana/sorafs_fetch_observability.json`) — ընդգրկում է բազմաթիվ աղբյուրների ծանրաբեռնվածությունը, կրկնվող փորձերը, մատակարարի ձախողումները և խցիկների պայթյունները:
3. **SoraNet Privacy Metrics** (`dashboards/grafana/soranet_privacy_metrics.json`) — գծապատկերներ անանուն ռելեի դույլերի, ճնշող պատուհանների և կոլեկտորի առողջությունը `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` և I18NI0000029 միջոցով:
4. **Կարողությունների առողջություն** (`dashboards/grafana/sorafs_capacity_health.json`) – հետևում է մատակարարի գլխամասային տարածքին և վերանորոգման SLA-ի սրացումներին, վերանորոգման հերթի խորությանը ըստ մատակարարի և GC-ի մաքրման/վտարման/բայթերի ազատված/արգելափակված պատճառների/ժամկետանց դրսևորվող տարիքային և հաշտեցման տարաձայնություններին:

Զգուշացումների փաթեթներ.

- `dashboards/alerts/sorafs_gateway_rules.yml` — դարպասի հասանելիություն, TTFB, անսարքության հաստատման բարձրացումներ:
- `dashboards/alerts/sorafs_fetch_rules.yml` — նվագախմբի ձախողումներ/կրկին փորձեր/կանգառներ; վավերացված `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` և `dashboards/alerts/tests/soranet_policy_rules.test.yml` միջոցով:
- `dashboards/alerts/sorafs_capacity_rules.yml` — հզորության ճնշում, գումարած վերանորոգման SLA/հետավարտ/վարձակալության ժամկետի լրանալու մասին ահազանգեր և GC-ի փակման/արգելափակման/սխալի ազդանշաններ պահպանման մաքրման համար:
- `dashboards/alerts/soranet_privacy_rules.yml` — գաղտնիության նվազման բարձրացումներ, զսպման ահազանգեր, կոլեկցիոների անգործության հայտնաբերում և անջատված կոլեկցիոների ազդանշաններ (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`):
- `dashboards/alerts/soranet_policy_rules.yml` — անանունության խափանման ազդանշաններ՝ միացված `sorafs_orchestrator_brownouts_total`-ին:
- `dashboards/alerts/taikai_viewer_rules.yml` — Taikai դիտողի drift/ingest/CEK հետաձգման ահազանգեր, գումարած նոր SoraFS առողջական տույժի/սառեցման ազդանշանները, որոնք ապահովված են `torii_sorafs_proof_health_*`-ով:

## Հետագծման ռազմավարություն

- Ընդունեք OpenTelemetry-ն ծայրից ծայր.
  - Դարպասներն արտանետում են OTLP տարածություններ (HTTP), որոնք նշում են հարցումների ID-ները, մանիֆեստի ամփոփագրերը և նշանային հեշերը:
  - Նվագավորն օգտագործում է `tracing` + `opentelemetry`՝ բեռնման փորձերի համար ընդգրկույթներ արտահանելու համար:
  - Ներկառուցված SoraFS հանգույցները արտահանում են PoR մարտահրավերների և պահեստավորման գործառնությունների համար: Բոլոր բաղադրիչներն ունեն ընդհանուր հետքի ID, որը տարածվում է `x-sorafs-trace`-ի միջոցով:
- `SorafsFetchOtel`-ը կամրջում է նվագախմբի չափորոշիչները OTLP հիստոգրամների մեջ, մինչդեռ `telemetry::sorafs.fetch.*` իրադարձությունները ապահովում են թեթև JSON ծանրաբեռնվածություն լոգակենտրոն հետնամասերի համար:
- Կոլեկցիոներներ. գործարկել OTEL կոլեկցիոներները Prometheus/Loki/Tempo-ի կողքին (նախընտրելի է Տեմպո): Jaeger API արտահանողները մնում են ընտրովի:
- Բարձր կարդինալության գործառնությունները պետք է ընտրվեն (10% հաջողության ուղիների համար, 100% ձախողումների համար):

## TLS հեռաչափության համակարգում (SF-5b)

- Մետրային հավասարեցում.
  - TLS ավտոմատացումը առաքվում է `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` և `sorafs_gateway_tls_ech_enabled`:
  - Ներառեք այս չափիչները Gateway Overview վահանակում TLS/Certificates վահանակի տակ:
- Զգուշացման կապ.
  - Երբ TLS-ի ժամկետի լրանալու մասին ահազանգերը (մնացել է ≤14 օր) կապված են անվստահելի հասանելիության SLO-ի հետ:
  - ECH-ի անջատումը թողարկում է երկրորդական ահազանգ՝ հղում կատարելով և՛ TLS, և՛ հասանելիության վահանակներին:
- Խողովակաշար. TLS ավտոմատացման աշխատանքը արտահանվում է նույն Prometheus կույտ, որպես դարպասների չափումներ; SF-5b-ի հետ համակարգումը ապահովում է կրկնօրինակված գործիքավորումը:

## Մետրիկ անվանման և պիտակի կոնվենցիաներ

- Մետրիկ անունները հետևում են գոյություն ունեցող `torii_sorafs_*` կամ `sorafs_*` նախածանցներին, որոնք օգտագործվում են Torii-ի և դարպասի կողմից:
- Պիտակների հավաքածուները ստանդարտացված են.
  - `result` → HTTP արդյունք (`success`, `refused`, `failed`):
  - `reason` → մերժման/սխալի կոդը (`unsupported_chunker`, `timeout` և այլն):
  - `status` → վերանորոգման առաջադրանքի վիճակ (`queued`, `in_progress`, `completed`, `failed`, `escalated`):
  - `outcome` → վերանորոգման վարձակալության կամ հետաձգման արդյունք (`requeued`, `escalated`, `completed`, `failed`):
  - `provider` → վեցանկյուն կոդավորված մատակարարի նույնացուցիչ:
  - `manifest` → կանոնական մանիֆեստի մարսողություն (կտրված է բարձր կարդինալության դեպքում):
  - `tier` → դեկլարատիվ մակարդակի պիտակներ (`hot`, `warm`, `archive`):
- Հեռաչափության արտանետման կետեր.
  - Դարպասի չափումները գործում են `torii_sorafs_*`-ի ներքո և վերօգտագործման կոնվենցիաները `crates/iroha_core/src/telemetry.rs`-ից:
  - Նվագախումբը թողարկում է `sorafs_orchestrator_*` չափումներ և `telemetry::sorafs.fetch.*` իրադարձություններ (կյանքի ցիկլ, կրկնակի փորձ, մատակարարի ձախողում, սխալ, փակուղի) հատկորոշված ​​մանիֆեստի ամփոփում, աշխատանքի ID, տարածաշրջան և մատակարարի նույնացուցիչներով:
  - Հանգույցների մակերեսը `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` և `torii_sorafs_por_*`:
- Համակարգեք Observability-ի հետ՝ գրանցելու մետրային կատալոգը համօգտագործվող Prometheus անվանման փաստաթղթում, ներառյալ պիտակի կարդինալության ակնկալիքները (մատակարար/ցուցաբերում է վերին սահմանները):

## Տվյալների խողովակաշար

- Կոլեկցիոներները տեղակայվում են յուրաքանչյուր բաղադրիչի կողքին՝ արտահանելով OTLP դեպի Prometheus (մետրիկա) և Loki/Tempo (տեղեկամատյաններ/հետքեր):
- Ընտրովի eBPF (Tetragon) հարստացնում է ցածր մակարդակի հետագծումը դարպասների/հանգույցների համար:
- Օգտագործեք `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` Torii և ներկառուցված հանգույցների համար; նվագախումբը շարունակում է զանգահարել `install_sorafs_fetch_otlp_exporter`:

## Վավերացման Կեռիկներ

- Գործարկեք `scripts/telemetry/test_sorafs_fetch_alerts.sh`-ը CI-ի ընթացքում՝ ապահովելու համար, որ Prometheus զգուշացման կանոնները մնան կողպեքի չափումների և գաղտնիության սահմանափակման ստուգումների հետ կապված:
- Պահեք Grafana վահանակները տարբերակների հսկողության տակ (`dashboards/grafana/`) և թարմացրեք սքրինշոթները/հղումները, երբ վահանակները փոխվեն:
- Քաոսի վարժանքների գրանցման արդյունքները `scripts/telemetry/log_sorafs_drill.sh`-ի միջոցով; վավերացման լծակներն օգտագործում է `scripts/telemetry/validate_drill_log.sh` (տես [Օպերացիաների գրքույկ](operations-playbook.md)):