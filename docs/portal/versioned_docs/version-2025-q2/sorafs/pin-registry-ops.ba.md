---
lang: ba
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0dc64bb4067d734250852a74a65a2100bd68e5ff35f9e8e9dbf3bd2b86f00cfa
source_last_modified: "2026-01-22T14:35:36.898296+00:00"
translation_last_reviewed: 2026-02-07
id: pin-registry-ops-ba
title: Pin Registry Operations
sidebar_label: Pin Registry Operations
description: Monitor and triage the SoraFS pin registry and replication SLA metrics.
translator: machine-google-reviewed
slug: /sorafs/pin-registry-ops-ba
---

:::иҫкәртергә канонлы сығанаҡ
Көҙгөләр `docs/source/sorafs/runbooks/pin_registry_ops.md`. Ике версияны ла релиздар буйынса тура килтереп тотоғоҙ.
::: 1990 й.

## Обзор

Был runbook документы нисек күҙәтеү һәм триаж SoraFS булавка реестры һәм уның репликация хеҙмәте кимәлендә килешеп (SLAs). метрикаһы `iroha_torii`-тан башлана һәм `torii_sorafs_*` исемдәр киңлеге буйынса Prometheus аша экспортлана. Torii өлгөләре реестр хәле 30second интервалында артҡы планда, шуға күрә приборҙар таҡталары ток ҡала, хатта бер ниндәй ҙә операторҙар һорау алыу `/v1/sorafs/pin/*` ос нөктәләре. Импорт курированный приборҙар таҡтаһы (Grafana) әҙер-ҡулланыу өсөн Grafana макеты, тип карталар туранан-тура түбәндәге бүлектәргә.

## метрик һылтанма

