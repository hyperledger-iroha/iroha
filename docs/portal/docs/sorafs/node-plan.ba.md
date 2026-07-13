---
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/node-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8e34d60198b5809cc1a609ccfb27687357b6814acefebbb1f29f328416c16c05
source_last_modified: "2026-07-10T10:11:25+00:00"
translation_last_reviewed: 2026-02-07
id: node-plan
title: SoraFS Node Implementation Plan
sidebar_label: Node Implementation Plan
description: Translate the SF-3 storage roadmap into actionable engineering work with milestones, tasks, and test coverage.
translator: machine-google-reviewed
---

:::иҫкәртергә канонлы сығанаҡ
::: 1990 й.

SF-3 беренсе йүгерә I18NI000000028X йәшник тапшыра, был I18NT000000009X/I18NT000000010X процесын I18NT000000008X һаҡлау провайдерына әйләндерә. Был планды ҡулланыу менән бер рәттән [төйөн һаҡлау етәксеһе](I18NU0000022X), [провайдер ҡабул итеү сәйәсәте](provider-admission-policy.md), һәм [һаҡлау ҡәҙерле баҙар юл картаһы] (storage-capacity-marketplace.md) ҡасан профтационныйҙар.

## Маҡсатлы даирәлә (Майлстоун М1)

1. **Чанк магазин интеграцияһы.** Wrap I18NI000000029X менән ныҡышмалы бэкэнд, тип һаҡлай өлөшө байт, манифест, һәм PoR ағастары конфигурацияланған мәғлүмәт каталогында.
2. **Шлюз ос нөктәләре.** Intion Norito HTTP ос нөктәләре өсөн булавка тапшырыу, өлөшө фетч, PoR үлсәү, һәм һаҡлау телеметрияһы сиктәрендә I18NT000000011X процесы.
**Конфигурация сантехника.** Өҫтәү I18NI0000000000300 X конфигурация структур (эфирлыҡ, ҡәҙерле, каталог, конкурентлыҡ сиктәре) аша проводной I18NI000000031X, I18NI0000000032Х, һәм I18NI00000000333.
4. **Quota/график.** Операция-билдәләнгән диск/параллелизм сиктәре һәм сират запростары менән артҡы баҫым.
5. **Телеметрия.** Эмит метрика/логтар өсөн булавка уңыш, өлөшө fetch латентлыҡ, ҡәҙерле утилизация, һәм PoR үлсәү һөҙөмтәләре.

## Эш өҙөлгән

### А. Йәшник & Модуль структураһы

| Эш | Хужа(тар) | Иҫкәрмәләр |
|------|----------|-------|
| I18NI000000034X модулдәр менән булдырыу: I18NI0000000035X, I18NI0000000036X, I18NI000000037X, I18NI000000000038X, I18NI000000039Х. | Һаҡлау командаһы | Ҡабаттан экспортҡа күп тапҡыр ҡулланыла торған типтары Torii интеграцияһы өсөн. |
| I18NI0000040X тормошҡа ашырыу I18NI000000041X картаһы (ҡулланыусы → фактик → ғәҙәттәгесә). | Һаҡлау командаһы / Конфигурация WG | I18NT000000003X/`iroha_config` ҡатламдарын тәьмин итеү детерминистик булып ҡала. |
| `NodeHandle` фасад I18NT0000000013X ҡулланыу өсөн штекерҙар/фетчтар тапшырырға. | Һаҡлау командаһы | Һаҡлау эске һәм асинк сантехника инкапсулировать. |

### Б. Ваҡытлыса өлөшө магазин

| Эш | Хужа(тар) | Иҫкәрмәләр |
|------|----------|-------|
| Диск бэкэнд уратып төҙөү I18NI0000000044X менән дискта манифест индексы (I18NI0000000045X/I18NI0000000046X). | Һаҡлау командаһы | Детерминистик планировка: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| I18NI000000048X ҡулланып PoR метамағлүмәттәрен һаҡлау (64KiB/4KiB ағастары). | Һаҡлау командаһы | Ярҙам реплей һуң перезапускать; коррупция буйынса тиҙ генә уңышһыҙлыҡҡа осрай. |
| Стартапта тормошҡа ашырыу бөтөнлөгө реплейы (рехаш манифестар, тулы булмаған булавкалар тулы булмаған). | Һаҡлау командаһы | Блок Torii башлана тиклем реплей тамамланған. |

### C. Ҡапҡа нөктәләре

| Endpoint | Behaviour | Tasks |
|----------|-----------|-------|
| `GET /v1/sorafs/pin`, `POST /v1/sorafs/pin/register`, `GET /v1/sorafs/pin/{digest_hex}` | Read the pin registry, register paid manifest pins, and fetch bounded manifest pin details. | Validate chunker profiles, manifest payloads, pin policy, fee receipt context, aliases, and successor links before queueing the signed transaction. |
| `POST /v1/sorafs/storage/pin`, `POST /v1/sorafs/storage/fetch`, `POST /v1/sorafs/storage/token` | Store payload bytes for an approved manifest, fetch content ranges, and issue storage access tokens. | Enforce quotas, token policy, provider capability checks, and scheduler/back-pressure limits. |
| `GET /v1/sorafs/storage/manifest/{manifest_id}`, `GET /v1/sorafs/storage/plan/{manifest_id}`, `GET /v1/sorafs/storage/car/{manifest_id}`, `GET /v1/sorafs/storage/chunk/{manifest_id}/{chunk_digest}` | Serve bounded manifest metadata, deterministic chunk plans, CAR bytes, and individual chunk bytes. | Keep readback arrays bounded while preserving total counts and verify digest/path bindings before streaming bytes. |
| `GET /v1/sorafs/storage/peers`, `GET /v1/sorafs/storage/state`, `POST /v1/sorafs/storage/por-sample` | Report peer/storage state and request bounded local PoR samples. Proof and verdict admission use the authenticated capacity lifecycle; direct storage mutation routes are not mounted. | Reuse chunk-store sampling, update telemetry, and preserve governance-verdict replay state. |


