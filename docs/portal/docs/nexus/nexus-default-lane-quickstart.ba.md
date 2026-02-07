---
lang: ba
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6120b882618e0f9b6113948d3b12d97e0152a5fc5d4350681ba30aaf114e99d3
source_last_modified: "2026-01-22T14:45:01.354580+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-default-lane-quickstart
title: Default lane quickstart (NX-5)
sidebar_label: Default Lane Quickstart
description: Configure and verify the Nexus default lane fallback so Torii and SDKs can omit lane_id in public lanes.
translator: machine-google-reviewed
---

:::иҫкәртергә канонлы сығанаҡ
Был биттә `docs/source/quickstart/default_lane.md` көҙгөһө. Ике күсермәһен дә һаҡлағыҙ
локализация локализацияһы порталында ерҙәрҙе һуҡҡанға тиклем тура килә.
::: 1990 й.

# Ғәҙәттәгесә Лейн Quickstart (NX-5)

> **Рад картаһы контексы:** NX-5 — йәмәғәт һыҙаты интеграцияһы ғәҙәттәгесә. Хәҙер эшләү ваҡыты
> фашлай I18NI000000015X fallback шулай I18NT000000002X REST/gRPC
> ос нөктәләре һәм һәр SDK хәүефһеҙ ҡалдыра ала I18NI000000016X ҡасан трафик ҡарай
> канон йәмәғәт һыҙатында. Был ҡулланма операторҙар аша йөрөп конфигурациялау
> каталог, тикшерергә fallback I18NI0000000017X, һәм клиент күнекмәләр
> тәртибе тамамлана.

## Алдан шарттар

- I18NI000000018X бинаһының Сора/Nexus бинаһы (I18NI000000019X менән эшләй).
- Конфигурация һаҡлағысына инеү, шулай итеп, һеҙ I18NI000000020X бүлектәрен мөхәррирләй алаһығыҙ.
- `iroha_cli` маҡсатлы кластер менән һөйләшергә конфигурацияланған.
- I18NI0000022223Х (йәки эквивалент) I18NT0000000003X `/status` файҙалы йөкләмәһен тикшерергә.

## 1. Һылтанма һәм мәғлүмәт киңлеге каталогын һүрәтләгеҙ

Селтәрҙә булырға тейешле һыҙаттарҙы һәм мәғлүмәт киңлектәрен иғлан итегеҙ. Фрагмент
аҫта (I18NI000000025X-тан ҡырҡылған) өс йәмәғәт һыҙаты теркәй
плюс тап килгән мәғлүмәттәр киңлеге псевдонимы:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

Һәр `index` уникаль һәм йәнәш булырға тейеш. Dataspace ids 64-битлы ҡиммәттәр;
өҫтәге миҫалдар асыҡлыҡ өсөн һыҙат индекстары менән бер үк һанлы ҡиммәттәрҙе ҡуллана.

## 2. Маршрутлаштырыу ғәҙәттәгесә һәм өҫтәмә өҫтөнлөктәр ҡуйыу

I18NI000000027X бүлеге fallback һыҙаты менән идара итә һәм һеҙгә мөмкинлек бирә
өҫтөнлөк маршрутлаштырыу өсөн аныҡ күрһәтмәләр йәки иҫәп префикстары. Әгәр ҡағиҙәһе юҡ.
матчтар, планлаштырыусы маршруттар транзакция конфигурацияланған I18NI000000028X
һәм `default_dataspace`. Маршрутизатор логикаһы 2018 йылда йәшәй.
I18NI000000030X һәм сәйәсәтте асыҡтан-асыҡ ҡуллана.
Torii REST/gRPC өҫтө.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

Һуңыраҡ яңы һыҙаттар өҫтәгәндә, каталогты тәүҙә яңыртыу, һуңынан маршрутлаштырыуҙы оҙайтығыҙ .
ҡағиҙәләр. Йыйыулы һыҙат йәмәғәт һыҙатына күрһәтелергә тейеш, тип тота

## 3. Сәйәсәт менән төйөн загрузка

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Төйөн стартап ваҡытында алынған маршрутлаштырыу сәйәсәтен теркәй. Теләһә ниндәй раҫлау хаталары
(индекстар, дубликаты псевдоним, дөрөҫ булмаған мәғлүмәттәр киңлеге ids) тиклем .
ғәйбәт башлана.

## 4. Раҫлау һыҙаты менән идара итеү дәүләте