| Метрика | Ярлыҡтар | Тасуирлама |
| ----- | ----- | ---------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired` X) | Сылбырҙа йәшәү циклы хәле буйынса инвентаризация асыҡ. |
| `torii_sorafs_registry_aliases_total` | — | Реестрҙа теркәлгән әүҙем асыҡ псевдонимдар һаны. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Репликация тәртибе статус буйынса сегментлы сегмент. |
| `torii_sorafs_replication_backlog_total` | — | Уңайлыҡтар датчигы `pending` заказдарын көҙгөләй. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA буйынса бухгалтерия: `met` һандары срок сиктәрендә заказдарҙы тамамланы, `missed` агрегаттары һуң тамамлау + срогы, `pending` көҙгөһөндәге заказдар. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| Агрегацияланған тамамланыу латентлығы (эмиссия һәм тамамланыу араһындағы эпохалар). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| Көтөү-заказ slack тәҙрәләр (решенсе минус сығарылған эпоха). |

Бөтә датчиктар һәр снимок тартыуҙа сброс, шуға күрә приборҙар таҡталары `1m` каденцияһы йәки тиҙерәк өлгө булырға тейеш.

## Grafana Приборҙар таҡтаһы

Приборҙар таҡтаһы JSON караптары менән ете панелдәр, улар оператор эш ағымын ҡаплай. Һорауҙар түбәндә тиҙ һылтанма өсөн исемлеккә индерелгән, әгәр һеҙ өҫтөнлөк бирәһегеҙ төҙөү өсөн махсус диаграммалар.

1. **Манифест йәшәү циклы** – `torii_sorafs_registry_manifests_total` (`status` менән төркөмләнгән).
2. **Калиса каталог тенденцияһы** – `torii_sorafs_registry_aliases_total`.
3. **Статус буйынса тәртипкә заказ – `torii_sorafs_registry_orders_total` (`status` менән төркөмләнгән).
4. **Берклог vs vs срогы ваҡыты ** – `torii_sorafs_replication_backlog_total` һәм `torii_sorafs_registry_orders_total{status="expired"}` ер өҫтө туйындырыу өсөн берләштерә.
5. **СЛА уңыш нисбәте** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Латенция vs сроклы слак** – ҡаплам `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` һәм `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Ҡулланыу Grafana трансформациялар өҫтәү өсөн `min_over_time` ҡараштары, ҡасан һеҙгә кәрәк абсолют слэк иҙән, мәҫәлән:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Һылтаны бойороҡтар (1h ставкаһы)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## Уғлаһы сиктәре- **SLA уңыш  0**
  - Порог: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0` XX
  - Ғәмәл: тикшерергә идара итеү провайдер churn раҫлау өсөн күренә.
- **Төшөрөү p95 > сроклы ялҡаулыҡ gag**
  - Порог: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Ғәмәл: тикшерергә провайдерҙар йөкләмәләр алдынан сроктар; ҡабаттан заданиеларҙы сығарыуҙы ҡарарға.

### Миҫал Prometheus ҡағиҙәләре

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## Триаж эш ағымы

1. **Сәбәпте билдәләү**
   - Әгәр SLA spike һағынып, ә артта ҡалыу түбән ҡала, провайдер эшмәкәрлегенә иғтибар йүнәлтергә (PoR етешһеҙлектәре, һуң тамамлауҙар).
   - Әгәр ҙә артта ҡалыу тотороҡло мисс менән үҫә, тикшерергә ҡабул итеү (`/v1/sorafs/pin/*`) раҫлау өсөн манифесттар көтә советы раҫлау.
2. **Провайдер статусын раҫлау**
   - Run `iroha app sorafs providers list` һәм реклама мөмкинлектәрен раҫлау репликация талаптарына тап килә.
   - Тикшерергә `torii_sorafs_capacity_*` датчиктар раҫлау өсөн тәьмин ителгән GiB һәм PoR уңыш.
3. **Респумент репликация**
   - Яңы заказдар сығарыу аша `sorafs_manifest_stub capacity replication-order` ҡасан артта ҡалған ялҡаулыҡ (`stat="avg"`) 5 эпоханан түбән төшә (махсус/CAR упаковка ҡулланыу `iroha app sorafs toolkit pack`).
   - Әгәр псевдонимдар әүҙем асыҡтан-асыҡ бәйләүҙәр булмаһа, идара итеүгә хәбәр итегеҙ (`torii_sorafs_registry_aliases_total` көтөлмәгәнсә төшә).
4. **Документ һөҙөмтәһе**
   - SoraFS операциялар журналында ваҡыт маркалары һәм зарарланған асыҡ һеңдереүҙең рекордтары яҙылған.
   - Был runbook-ты яңыртығыҙ, әгәр яңы етешһеҙлектәр режимдары йәки приборҙар таҡталары индерелһә.

## рулет планы

Был стадияланған процедураны үтәргә, ҡасан мөмкинлек бирә йәки тығыҙлау псевдоним кэш сәйәсәте етештереү:1. **Бер конфигурацияны әҙерләү**
   - Яңыртыу `torii.sorafs_alias_cache` `iroha_config` (ҡулланыусы → факты) менән TTLs һәм грация тәҙрәләре: `positive_ttl`, `refresh_window`, `hard_expiry`, Grafana, `revocation_ttl`, `rotation_max_age`, `successor_grace`, һәм `governance_grace`. Ғәҙәттәгесә сәйәсәткә тап килә `docs/source/sorafs_alias_policy.md`.
   - SDKs өсөн, уларҙы конфигурация ҡатламдары аша бер үк ҡиммәттәрҙе бүлергә (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` X-та / NAPI / Python бәйләүҙәре) шулай клиент үтәү шлюзға тап килә.
2. **Ҡоро-йүгереү сәхнәләштереү **
   - Конфиг үҙгәртергә стажировка кластерына йүнәлтергә, был етештереү топологияһын көҙгөләй.
   - Run `cargo xtask sorafs-pin-fixtures` канон псевдонимдарын раҫлау өсөн һаман да расшифровка һәм түңәрәк сәйәхәт; ниндәй ҙә булһа тап килмәү өҫкө ағымдағы манифест дрейфын күҙ уңында тота, уларҙы тәүҙә хәл итергә кәрәк.
   - `/v1/sorafs/pin/{digest}` һәм `/v1/sorafs/aliases` ос нөктәләре менән синтетик дәлилдәр менән яңы, яңыртыу-тәҙрә ҡаплай, һәм ҡаты сирле осраҡтарҙа. HTTP статус кодтарын, башлыҡтарын (`Sora-Proof-Status`, `Retry-After`, `Warning`) һәм JSON корпусына ҡаршы раҫлау.
3. **Производствола **
   - Яңы конфигурацияны стандарт үҙгәрештәр тәҙрәһе аша йәйергә. Ҡулланыу Torii тәүҙә, һуңынан шлюздарҙы ҡабаттан эшләтеп ебәрергә/SDK хеҙмәттәре бер тапҡыр төйөн яңы сәйәсәтте раҫлай журналдарҙа.
   - Import `docs/source/grafana_sorafs_pin_registry.json` Grafana (йәки ғәмәлдәге приборҙар таҡталарын яңыртыу) һәм псевдоним кэш яңыртыу панелдәре NOC эш урыны.
4. **Пост-йөкмәткеһе тикшерелгән**
   - Монитор `torii_sorafs_alias_cache_refresh_total` һәм `torii_sorafs_alias_cache_age_seconds` 30 минут. SoraFS/`expired` ҡойроҡтарында шпайкс сәйәсәтте яңыртыу тәҙрәләре менән корреляцияларға тейеш; көтөлмәгән үҫеш тигәнде аңлата операторҙар тикшерергә тейеш псевдоним һәм провайдер һаулыҡ дауам итеү алдынан.
   - Клиент яғынан логтар раҫлау шул уҡ сәйәси ҡарарҙарҙы күрһәтә (SDKs хаталар өҫтөндә буласаҡ, ҡасан иҫбатлау иҫке йәки срогы үткән). Клиенттар тураһында иҫкәртмәләр булмауы дөрөҫ булмаған конфигурацияны күрһәтә.
5. **Фаллбек**
   - Әгәр псевдонимдар сығарыу артта ҡала һәм яңыртыу тәҙрә сәйәхәттәре йыш, ваҡытлыса сәйәсәтте еңеләйтеү өсөн `refresh_window` һәм `positive_ttl` конфигында, һуңынан үҙгәртеп ҡороу. `hard_expiry` һаҡлау бөтөн, шуға күрә ысын мәғәнәһендә иҫке дәлилдәр һаман да кире ҡағыла.
   - Ҡайтып, алдан конфигурация тергеҙеү аша үткән `iroha_config` снимок, әгәр телеметрия күрһәтеү дауам итә, юғары `error` һандарын күрһәтеү, һуңынан инцидент асыу өсөн псевдоним быуын тотҡарлыҡтар.

## Бәйләнешле материалдар

- `docs/source/sorafs/pin_registry_plan.md` — юл картаһын тормошҡа ашырыу һәм идара итеү контексы.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — һаҡлау эшселәре операциялары, был реестр пьеса китабын тулыландыра.
