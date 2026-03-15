---
lang: ba
direction: ltr
source: docs/source/da/replication_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70163ed6740c80c48c78ae918c37d34e0022ab97ffabce6d451bbf85060e24b4
source_last_modified: "2026-01-22T14:35:37.691616+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Мәғлүмәттәр булыуы репликация сәйәсәте (DA-4)

_Статус: Прогресс — Хужалар: Ядро протоколы WG / Һаҡлау командаһы / SRE_

DA ингестия торбаһы хәҙер детерминистик һаҡлау маҡсаттарын үтәй.
һәр блоб класы һүрәтләнгән `roadmap.md` (эш ағымы DA-4). Torii 2012 йылға баш тарта.
шылтыратыусы-шылтыратыу биргән һаҡлау конверттары, улар тура килмәй конфигурацияланған
сәйәсәт, гарантия бирә, тип, һәр валидатор/һаҡлау төйөндәре кәрәкле һаҡлап ҡала
эпоха һәм репликалар һаны тапшырыусы ниәтенә таянмай.

## Ғәҙәттәгесә сәйәсәт

| Блоб класы | Ҡайнар һаҡлау | Һалҡын һаҡлау | Кәрәкле репликалар | Һаҡлау класы | Идара итеү тегы |
|----------------------------------------------------------------------------- |------------------------ |
| `taikai_segment` | 24 сәғәт | 14 көн | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 сәғәт | 7 көн | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 сәғәт | 180 көн | 3 | `cold` | `da.governance` |
| _Күбәйә (башҡа кластар барыһы ла)_ | 6 сәғәт | 30 көн | 3 | `warm` | `da.default` |

Был ҡиммәттәр `torii.da_ingest.replication_policy` XX һеңдерелгән һәм 2012 йылға ҡулланыла.
бөтә `/v1/da/ingest` тапшырыуҙары. Torii яңынан яҙыуҙар көсләнгән менән күренә
һаҡлау профиле һәм иҫкәртмә сығара, ҡасан шылтыратыусылар тап килмәгән ҡиммәттәр бирә, шулай
операторҙары иҫке СДК-ларҙы асыҡлай ала.

### Тайкай доступность кластары

Тайкай маршрутлаштырыу манифесттары (`taikai.trm` метамағлүмәттәр) хәҙер инә
`availability_class` һиҙеү (`Hot`, `Warm`, йәки `Cold`). Ҡасан бар, Torii
тап килгән һаҡлау профилен һайлай `torii.da_ingest.replication_policy` .
файҙалы йөктө бүлдерер алдынан, ваҡиғалар операторҙары әүҙем булмағанын кәметергә мөмкинлек бирә
глобаль сәйәсәт таблицаһын мөхәррирләүһеҙ рендиялар. Ғәҙәттәгесә:

| Доступность класы | Ҡайнар һаҡлау | Һалҡын һаҡлау | Кәрәкле репликалар | Һаҡлау класы | Идара итеү тегы |
|-------------------- |-------------------------------------------------------------------|-------------------------------леб.
| `hot` | 24 сәғәт | 14 көн | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 сәғәт | 30 көн | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 сәғәт | 180 көн | 3 | `cold` | `da.taikai.archive` |

Әгәр ҙә манифест `availability_class` үткәрһә, ингест юлына кире төшә.
`hot` профиле шулай тура эфирҙа үҙҙәренең тулы реплика комплекты һаҡлай. Операторҙар ала
был ҡиммәттәрҙе яңы мөхәррирләү юлы менән өҫтөнлөк итә
`torii.da_ingest.replication_policy.taikai_availability` блок конфигурацияла.

## Конфигурация

Сәйәсәт `torii.da_ingest.replication_policy` буйынса йәшәй һәм фашлай
* ғәҙәттәгесә * шаблон плюс массив өсөн класлы өҫтөнлөктәре. Класс идентификаторҙары булып тора.
ҡабул итеү һәм ҡабул итеү `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, йәки `custom:<u16>` идара итеү өсөн раҫланған оҙайтыу өсөн.
Һаҡлау дәрестәре `hot`, `warm`, йәки SoraFS ҡабул итә.

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

Ҡалдырырға блок тейелмәгән йүгерергә йүгерергә ғәҙәттәгесә өҫтә күрһәтелгән. Аҙыҡлау өсөн а .
класс, тап килгән өҫтөнлөктө яңыртыу; яңы кластар өсөн база линияһын үҙгәртергә,
`default_retention` мөхәррире.Тәүкәйҙең аныҡ мөмкинлектәрен көйләү өсөн
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "warm"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 14400         # 4 h
cold_retention_secs = 604800       # 7 d
required_replicas = 4
storage_class = "warm"
governance_tag = "da.taikai.warm"
```

## Ҡабул итеү семантикаһы

- Torii ҡулланыусы менән тәьмин ителгән `RetentionPolicy` XX 2-се урынды мәжбүри профиль менән алмаштыра.
  йәки эмиссияны асыҡлағансы йәки асыҡланғансы.
- Алдан төҙөлгән нәтижәләр, улар тап килмәгән һаҡлау профилен иғлан итә, кире ҡағыла
  `400 schema mismatch` менән шулай иҫке клиенттар контрактты көсһөҙләндерә алмай.
- Һәр өҫтөнлөклө сара теркәлә (`blob_class`, көтөлгән сәйәсәт тапшырылған)
  өҫкө ҡатламға ҡаршы тороусан шылтыратыусыларҙы таратыу ваҡытында.

