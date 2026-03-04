---
lang: ba
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a37b7ca6ae1aa64e6289ecc44b48ef29c1c884abc039123c1a03b9c35b2e7120
source_last_modified: "2026-01-22T14:35:36.900283+00:00"
translation_last_reviewed: 2026-02-07
id: node-operations
title: Node Operations Runbook
sidebar_label: Node Operations Runbook
description: Validate the embedded `sorafs-node` deployment inside Torii.
translator: machine-google-reviewed
---

:::иҫкәртергә канонлы сығанаҡ
Көҙгөләр `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Ике дана ла релиздар буйынса тура килтереп тотоғоҙ.
::: 1990 й.

## Обзор

Был runbook операторҙары `sorafs-node` встроенный Torii эсендә таратыуҙы раҫлау аша йөрөй. Һәр бүлек туранан-тура SF-3 тапшырыуҙарына картаға төшөрә: булавка/йүнәлештәр, тергеҙеү һауығыу, квота кире ҡағыу, һәм PoR үлсәү.

## 1. Алдан шарттар

- `torii.sorafs.storage`-та һаҡлау эшсеһен бирергә:

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

- Torii процесын тәьмин итеү уҡыу/яҙыу рөхсәт `data_dir`.
- Төйөн раҫлай, көтөлгән ҡөҙрәтте реклама аша `GET /v1/sorafs/capacity/state` аша бер тапҡыр декларация теркәлгән.
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
```

Командалар Norito JSON йомғаҡтары һәм баш тарта өлөшө-профиль йәки диссхенция тап килмәүе, уларҙы файҙалы CI төтөн тикшерергә алдынан Torii проводка.

Бер тапҡыр Torii тура эфирҙа һеҙ HTTP аша шул уҡ артефакттарҙы ала алаһығыҙ:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
``` X

Ике ос нөктәһе лә встраиваемый һаҡлау эшсеһе тарафынан хеҙмәтләндерә, шуға күрә CLI төтөн һынауҙары һәм шлюз зондтары синхронлаша.【крат/ироха_тории/сраф/ап. #L1207】【крат/ироха_тории/срк/sorafs/aporafs/ap.#L1259】

## 2. Пан → Түңәрәк Түңәрәк сәйәхәт

1. Производство + файҙалы йөк өйөмө (мәҫәлән, `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json` менән).
2. Манифестты base64 кодлауы менән тапшырығыҙ:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   JSON үтенесендә `manifest_b64` һәм `payload_b64` булырға тейеш. Уңышлы яуап ҡайтарыу Torii һәм файҙалы йөк һеңдерергә.
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

   База64-decode `data_b64` яланы һәм уны раҫлау өсөн тап килә оригиналь байт.

## 3. Һауыҡтырыу быраулауҙы яңынан башлау

1. Ҡайнатып, кәмендә бер асыҡ өҫтәге кеүек.
2. Torii процесын (йәки бөтә төйөндө) яңынан башлау.
3. Яңынан запросты яңынан тапшырығыҙ. Файҙалы йөк һаман да алыусан булырға тейеш һәм ҡайтарылған дайджер тура килергә тейеш, алдан ҡасыу ҡиммәте.
.

## 4.

.
2. Бер асыҡ күренә; үтенес уңышҡа өлгәшергә тейеш.
. Torii HTP `400` һәм `storage capacity exceeded` булған хата тураһында хәбәр менән запросты кире ҡағырға тейеш.
4. Бөткәс, ғәҙәти ҡөҙрәт сиген тергеҙергә.

## 5. PoR өлгөһө зонд

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

3. Яуапты тикшерергә `samples` менән һоралған иҫәп менән һәм һәр дәлил һаҡланған манифест тамырына ҡаршы раҫлай.

## 6. Автоматлаштырыу ҡармаҡтары

- CI / төтөн һынауҙары өҫтәлгән маҡсатлы тикшерелгәндәрҙе ҡабаттан ҡулланырға мөмкин:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```был `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`, һәм `por_sampling_returns_verified_proofs` ҡаплай.
- Приборҙар таҡталары күҙәтергә тейеш:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` һәм `torii_sorafs_storage_fetch_inflight`.
  - `/v1/sorafs/capacity/state` аша үткән PoR уңыштары/уңышһыҙлыҡҡа осраған иҫәпләүселәр
  - Ҡасаба баҫтырып сығарыу тырышлыҡтары аша `sorafs_node_deal_publish_total{result=success|failure}`

Һуңынан был күнекмәләр тәьмин итә встраиваемый һаҡлау эшсеһе мәғлүмәттәрҙе ашау, йәшәй, ҡабаттан башлау, хөрмәт конфигурацияланған квоталар, һәм генерациялау детерминистик PoR иҫбатлауҙар алдынан төйөн реклама ҡәҙерле киң селтәр.