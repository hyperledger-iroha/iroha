---
id: node-operations
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/node-operations.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Node Operations Runbook
sidebar_label: Node Operations Runbook
description: Validate the embedded `sorafs-node` deployment inside Torii.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::иҫкәртергә канонлы сығанаҡ
Көҙгөләр I18NI000000018X. Ике версияны ла синхронлаштырыуға тиклем Сфинкс комплекты пенсияға сыҡҡансы һаҡлағыҙ.
::: 1990 й.

## Обзор

Был runbook операторҙары аша раҫланған I18NI0000000019X Xnation I18NT000000001X эсендә раҫлау. Һәр бүлек туранан-тура SF-3 тапшырыуҙарына картаға төшөрә: булавка/йүнәлештәр, тергеҙеү һауығыу, квота кире ҡағыу, һәм PoR үлсәү.

## 1. Алдан шарттар

- I18NI000000020X-та һаҡлау эшсеһен бирергә:

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- Torii процесын тәьмин итеү уҡыу/яҙыу рөхсәт I18NI000000021X.
- Төйөн раҫлай, көтөлгән ҡөҙрәтте I18NI000000022X аша иғлан итеү бер тапҡыр декларация теркәлгән.
- Ҡасан тигеҙләү өҫтөндә эшләй, приборҙар таҡталары сеймал һәм тигеҙләнгән GiB·our/PoR иҫәпләүселәрҙе фашлау өсөн дрожь-бушлай тенденцияларҙы айырып күрһәтеү өсөн тап ҡиммәттәре.

### CLI ҡоро йүгерә (Һоралы)

