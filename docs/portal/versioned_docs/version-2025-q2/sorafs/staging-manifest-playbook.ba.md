---
lang: ba
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f708c9c597c0455761049a17989369498d318be348e28f71196bb82761dd36b
source_last_modified: "2025-12-29T18:16:35.911952+00:00"
translation_last_reviewed: 2026-02-07
id: staging-manifest-playbook-ba
title: Staging Manifest Playbook
sidebar_label: Staging Manifest Playbook
description: Checklist for enabling the Parliament-ratified chunker profile on staging Torii deployments.
translator: machine-google-reviewed
slug: /sorafs/staging-manifest-playbook-ba
---

:::иҫкәртергә канонлы сығанаҡ
Көҙгөләр `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Ике дана ла релиздар буйынса тура килтереп тотоғоҙ.
::: 1990 й.

## Обзор

Был playbook аша йөрөп мөмкинлек бирә Парламент-ратифицированный chunker профиле стажировка Torii таратыу алдынан үҙгәрештәрҙе пропагандалау етештереүгә булышлыҡ итеү. Ул SoraFS идара итеү уставын ратификацияланған тип фаразлай һәм канон ҡорамалдары һаҡлағыста бар.

## 1. Алдан шарттар

1. Каноник ҡоролмаларын һәм ҡултамғаларын синхронлаштырыу:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Ҡабул итеү конверт каталогы әҙерләй, тип Torii стартапта уҡый (миҫал юл): `/var/lib/iroha/admission/sorafs`.
3. Torii конфигын тәьмин итеү асыш кэш һәм ҡабул итеү органдарына мөмкинлек бирә:

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

1. Ҡабул ителгән провайдер ҡабул итеү конверттарын күсерергә каталогҡа һылтанма `torii.sorafs.discovery.admission.envelopes_dir`:

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

1. Ҡул ҡуйылған провайдер реклама файҙалы йөк (Norito байт) һеҙҙең етештереү
   провайдер торбаһы:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. Асыу ос нөктәһен һорап, рекламаның канонлы псевдонимдар менән барлыҡҡа килгәнен раҫлау:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   `profile_aliases` X-та беренсе яҙма булараҡ `"sorafs.sf1@1.0.0"` инә.

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

- Prometheus раҫлау яңы профиль метрикаларын фашлай:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Приборҙар таҡталары көтөлгән псевдоним аҫтында стажировка провайдерын күрһәтергә һәм нулдә браунут иҫәпләүселәрен һаҡларға тейеш, ә профиль әүҙем.

## 6.

1. Ҡыҫҡаса отчет менән URL-адрестар, асыҡ идентификатор һәм телеметрия снимоктарын төшөрөп.
.
.

Был playbook-ты яңыртыу һәр өлөш/ҡабул итеүҙе таратыуҙы тәьмин итә, сәхнәләштереү һәм етештереү буйынса бер үк детерминистик аҙымдарҙы эҙләй.
