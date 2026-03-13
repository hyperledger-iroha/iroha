---
lang: kk
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

:::ескерту Канондық дереккөз
:::

## Мақсаттар
- Шлюздер, түйіндер және көп көзді оркестр үшін көрсеткіштер мен құрылымдық оқиғаларды анықтаңыз.
- Grafana бақылау тақталарын, ескерту шектерін және тексеру ілмектерін қамтамасыз етіңіз.
- Қате-бюджет және хаос-бұрғылау саясаттарымен қатар SLO мақсаттарын белгілеңіз.

## Метрикалық каталог

### Шлюз беттері

| метрикалық | |түрі Белгілер | Ескертпелер |
|--------|------|--------|-------|
| `sorafs_gateway_active` | Өлшеуіш (жоғары төмен санауыш) | `endpoint`, `method`, `variant`, `chunker`, `profile` | `SorafsGatewayOtel` арқылы шығарылады; соңғы нүкте/әдіс комбинациясы бойынша ұшудағы HTTP операцияларын бақылайды. |
| `sorafs_gateway_responses_total` | Есептегіш | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, I18NI000000040X, I18NI00000041002 | Әрбір аяқталған шлюз сұрауы бір рет артады; `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Гистограмма | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, I18NI000000053X, I18NI0000000505 | Шлюз жауаптары үшін бірінші байтқа дейінгі кідіріс уақыты; Prometheus `_bucket/_sum/_count` ретінде экспортталды. |
| `sorafs_gateway_proof_verifications_total` | Есептегіш | `profile_version`, `result`, `error_code` | Сұрау уақытында түсірілген дәлелді тексеру нәтижелері (`result` ∈ {`success`, `failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Гистограмма | `profile_version`, `result`, `error_code` | PoR түбіртектері үшін тексеру кідірісін бөлу. |
| `telemetry::sorafs.gateway.request` | Құрылымдық оқиға | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Loki/Темпо корреляциясына арналған әрбір сұрау аяқталғанда шығарылатын құрылымдық журнал. |

`telemetry::sorafs.gateway.request` events mirror the OTEL counters with structured payloads, surfacing `endpoint`, `method`, `variant`, `status`, `error_code`, and `duration_ms` for Бақылау тақталары SLO бақылауы үшін OTLP сериясын пайдаланған кезде Loki/Темпо корреляциясы.

### Денсаулықты дәлелдейтін телеметрия

| метрикалық | |түрі Белгілер | Ескертпелер |
|--------|------|--------|-------|
| `torii_sorafs_proof_health_alerts_total` | Есептегіш | `provider_id`, `trigger`, `penalty` | `RecordCapacityTelemetry` `SorafsProofHealthAlert` шығарған сайын артады. `trigger` PDP/PoTR/Екі сәтсіздікті ажыратады, ал `penalty` кепілзаттың шын мәнінде қысқартылғанын немесе салқындату арқылы басылғанын анықтайды. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Өлшеуіш | `provider_id` | Ең соңғы PDP/PoTR санаулары бұзылған телеметрия терезесінде хабарланады, осылайша топтар провайдерлердің саясатты қаншалықты асырғанын сандық түрде анықтай алады. |
| `torii_sorafs_proof_health_penalty_nano` | Өлшеуіш | `provider_id` | Соңғы ескертуде Nano-XOR мөлшері қысқартылды (салқындату күшіне енген кезде нөлге тең). |
| `torii_sorafs_proof_health_cooldown` | Өлшеуіш | `provider_id` | Логикалық көрсеткіш (`1` = салқындату арқылы басылған ескерту) кейінгі ескертулердің дыбысы уақытша өшірілгенде пайда болады. |
| `torii_sorafs_proof_health_window_end_epoch` | Өлшеуіш | `provider_id` | Операторлар Norito артефактілерімен корреляция жасай алатындай етіп ескертуге байланыстырылған телеметрия терезесі үшін жазылған дәуір. |

