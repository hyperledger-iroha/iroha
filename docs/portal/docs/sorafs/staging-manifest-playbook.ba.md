---
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 768bcb70ff95d1445e6bd02a3f255ff2272a7796cc32d94f52abf99971b8dc7a
source_last_modified: "2026-01-05T09:28:11.910212+00:00"
translation_last_reviewed: 2026-02-07
id: staging-manifest-playbook
title: Staging Manifest Playbook
sidebar_label: Staging Manifest Playbook
description: Checklist for enabling the Parliament-ratified chunker profile on staging Torii deployments.
translator: machine-google-reviewed
---

:::иҫкәртергә канонлы сығанаҡ
::: 1990 й.

## Обзор

Был playbook аша йөрөп мөмкинлек бирә Парламент-ратифицированный chunker профиле стажировка I18NT00000000004X таратыу алдынан үҙгәрештәрҙе пропагандалау етештереүгә булышлыҡ итеү. Ул SoraFS идара итеү уставын ратификацияланған тип фаразлай һәм канон ҡорамалдары һаҡлағыста бар.

## 1. Алдан шарттар

1. Каноник ҡоролмаларын һәм ҡултамғаларын синхронлаштырыу:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Ҡабул итеү конверт каталогы әҙерләй, тип I18NT000000005X стартапта уҡый (миҫал юл): `/var/lib/iroha/admission/sorafs`.
3. I18NT000000006X конфигын тәьмин итеү асыш кэш һәм ҡабул итеү органдарына мөмкинлек бирә:

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ``` X

## 2. Баҫма ҡабул итеү конверттары

1. Copy раҫланған провайдер ҡабул итеү конверттары каталогҡа һылтанма I18NI0000000017X:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

.
3. Ҡабул итеү тураһында хәбәрҙәр өсөн журналдарҙы ҡойроҡ:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Асыш итеүҙе раҫлаусы таралыу

1. Ҡул ҡуйылған провайдер реклама файҙалы йөк (I18NT00000000001X байт) һеҙҙең етештереү
   провайдер торбаһы:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. Асыу ос нөктәһен һорап, рекламаның канонлы псевдонимдар менән барлыҡҡа килгәнен раҫлау:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   `profile_aliases` XI18NI000000019X тәүге яҙма булараҡ үҙ эсенә ала.

## 4. Күнекмәләр манифест & план остары .

.

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. JSON сығышын тикшерергә һәм раҫлау:
   - `chunk_profile_handle` — `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` детерминизм тураһында отчетҡа тап килә.
   - `chunk_digests_blake3` регенерацияланған ҡорамалдар менән тура килә.

## 5. Телеметрия тикшерә

- I18NT000000000X раҫлау яңы профиль метрикаларын фашлай:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Приборҙар таҡталары көтөлгән псевдоним аҫтында стажировка провайдерын күрһәтергә һәм нулдә браунут иҫәпләүселәрен һаҡларға тейеш, ә профиль әүҙем.

## 6.

1. Ҡыҫҡаса отчет менән URL-адрестар, асыҡ идентификатор һәм телеметрия снимоктарын төшөрөп.
.
.

Был playbook-ты яңыртыу һәр өлөш/ҡабул итеүҙе таратыуҙы тәьмин итә, сәхнәләштереү һәм етештереү буйынса бер үк детерминистик аҙымдарҙы эҙләй.