HTTP ос нөктәләрен фашлау алдынан һеҙ аҡыл-тикшерергә мөмкин һаҡлау бэкэнд менән өйөлгән CLI.【крат/sorafs_node/src/bin/sorafs-node.rs#L1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
``` X

Командалар I18NNT00000000000 JSON йомғаҡтары һәм баш тарта өлөшө-профиль йәки диссхенция тап килмәүе, уларҙы файҙалы CI төтөн тикшерергә алдынан I18NT0000000000003X проводка.

### PoR иҫбатлау репетицияһы

Операторҙар хәҙер идара итеүҙе реплей-приповать PoR артефакттар урындағы уларҙы тейәп, уларҙы I18NT0000000004X. CLI шул уҡ `sorafs-node` ингестия юлын ҡабаттан ҡуллана, шуға күрә локаль йүгерә теүәл раҫлау хаталары, тип HTTP API ҡайтарыласаҡ.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

Команда JSON резюмеһы сығара (төшөү өсөн һеңдерелгән, провайдер id, иҫбатлау disist, өлгө иҫәбе, факультатив хөкөм һөҙөмтәһе). `--manifest-id=<hex>` тәьмин итеү өсөн һаҡланған манифест тап килә һынау үҙләштереү, һәм I18NI000000025X ҡасан һеҙ теләйһегеҙ, архив резюме менән оригиналь артефакттар өсөн аудит дәлилдәре. Шул иҫәптән I18NI000000026X һеҙгә бөтә һынауҙы репетициялау мөмкинлеге бирә → иҫбатлау → офлайн офлайн HTTP API шылтыратыу алдынан.

Бер тапҡыр I18NT0000000005X тура эфирҙа һеҙ HTTP аша шул уҡ артефакттарҙы ала алаһығыҙ:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Ике ос нөктәһе лә встраиваемый һаҡлау эшсеһе тарафынан хеҙмәтләндерә, шуға күрә CLI төтөн һынауҙары һәм шлюз зондтары синхронлаша.【крат/ироха_тории/сраф/ап. #L1207】【крат/ироха_тории/срк/sorafs/aporafs/ap.#L1259】

## 2. Пан → Түңәрәк Түңәрәк сәйәхәт

1. Манифест + файҙалы йөк өйөмөн эшләгеҙ (мәҫәлән, `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json` менән).
2. Манифестты base64 кодлауы менән тапшырығыҙ:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   JSON үтенесендә `manifest_b64` һәм I18NI000000029X XX. Уңышлы яуап ҡайтарыу I18NI00000000300Х һәм файҙалы йөк һеңдерергә.
3. Ҡырҡылған мәғлүмәттәрҙе ала:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   База64-decode I18NI000000031X яланында һәм уны раҫлау өсөн тап килә оригиналь байт.

## 3. Һауыҡтырыу быраулауҙы яңынан башлау

1. Ҡайнатып, кәмендә бер асыҡ өҫтәге кеүек.
2. I18NT000000006X процесы (йәки бөтә төйөн) яңынан эшләтеп ебәрегеҙ.
3. Яңынан запросты яңынан тапшырығыҙ. Файҙалы йөк һаман да алыусан булырға тейеш һәм ҡайтарылған дайджер тура килергә тейеш, алдан ҡасыу ҡиммәте.
.

## 4.

1. Ваҡытлыса түбәнерәк `torii.sorafs.storage.max_capacity_bytes` бәләкәй ҡиммәткә тиклем (мәҫәлән, бер манифест ҙурлығы).
2. Бер асыҡ күренә; үтенес уңышҡа өлгәшергә тейеш.
. I18NT000000007X HTP I18NI0000000035X һәм I18NI000000036X XX Хата тураһында хәбәр менән запросты кире ҡағырға тейеш.
4. Бөткәс, ғәҙәти ҡөҙрәт сиген тергеҙергә.

## 5. Һаҡлау / GC тикшерергә (Уҡыу ғына)

1. Урындағы һаҡлау сканерлауын һаҡлау каталогына ҡаршы йүгерергә:

   ```bash
   iroha app sorafs gc inspect --data-dir ./storage/sorafs
   ```

2. Тикшерелеү генә тамамланған сағылыш таба (Ҡоро йүгерә генә, юйыу юҡ):

   ```bash
   iroha app sorafs gc dry-run --data-dir ./storage/sorafs
   ```

3. I18NI000000037X йәки I18NI000000038X ҡулланыу, баһалау тәҙрәһен ҡаҙаҡлау өсөн, ҡасан сағыштырыу отчеттар буйынса хужалар йәки инциденттар.

ГК CLI аңлы рәүештә уҡыла-тик. Уны ҡулланыу өсөн тотоу сроктары һәм срогы-махсус инвентаризация өсөн аудит юлдары; етештереүҙә мәғлүмәттәрҙе ҡул менән алып ташламағыҙ.

## 6. PoR өлгөһө зонд

1. Аңлайышлы итеп ҡабартығыҙ.
2. PoR өлгөһөн һорағыҙ:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Яуапты тикшерергә I18NI000000039X менән һоралған иҫәп менән һәм һәр дәлил һаҡланған манифест тамырына ҡаршы раҫлай.

## 7. Автоматлаштырыу ҡармаҡтары

- CI / төтөн һынауҙары өҫтәлгән маҡсатлы тикшерелгәндәрҙе ҡабаттан ҡулланырға мөмкин:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  был `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`, һәм `por_sampling_returns_verified_proofs` ҡаплай.
- Приборҙар таҡталары күҙәтергә тейеш:
  - I18NI000000044Х.
  - I18NI000000045X һәм I18NI000000046X
  - I18NI000000047X аша үткән PoR уңыш/уңышһыҙлыҡҡа осраған иҫәпләүселәр
  - Ҡасаба баҫтырып сығарыу тырышлыҡтары аша I18NI00000000048X

Һуңынан был күнекмәләр тәьмин итә встроенный һаҡлау эшсеһе мәғлүмәттәр ашау, йәшәү өсөн перерасписание, хөрмәт конфигурацияланған квоталар, һәм генерациялау детерминистик PoR иҫбатлауҙар алдынан төйөн реклама ҡәҙерен киң селтәр.