Бұл арналар енді Taikai көру құралының бақылау тақтасының денсаулықты тексеру жолына қуат береді
(`dashboards/grafana/taikai_viewer.json`), CDN операторларына тікелей көріну мүмкіндігін береді
ескерту көлемдеріне, PDP/PoTR триггерлерінің қоспасына, айыппұлдарға және әр адамға арналған салқындату күйіне
провайдер.

Дәл сол көрсеткіштер енді Taikai қарауының екі ескерту ережесін қайтарады:
`SorafsProofHealthPenalty` кез келген уақытта жанады
`torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}` артады
соңғы 15 минут, ал `SorafsProofHealthCooldown` егер
провайдер бес минут бойы салқындату режимінде қалады. Екі ескерту де сақталады
`dashboards/alerts/taikai_viewer_rules.yml`, сондықтан SRE дереу мәтінмәнді алады
PoR/PoTR талаптарын орындау күшейген сайын.

### Оркестр беттері

| Метрика / Оқиға | |түрі Белгілер | Өндіруші | Ескертпелер |
|----------------|------|--------|----------|-------|
| `sorafs_orchestrator_active_fetches` | Өлшеуіш | `manifest_id`, `region` | `FetchMetricsCtx` | Сеанстар қазір ұшуда. |
| `sorafs_orchestrator_fetch_duration_ms` | Гистограмма | `manifest_id`, `region` | `FetchMetricsCtx` | Ұзақтық гистограммасы миллисекундпен; 1мс→30с шелек. |
| `sorafs_orchestrator_fetch_failures_total` | Есептегіш | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Себептер: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Есептегіш | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Қайталау себептерін ажыратады (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Есептегіш | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Сеанс деңгейіндегі өшіру/сәтсіздік көрсеткіштерін жазады. |
| `sorafs_orchestrator_chunk_latency_ms` | Гистограмма | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Өткізу қабілеттілігі/SLO талдауы үшін әр бөлікті алу кідірісін бөлу (мс). |
| `sorafs_orchestrator_bytes_total` | Есептегіш | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Манифестке/провайдерге жеткізілетін байттар; PromQL жүйесінде `rate()` арқылы өткізу қабілеттілігін алыңыз. |
| `sorafs_orchestrator_stalls_total` | Есептегіш | `manifest_id`, `provider_id` | `FetchMetricsCtx` | `ScoreboardConfig::latency_cap_ms` асатын бөліктерді санайды. |
| `telemetry::sorafs.fetch.lifecycle` | Құрылымдық оқиға | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, I18NI0000016010, I18NI0000016160 `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Norito JSON пайдалы жүктемесі бар айна жұмысының өмірлік циклін (бастау/аяқталады). |
| `telemetry::sorafs.fetch.retry` | Құрылымдық оқиға | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | Әрбір провайдердің қайталау сызығына шығарылады; `attempts` қосымша қайталауларды санайды (≥1). |
| `telemetry::sorafs.fetch.provider_failure` | Құрылымдық оқиға | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Провайдер сәтсіздік шегінен өткенде пайда болады. |
| `telemetry::sorafs.fetch.error` | Құрылымдық оқиға | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Loki/Splunk қабылдауға ыңғайлы терминал ақауы туралы жазба. |
| `telemetry::sorafs.fetch.stall` | Құрылымдық оқиға | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Бөлшектердің кешігуі конфигурацияланған қақпақты бұзған кезде көтеріледі (айналар санауыштарды тоқтатады). |

### Түйін / репликация беттері

| метрикалық | |түрі Белгілер | Ескертпелер |
|--------|------|--------|-------|
| `sorafs_node_capacity_utilisation_pct` | Гистограмма | `provider_id` | Жадты пайдалану пайызының OTEL гистограммасы (`_bucket/_sum/_count` ретінде экспортталған). |
| `sorafs_node_por_success_total` | Есептегіш | `provider_id` | Жоспарлағыш суреттерінен алынған сәтті PoR үлгілеріне арналған монотонды есептегіш. |
| `sorafs_node_por_failure_total` | Есептегіш | `provider_id` | Сәтсіз PoR үлгілері үшін монотонды есептегіш. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Өлшеуіш | `provider` | Қолданылған байттар үшін бар Prometheus өлшеуіштері, кезек тереңдігі, PoR рейс санаулары. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Өлшеуіш | `provider` | Провайдердің сыйымдылығы/жұмыс уақыты табысы туралы деректер сыйымдылық бақылау тақтасында пайда болды. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Өлшеуіш | `provider`, `manifest` | `/v2/sorafs/por/ingestion/{manifest}` сұралған сайын экспортталатын, "PoR Stalls" панелін/ескертуін беретін артта қалу тереңдігі және жиынтық ақаулық есептегіштері. |

### Жөндеу және SLA

| метрикалық | |түрі Белгілер | Ескертпелер |
|--------|------|--------|-------|
| `sorafs_repair_tasks_total` | Есептегіш | `status` | Жөндеу тапсырмаларын ауыстыруға арналған OTEL есептегіші. |
| `sorafs_repair_latency_minutes` | Гистограмма | `outcome` | Жөндеу өмірлік циклінің кешігуіне арналған OTEL гистограммасы. |
| `sorafs_repair_queue_depth` | Гистограмма | `provider` | Әр провайдерге кезекте тұрған тапсырмалардың OTEL гистограммасы (сурет стилі). |
| `sorafs_repair_backlog_oldest_age_seconds` | Гистограмма | — | Ең ескі кезекте тұрған тапсырма жасының OTEL гистограммасы (секундтар). |
| `sorafs_repair_lease_expired_total` | Есептегіш | `outcome` | Жалдау мерзімі аяқталатын OTEL есептегіші (`requeued`/`escalated`). |
| `sorafs_repair_slash_proposals_total` | Есептегіш | `outcome` | Ұсыныстың қиғаш өтуіне арналған OTEL есептегіші. |
| `torii_sorafs_repair_tasks_total` | Есептегіш | `status` | Тапсырмаларды ауыстыруға арналған Prometheus есептегіші. |
| `torii_sorafs_repair_latency_minutes_bucket` | Гистограмма | `outcome` | Жөндеу өмірлік циклінің кешігуіне арналған Prometheus гистограммасы. |
| `torii_sorafs_repair_queue_depth` | Өлшеуіш | `provider` | Әр провайдерге кезекте тұрған тапсырмаларға арналған Prometheus көрсеткіші. |
| `torii_sorafs_repair_backlog_oldest_age_seconds` | Өлшеуіш | — | Ең ескі кезекте тұрған тапсырма жасына арналған Prometheus көрсеткіші (секундтар). |
| `torii_sorafs_repair_lease_expired_total` | Есептегіш | `outcome` | Prometheus есептегіші жалдау мерзімі аяқталады. |
| `torii_sorafs_slash_proposals_total` | Есептегіш | `outcome` | Prometheus қиғаш сызық ұсыныс ауысуларына арналған есептегіш. |

Басқару аудиті JSON метадеректері жөндеу телеметрия белгілерін көрсетеді (`status`, `ticket_id`, `manifest`, жөндеу оқиғалары бойынша `provider`; `outcome` slash correlics және slash correlics бойынша аудиттер) анықтаушы түрде.

### Сақтау және GC| метрикалық | |түрі Белгілер | Ескертпелер |
|--------|------|--------|-------|
| `sorafs_gc_runs_total` | Есептегіш | `result` | Енгізілген түйін арқылы шығарылатын GC сыпыруға арналған OTEL есептегіші. |
| `sorafs_gc_evictions_total` | Есептегіш | `reason` | Себеп бойынша топтастырылған шығарылған манифесттерге арналған OTEL есептегіші. |
| `sorafs_gc_bytes_freed_total` | Есептегіш | `reason` | Себеп бойынша топтастырылған босатылған байттар үшін OTEL есептегіші. |
| `sorafs_gc_blocked_total` | Есептегіш | `reason` | Белсенді жөндеу немесе саясатпен блокталған көшірулерге арналған OTEL есептегіші. |
| `torii_sorafs_gc_runs_total` | Есептегіш | `result` | GC тазалауға арналған Prometheus есептегіші (сәттілік/қате). |
| `torii_sorafs_gc_evictions_total` | Есептегіш | `reason` | Prometheus себебі бойынша топтастырылған шығарылған манифесттерге арналған есептегіш. |
| `torii_sorafs_gc_bytes_freed_total` | Есептегіш | `reason` | Себеп бойынша топтастырылған босатылған байттар үшін Prometheus есептегіш. |
| `torii_sorafs_gc_blocked_total` | Есептегіш | `reason` | Prometheus себебі бойынша топтастырылған блокталған көшірулерге арналған есептегіш. |
| `torii_sorafs_gc_expired_manifests` | Өлшеуіш | — | Мерзімі өткен манифесттердің ағымдағы саны МК сыпырушылары бақылайды. |
| `torii_sorafs_gc_oldest_expired_age_seconds` | Өлшеуіш | — | Мерзімі өткен ең ескі манифесттің секундтарымен жасы (сақтаудан кейін). |

### Татуласу

| метрикалық | |түрі Белгілер | Ескертпелер |
|--------|------|--------|-------|
| `sorafs.reconciliation.runs_total` | Есептегіш | `result` | Салыстыру суреттеріне арналған OTEL есептегіші. |
| `sorafs.reconciliation.divergence_total` | Есептегіш | — | OTEL әр жүгірістегі дивергенция санаушысы. |
| `torii_sorafs_reconciliation_runs_total` | Есептегіш | `result` | Prometheus салыстыру орындалу үшін есептегіш. |
| `torii_sorafs_reconciliation_divergence_count` | Өлшеуіш | — | Салыстыру есебінде байқалған соңғы айырмашылықтар саны. |

### Уақтылы іздеу (PoTR) және SLA бөлігінің дәлелі

| метрикалық | |түрі Белгілер | Өндіруші | Ескертпелер |
|--------|------|--------|----------|-------|
| `sorafs_potr_deadline_ms` | Гистограмма | `tier`, `provider` | PoTR үйлестірушісі | Миллисекундтағы соңғы мерзімнің әлсіреуі (оң = орындалды). |
| `sorafs_potr_failures_total` | Есептегіш | `tier`, `provider`, `reason` | PoTR үйлестірушісі | Себептер: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Есептегіш | `provider`, `manifest_id`, `reason` | SLA мониторы | Бөлшектерді жеткізу SLO (кідіріс, сәттілік жылдамдығы) өткізіп алған кезде іске қосылады. |
| `sorafs_chunk_sla_violation_active` | Өлшеуіш | `provider`, `manifest_id` | SLA мониторы | Логикалық көрсеткіш (0/1) белсенді бұзу терезесі кезінде ауыстырылды. |

## SLO мақсаттары

- Шлюздің сенімсіз қолжетімділігі: **99,9%** (HTTP 2xx/304 жауаптары).
- Сенімсіз TTFB P95: ыстық деңгей ≤120 мс, жылы деңгей ≤300 мс.
- Дәлелдеу сәттілігі: тәулігіне ≥99,5%.
- Оркестрдің жетістігі (бөлшектерді аяқтау): ≥99%.

## Бақылау тақталары және ескертулер

1. **Gateway Observability** (`dashboards/grafana/sorafs_gateway_observability.json`) — OTEL көрсеткіштері арқылы сенімсіз қолжетімділікті, TTFB P95, бас тартудың бұзылуын және PoR/PoTR қателерін қадағалайды.
2. **Orchestrator Health** (`dashboards/grafana/sorafs_fetch_observability.json`) — көп көзді жүктеуді, қайталауларды, провайдердің қателерін және тоқтап қалуларды қамтиды.
3. **SoraNet Privacy Metrics** (`dashboards/grafana/soranet_privacy_metrics.json`) — `soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled` және I18NI0000292X және I18NI000002 арқылы анонимделген релелік шелектерді, басу терезелерін және коллектор денсаулығын диаграммалар.
4. **Capacity Health** (`dashboards/grafana/sorafs_capacity_health.json`) — провайдердің бос орынын және жөндеу SLA ұлғаюын, провайдер бойынша жөндеу кезек тереңдігін және GC тазалау/шығару/босатылған/бұғатталған себептер/мерзімі өткен манифест жасы мен келісу алшақтықтары суреттерін қадағалайды.

Ескерту жинақтары:

- `dashboards/alerts/sorafs_gateway_rules.yml` — шлюздің қолжетімділігі, TTFB, сәтсіздікті растау.
- `dashboards/alerts/sorafs_fetch_rules.yml` — оркестрдің ақаулары/қайталануы/тұрақтары; `scripts/telemetry/test_sorafs_fetch_alerts.sh`, `dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/soranet_privacy_rules.test.yml` және `dashboards/alerts/tests/soranet_policy_rules.test.yml` арқылы расталған.
- `dashboards/alerts/sorafs_capacity_rules.yml` — сыйымдылық қысымы плюс жөндеу SLA/қатар/жалдау мерзімінің аяқталуы туралы ескертулер және сақтауды сыпыру үшін GC тоқтауы/бұғатталған/қате туралы ескертулер.
- `dashboards/alerts/soranet_privacy_rules.yml` — құпиялылық деңгейін төмендету жоғарылатулары, басу дабылдары, коллектордың бос тұруын анықтау және өшірілген коллектор туралы ескертулер (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — `sorafs_orchestrator_brownouts_total` сымына қосылған анонимді өшіру дабылдары.
- `dashboards/alerts/taikai_viewer_rules.yml` — Taikai қарау құралының дрейф/инест/CEK кідіріс дабылдары және `torii_sorafs_proof_health_*` арқылы жұмыс істейтін жаңа SoraFS денсаулықты тексеретін айыппұл/салқындату туралы ескертулер.

## Бақылау стратегиясы

- OpenTelemetry-ді соңына дейін қабылдаңыз:
  - Шлюздер сұрау идентификаторлары, манифест дайджесттері және таңбалауыш хэштері бар аннотацияланған OTLP аралықтарын (HTTP) шығарады.
  - Оркестр алу әрекеттері үшін аралықты экспорттау үшін `tracing` + `opentelemetry` пайдаланады.
  - Енгізілген SoraFS түйіндерінің PoR проблемалары мен сақтау операциялары үшін экспорт аралығы. Барлық компоненттер `x-sorafs-trace` арқылы таралатын ортақ бақылау идентификаторын бөліседі.
- `SorafsFetchOtel` оркестр көрсеткіштерін OTLP гистограммаларына көпірлейді, ал `telemetry::sorafs.fetch.*` оқиғалары журналға бағытталған серверлер үшін жеңіл JSON пайдалы жүктемелерін қамтамасыз етеді.
- Коллекторлар: OTEL коллекторларын Prometheus/Loki/Tempo (Темпо қолайлы) қатарында іске қосыңыз. Jaeger API экспорттаушылары міндетті емес болып қалады.
- Кардиналдылығы жоғары операцияларды іріктеу керек (сәтті жолдар үшін 10%, сәтсіздіктер үшін 100%).

## TLS телеметриялық үйлестіру (SF-5b)

- Метрикалық туралау:
  - TLS автоматтандыруы `sorafs_gateway_tls_cert_expiry_seconds`, `sorafs_gateway_tls_renewal_total{result}` және `sorafs_gateway_tls_ech_enabled` жібереді.
  - Бұл өлшеуіштерді TLS/Сертификаттар тақтасының астындағы шлюзге шолу бақылау тақтасына қосыңыз.
- Ескерту байланысы:
  - TLS мерзімінің аяқталу туралы ескертулері өрт (≤14 күн қалды) SLO сенімсіз қолжетімділігімен байланысты.
  - ECH өшіру TLS және қолжетімділік тақталарына сілтеме жасайтын қосымша ескертуді шығарады.
- Құбыр: TLS автоматтандыру тапсырмасы шлюз метрикасымен бірдей Prometheus стекіне экспортталады; SF-5b көмегімен үйлестіру қайталанатын аспаптарды қамтамасыз етеді.

## Метрикалық атау және белгі конвенциялары

- Көрсеткіш атаулары Torii және шлюз пайдаланатын бар `torii_sorafs_*` немесе `sorafs_*` префикстеріне сәйкес келеді.
- Жапсырмалар жиынтығы стандартталған:
  - `result` → HTTP нәтижесі (`success`, `refused`, `failed`).
  - `reason` → бас тарту/қате коды (`unsupported_chunker`, `timeout`, т.б.).
  - `status` → жөндеу тапсырмасының күйі (`queued`, `in_progress`, `completed`, `failed`, `escalated`).
  - `outcome` → жөндеуді жалға алу немесе кешіктіру нәтижесі (`requeued`, `escalated`, `completed`, `failed`).
  - `provider` → он алтылық кодталған провайдер идентификаторы.
  - `manifest` → канондық манифест дайджесті (кардиналдылығы жоғары болған кезде кесілген).
  - `tier` → декларативті деңгейдегі белгілер (`hot`, `warm`, `archive`).
- телеметриялық сәуле шығару нүктелері:
  - Шлюз көрсеткіштері `torii_sorafs_*` астында өмір сүреді және `crates/iroha_core/src/telemetry.rs` конвенцияларын қайта пайдалану.
  - Оркестр манифест дайджестімен, тапсырма идентификаторымен, аймақпен және провайдер идентификаторларымен тегтелген `sorafs_orchestrator_*` метрикасын және `telemetry::sorafs.fetch.*` оқиғаларын (өмір циклі, қайталау, провайдер қатесі, қате, тоқтау) шығарады.
  - `torii_sorafs_storage_*`, `torii_sorafs_capacity_*` және `torii_sorafs_por_*` түйіндерінің беті.
- Көрсеткіштік каталогты ортақ Prometheus атау құжатында тіркеу үшін Бақылау мүмкіндігімен үйлестіріңіз, соның ішінде жапсырманың негізгі күтулері (провайдер/манифест жоғарғы шекаралары).

## Деректер құбыры

- Коллекторлар OTLP-ті Prometheus (метрика) және Loki/Tempo (журналдар/іздер) түріне экспорттай отырып, әрбір құрамдаспен бірге орналастырады.
- Қосымша eBPF (Тетрагон) шлюздер/түйіндер үшін төмен деңгейлі бақылауды байытады.
- Torii және ендірілген түйіндер үшін `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` пайдаланыңыз; оркестр `install_sorafs_fetch_otlp_exporter` қоңырауын жалғастырады.

## Тексеру ілмектері

- Prometheus ескерту ережелерінің тоқтау көрсеткіштері мен құпиялылықты басу тексерулерімен құлыптаулы күйде қалуын қамтамасыз ету үшін CI кезінде `scripts/telemetry/test_sorafs_fetch_alerts.sh` іске қосыңыз.
- Grafana бақылау тақталарын нұсқа бақылауында ұстаңыз (`dashboards/grafana/`) және панельдер өзгерген кезде скриншоттарды/сілтемелерді жаңартыңыз.
- Хаос журнал нәтижелерін `scripts/telemetry/log_sorafs_drill.sh` арқылы жасайды; валидация рычагтары `scripts/telemetry/validate_drill_log.sh` ([Operations Playbook](operations-playbook.md) қараңыз).