Яңыртылған ҡапҡа өсөн `docs/source/da/ingest_plan.md` (Валидация тикшерелгән исемлеге) ҡарағыҙ
ҡаплау һаҡлау һаҡлау органдары.

## Репликация эш ағымы (ДА-4 күҙәтеү)

Һаҡлау органдары – тәүге аҙым ғына. Операторҙар шулай уҡ иҫбатларға тейеш, тип
йәшәү өсөн нәфис һәм репликация заказдары ҡала менән тура килә конфигурацияланған сәйәсәт шулай
тип SoraFS автоматик рәүештә ҡабаттан ҡабатлау өсөн тыш-ҡаршы таптар.

1. **Дрейф өсөн ҡарау.** Torii X эмиссия
   `overriding DA retention policy to match configured network baseline` 2012 йыл.
   шылтыратыусы иҫке һаҡлау ҡиммәттәрен тапшыра. Пар, тип журнал менән .
   `torii_sorafs_replication_*` телеметрияһы реплика етешһеҙлектәрен күрергә йәки тотҡарланған
   үҙгәртеп ҡороуҙар.
2. **Дифф ниәт vs тере репликалар.** Яңы аудит ярҙамсыһын ҡулланыу:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   Команда йөктәре `torii.da_ingest.replication_policy` бирелгәндән.
   конфиг, һәр манифест (JSON йәки Norito), һәм теләһә ниндәй тап килә
   `ReplicationOrderV1` файҙалы йөктәр асыҡ һеңдерелгән. Йыйынтыҡ флагтар ике
   шарттар:

   - `policy_mismatch` – асыҡ һаҡлау профиле мәжбүриҙән айырыла
     сәйәсәт (был бер ҡасан да булырға тейеш түгел, әгәр Torii дөрөҫ булмаған конфигурацияланмаған).
   - `replica_shortfall` – тура репликация заказы репликаларҙан аҙыраҡ репликаларҙы һорай.
     `RetentionPolicy.required_replicas` йәки уның ҡарағанда аҙыраҡ заданиелар бирә
     маҡсат.

   Нулдән тыш сығыу статусы әүҙем етешһеҙлек күрһәтә, шуға күрә CI/шылтыратыу автоматлаштырыу .
   шунда уҡ бит ала. JSON отчеты менән беркетергә
   Парламент тауыштары өсөн `docs/examples/da_manifest_review_template.md` пакеты.
3. **Триггер ҡабаттан репликация.** Ревизияла етешһеҙлек тураһында хәбәр иткәндә, яңы сығарыла
   `ReplicationOrderV1` аша идара итеү инструменттары 2012 йылда һүрәтләнгән.
   `docs/source/sorafs/storage_capacity_marketplace.md`X һәм яңынан идара итеү
   реплика ҡуйылғанға тиклем йыйыла. Ғәҙәттән тыш хәлдәр өсөн өҫтөнлөктәр өсөн, пар CLI сығыш
   `iroha app da prove-availability` менән, шулай итеп, SREs шул уҡ һеңдерергә һылтанма яһай ала
   һәм ПДП дәлилдәре.

Регрессия яҡтыртыу йәшәй `integration_tests/tests/da/replication_policy.rs`;
люкс тапшыра тап килмәгән һаҡлау сәйәсәте `/v1/da/ingest` һәм раҫлай
тип, ветированный манифест фашлай мәжбүри профиль урынына шылтыратыусы .
ниәт.

## Һаулыҡ һаҡлау телеметрияһы & приборҙар таҡталары (DA-5 күпер)

Юл картаһы әйбер **ДА-5** PDP/PoTR үтәү һөҙөмтәләрен талап итә.
реаль ваҡытта. `SorafsProofHealthAlert` саралары хәҙер 1990 йылдың махсус комплектын йөрөтә.
Prometheus метрикаһы:

- SoraFS.
- `torii_sorafs_proof_health_pdp_failures{provider_id}`
- `torii_sorafs_proof_health_potr_breaches{provider_id}`
- `torii_sorafs_proof_health_penalty_nano{provider_id}`
- `torii_sorafs_proof_health_cooldown{provider_id}`
- `torii_sorafs_proof_health_window_end_epoch{provider_id}`

**SoraFS PDP & PoTR Һаулыҡ** Grafana платаһы
(`dashboards/grafana/sorafs_pdp_potr_health.json`) хәҙер был сигналдарҙы фашлай:- *Дәлил Һаулыҡ тураһында иҫкәртмәләр Trigger* диаграммалары буйынса иҫкәртергә ставкалары буйынса триггер/штраф флагы шулай
  Тайкай/CDN операторҙары иҫбатлай ала, PDP-тик, PoTR-тик, йәки икеләтә забастовкалар
  атыу.
- *Колдаудала тәьмин итеүселәр* хәбәр итә, тура эфирҙа провайдерҙар әлеге ваҡытта аҫтында
  SorafsProofHealthAlert һыуытыу.
- *Дәлил Һаулыҡ тәҙрәһе Снимок* PDP/PoTR иҫәпләүселәрен берләштерә, штраф суммаһы,
  cooldown флагы, һәм забастовка тәҙрәһе эпохаһы бер провайдер шулай идара итеү рецензенттары
  өҫтәлде ваҡиға пакеттарына беркетергә мөмкин.

Ранбуктар был панелдәрҙе бәйләргә тейеш, ҡасан DA үтәү дәлилдәрен тәҡдим итә; улар
бәйләү CLI корпус-ағым етешһеҙлектәре туранан-тура сылбырлы штраф метамағлүмәттәре һәм
юл картаһында саҡырылған күҙәтеүсәнлек ҡармаҡ тәьмин итеү.