---
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a117889e81f876c00129ade76a9a04aa39181add2378ef5c19110b7be30f9d6f
source_last_modified: "2026-01-05T09:28:11.859335+00:00"
translation_last_reviewed: 2026-02-07
id: chunker-registry-rollout-checklist
title: SoraFS Chunker Registry Rollout Checklist
sidebar_label: Chunker Rollout Checklist
description: Step-by-step rollout plan for chunker registry updates.
translator: machine-google-reviewed
---

:::иҫкәртергә канонлы сығанаҡ
::: 1990 й.

# I18NT000000000X теркәү исемлеге

Был тикшерелгән исемлек яңы chunker профилен пропагандалау өсөн кәрәкле аҙымдарҙы тота йәки
провайдер ҡабул итеү өйөмө тикшерелгәндән һуң идара итеүҙән һуң
устав ратификацияланған.

> **Сик:** Үҙгәргән бөтә релиздарға ла ҡағыла.
> `sorafs_manifest::chunker_registry`, провайдер ҡабул итеү конверттары, йәки был
> канонлы ҡоролма өйөмдәре (`fixtures/sorafs_chunker/*`).

## 1. Осоу алдынан раҫлау

1. Предприятиеларҙы тергеҙергә һәм детерминизмды раҫлау:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Детерминизм хештарын раҫлау.
   `docs/source/sorafs/reports/sf1_determinism.md` (йәки тейешле профиль
   отчет) регенерацияланған артефакттарға тап килә.
.
   `ensure_charter_compliance()` йүгерә:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Тәҡдимде яңыртыу досье:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Совет протоколдары I18NI000000015X буйынса инеү
   - Детерминизм отчеты

## 2. Идара итеү

1. Ҡоролма эшсе төркөмө отчеты һәм тәҡдим үҙләштереү тәҡдим Сора
   Парламент инфраструктура панелендә.
2. 2012 йылда раҫлау реквизиттарын яҙып алыу.
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Парламент-ҡул ҡуйылған конвертты ҡоролмалар менән бер рәттән баҫтырып сығарыу:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Конвертты тикшерергә идара итеү аша ярҙам итеүсе ярҙам итеүсе:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3.

Һылтанма [стажировка манифест плейбук] (./staging-manifest-playbook) өсөн
ентекле проходка был аҙымдар.

2
   үтәлеше башланды (I18NI000000019X).
2. Ҡабул ителгән провайдер ҡабул итеү конверттарын сәхнәләштереү реестрына этәрергә
   каталогы I18NI000000020X тарафынан һылтанма яһаны.
3. Тикшерергә провайдер реклама аша таралыу асыш API:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Күнекмәләр манифест/план остары менән идара итеү башлыҡтары:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Телеметрия таҡталарын раҫлау (`torii_sorafs_*`) һәм иҫкәртмә ҡағиҙәләре тураһында хәбәр итә
   яңы профиль хатаһыҙ.

## 4. Етештереү рулеты

1. I18NT000000002X төйөндәренә ҡаршы сәхнәләштереү аҙымдарын ҡабатлау.
2. Иғлан активация тәҙрәһе (дата/ваҡыт, льгота осоро, кире ҡайтарыу планы)
   операторы һәм SDK каналдары.
3. ПР-ҙы үҙ эсенә алған сығарыуҙы берләштерегеҙ:
   - Яңыртылған ҡорамалдар һәм конверт
   - Документация үҙгәрештәре ( устав һылтанмалары, детерминизм отчеты)
   - Юл картаһы/статус яңыртыу
4. Реклавканы билдәләгеҙ һәм провенанс өсөн ҡул ҡуйылған артефакттарҙы архивлау.

## 5.

1. Һуңғы метрикаларҙы тотоу (асыу иҫәптәре, уңыш ставкаһы, хата
   гистограммалар) 24 сәғәт йәйелгәндән һуң.
2. Яңыртыу I18NI000000022 X ҡыҫҡаса резюме һәм һылтанма менән детерминизм отчеты.
3. Файл теләһә ниндәй эҙмә-эҙлекле бурыстар (мәҫәлән, өҫтәмә профиль авторлыҡ етәкселеге)
   `roadmap.md`.