Йүгереп йөрөү ептәре PoR үҙ-ара I18NI000000055X аша: трекер рекордтары һәр I18NI0000000056X, I18NI000000057X, һәм I18NI0000000058X шулай I18NI0000000000059Х метрикаһы идара итеү тураһында хөкөм ҡарары сығара. I18NT0000000015X логикаһы.【крат/сорафтар_төймә/срк/график.р#L147】

Ғәмәлгә ашырыу тураһында иҫкәрмәләр:

- Ҡулланыу I18NT0000000016X’s Axum стека менән I18NI000000060X файҙалы йөкләмәләр.
- Яуаптар өсөн I18NT000000005X схемалары өҫтәй (`PinResultV1`, `FetchErrorV1`, телеметрия структурҙары).

- ✅ I18NI000000063X хәҙер артта ҡалыу тәрәнлеген фашлай плюс иң боронғо эпоха/уҡыу линияһы һәм
  иң һуңғы уңыш/уңышһыҙлыҡ ваҡыт маркалары өсөн һәр провайдер, ҡоролмалары
  I18NI000000064X, һәм I18NT0000000017X
  I18NI0000000065X/I18NI00000000666X өсөн 1990 й. Приборҙар таҡталары.【крат/сорафтар_төймә/src/lib.rs:510】【крат/ироха_тории/срк/сораф/апи.18. 83 шундай

### Д. Расписание & Квота үтәү

| Эш | Ентекле |
|-----|---------|
| Диск квота | Трек байттар диск буйынса; яңы булавкалар кире ҡағыу ҡасан артып I18NI0000000067X. Киләсәктә сәйәсәт өсөн күсерергә ҡармаҡтар бирергә. |
| Фетч конкурентлыҡ | Глобаль семафор (I18NI000000068X) плюс SF-2d диапазоны ҡапҡастарынан алынған провайдер бюджеттары. |
| Пин сираты | Сикләү күренекле ашау эштәре; фашлау I18NT0000000006X статус ос нөктәләре өсөн сират тәрәнлеге. |
| PoR каденцияһы | 18-се һанлы `por_sample_interval_secs` идара иткән фон эшсеһе. |

### E. Телеметрия & Яҡтылыҡ

Метрика (I18NT000000000X):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- I18NI000000072X (гистограмма менән `result` маркалары)
- `torii_sorafs_storage_bytes_used`, I18NI000000075X
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- I18NI000000079X
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Журнал / ваҡиғалар:

- Структуралы I18NT000000007X телеметрияһы өсөн идара итеү ашау (`StorageTelemetryV1`).
- утилләштереү >90% йәки PoR етешһеҙлеге серияһы сигенән артып киткәндә иҫкәртмәләр.

### Ф. Һынау стратегияһы

1. **Берәмек һынауҙары.** Чанк магазин ныҡышмалы, квота иҫәпләүҙәре, график инварианттары (ҡара: I18NI0000083X).  
2. **Интеграция һынауҙары** (`crates/sorafs_node/tests`). Пен → тура юлға килтерергә, тергеҙеү һауығыу, квота кире ҡағыу, PoR үлсәү иҫбатлау раҫлау.  
3. **I18NT0000000018X интеграция һынауҙары.** Run I18NT0000000019X һаҡлау менән мөмкинлек бирҙе, HTTP ос нөктәләре аша I18NI000000085X.  
4. **Хаос юл картаһы.** Киләсәктә диск арыуҙы моделләштерә, яй ИО, провайдерҙы сығарыу.

##

- SF-2b ҡабул итеү сәйәсәте — төйөндәр реклама алдынан ҡабул итеү конверттарын раҫлауҙы тәьмин итеү.  
- SF-2c ҡөҙрәттәре баҙары — телеметрия бәйләүен кире ҡәҙерле декларацияларға.  
- SF-2d реклама оҙайтыуҙары — ҡулланыу диапазоны мөмкинлектәрен ҡулланыу + ағым бюджеттары ҡасандыр бар.

## Мильстоун сығыу критерийҙары

- `cargo run -p sorafs_node --example pin_fetch` урындағы ҡоролмаларға ҡаршы эшләй.  
- Torii I18NI000000087X менән төҙөлә һәм интеграция һынауҙары үтә.  
- Документация ([төйөн һаҡлау буйынса ҡулланма](node-storage.md)) конфигурация ғәҙәттәгесә ғәҙәттәгесә + CLI миҫалдары менән яңыртыла; оператор runbook бар.  
- стадиялау таҡталарында күренгән телеметрия; иҫкәртмәләр ҡәҙерле туйындырыу һәм PoR етешһеҙлектәре өсөн конфигурацияланған.

## Документация & Оптар

- Яңыртыу [төйөн һаҡлау һылтанмаһы](node-storage.md) конфигурация ғәҙәттәгесә, CLI ҡулланыу, һәм проблемаларҙы хәл итеү аҙымдары.  
- [төйөн операциялары runbook] (I18NU000000027X) һаҡлау менән тура килтерелгән тормошҡа ашырыу SF-3 үҫеш.  
- Keep API reference for `/v1/sorafs/pin*` and `/v1/sorafs/storage/*` endpoints aligned with the OpenAPI manifest.