Бер тапҡыр төйөн онлайн, ҡулланыу CLI ярҙамсыһы раҫлау өсөн, тип стандарт һыҙаты .
герметизацияланған (төшөү тейәлгән) һәм трафикҡа әҙер. Йыйынтыҡлы ҡараш бер рәт баҫтыра.
һыҙатҡа:

```bash
iroha_cli app nexus lane-report --summary
``` X

Миҫал сығарыу:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Әгәр ҙә стандарт һыҙат I18NI000000031X күрһәтһә, 2012 йылға тиклем һыҙат идара итеү лауреаты үтә.
тышҡы трафик рөхсәт итеү. I18NI000000032X флагы CI өсөн ҡулайлы.

## 5. I18NT0000000005X статус файҙалы йөкләмәләрен тикшерергә

I18NI000000033X яуап маршрутлаштырыу сәйәсәтен дә, һыҙат буйынса планлаштырыусы ла фашлай.
снимок. Ҡулланыу I18NI0000000034X/I18NI00000000035X раҫлау өсөн конфигурацияланған ғәҙәттәгесә һәм тикшерергә, тип.
fallback һыҙаты телеметрия етештерә:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Өлгө сығыш:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

18NI0000000036X һыҙаты өсөн йәшәү планлаштырыусы иҫәпләүсеһен тикшерергә:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Был раҫлай, тип ТЭУ снимок, псевдоним метамағлүмәт, һәм асыҡ флагтар тура килә
конфигурация менән. Шул уҡ файҙалы йөк нимә I18NT000000000000Х панелдәре өсөн ҡулланыу өсөн
һыҙат-инергән приборҙар таҡтаһы.

## 6. Клиенттарҙы күнегеү ғәҙәттәгесә

- **Раст/CLI.** I18NI0000000037X һәм Rust клиент йәшник I18NI0000000038X яланын үткәрмәй
  ҡасан һеҙ үтмәй I18NI000000039X / I18NI000000040X. Шуға күрә сират маршрутизаторы
  кире I18NI000000041X-ҡа инә. Ҡулланыу асыҡ I18NI000000042X/`--dataspace-id` флагтары
  тик ғәҙәттән тыш булмаған һыҙатҡа йүнәлтелгәндә генә.
- **JS/Swift/Android.** Һуңғы SDK релиздары `laneId`/I18NI0000000045X опциональ булараҡ дауалана.
  һәм I18NI000000046X XX тарафынан рекламаланған ҡиммәткә кире төшә. Маршрутлаштырыу сәйәсәтен 1990 йылда һаҡлағыҙ.
  синхронлаштырыу буйынса сәхнәләштереү һәм етештереү
  үҙгәртеп ҡороуҙар.
- **Пипелин/SSE һынауҙары.** Транзакция ваҡиғаһы фильтрҙары ҡабул итә
  I18NI000000047X предикаттары (ҡара: I18NI000000048X). Яҙылыу өсөн
  I18NI0000000049X менән шул фильтр менән иҫбатлау өсөн, тип яҙа ебәрелгән
  асыҡ һыҙатһыҙ fallback һыҙаты аҫтында килә id.

## 7. Күҙәтеүсәнлек һәм идара итеү ҡармаҡтары

- `/status` шулай уҡ I18NI000000051X һәм
  I18NI000000052X шулай иҫкәртмә ағзаһы иҫкәртә ала, ҡасан да булһа
  һыҙат үҙенең асыҡлығын юғалта. Шул иҫкәртмәләрҙе хатта diventes өсөн мөмкинлек бирҙе.
- Плацер телеметрия картаһы һәм һыҙат идара итеү приборҙар таҡтаһы
  (`dashboards/grafana/nexus_lanes.json`) көтә, псевдоним/слама баҫыуҙары .
  каталог. Әгәр һеҙ псевдоним тип үҙгәртергә, relabel relabel Kura каталогтары шулай
  аудиторҙар детерминистик юлдарҙы һаҡлай (NX-1 аҫтында күҙәтелә).
- Парламент раҫлауҙары өсөн ғәҙәттәгесә һыҙаттар үҙ эсенә алырға тейеш кире ҡайтарыу планы. Яҙымта
  асыҡ хеш һәм идара итеү дәлилдәре менән бер рәттән был тиҙ старт һеҙҙең
  оператор runbook шулай киләсәктә әйләнештәр кәрәкле дәүләтте фаразламай.

Бер тапҡыр был чектар үткән һеҙ дауалай ала I18NI0000000054X тип,
селтәрҙәге юлдарҙы кодлай.