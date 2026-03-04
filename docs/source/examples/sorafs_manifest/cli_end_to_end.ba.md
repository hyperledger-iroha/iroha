---
lang: ba
direction: ltr
source: docs/source/examples/sorafs_manifest/cli_end_to_end.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a8209e602132efb6c29962bf09aea8cd74f972fa956ea8a7a1dbac08a7f6f00f
source_last_modified: "2026-01-05T09:28:12.006380+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Manifest CLI End-to-End Example"
translator: machine-google-reviewed
---

# SoraFS Манифест CLI-ға тиклем Миҫал

Был миҫал аша атлай аша баҫтырып сығарыу документация төҙөү SoraFS ҡулланып
`sorafs_manifest_stub` CLI детерминистик өлөштәр менән бергә
һүрәтләнгән SoraFS архитектураһы RFC. Ағым ҡаплауҙары быуын күренә,
көтөү тикшерелгән, ветч-план раҫлау, һәм репетиция репетициялары шулай
командалары шул уҡ аҙымдарҙы CI индерә ала.

## Алдан шарттар

- Эш киңлеге клонланған һәм инструменттар сылбырлы әҙер (`cargo`, `rustc`).
- `fixtures/sorafs_chunker`-тан фикстуралар бар, шуға күрә көтөү ҡиммәттәре була ала
  алынған (етештереү өсөн йүгерә, ҡиммәттәрҙе тартып, миграция баш китабына инеү
  артефакт менән бәйле).
- Өлгө файҙалы йөк каталогы баҫтырып сығарыу өсөн (был миҫал `docs/book` ҡулланыла).

## 1-се аҙым — генерациялау манифест, CAR, ҡултамғалар, һәм пландар планы

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-out target/sorafs/docs.manifest_signatures.json \
  --car-out target/sorafs/docs.car \
  --chunk-fetch-plan-out target/sorafs/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101d0cfa9be459f4a4ba4da51990b75aef262ef546270db0e42d37728755d \
  --dag-codec=0x71 \
  --chunker-profile=sorafs.sf1@1.0.0
```

Команда:

- Файҙалы йөктө `ChunkProfile::DEFAULT` аша ағымдар.
- CARv2 архивын плюс өлөш-фетч планын сығара.
- `ManifestV1` яҙмаһы төҙөй, асыҡ ҡултамғаларҙы раҫлай (әгәр ҙә ҡаралған), һәм
  конвертты яҙа.
- Көтөү флагтарын үтәй, шуға күрә йүгерә уңышһыҙлыҡҡа осрай, әгәр байт дрейф.

## 2-се аҙым — Ҡулланыусылар магазины менән сығыштарҙы тикшерергә + PoR репетицияһы

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  target/sorafs/docs.car \
  --manifest target/sorafs/docs.manifest \
  --report-out target/sorafs/docs.manifest_report.json \
  --por-json-out target/sorafs/docs.por.json
```

Был CAR аша детерминистик өлөш магазины реплей, ала
Дәлил итеү-алыусанлыҡ үлсәү ағас, һәм асыҡ отчет сығара өсөн яраҡлы .
идара итеү тикшерелеүе.

## 3-сө аҙым — моделләштереү күп провайнер эҙләү

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=target/sorafs/docs.fetch_plan.json \
  --provider=primary=target/sorafs/docs.car \
  --chunk-receipts-out=target/sorafs/docs.chunk_receipts.json \
  --json-out=target/sorafs/docs.fetch_report.json
```

CI мөхиттәре өсөн, айырым файҙалы йөк юлдары менән тәьмин итеү провайдеры (мәҫәлән, монтаж
ҡорамалдар) диапазоны планлаштырыу һәм етешһеҙлектәр менән эш итеү өсөн күнекмәләр.

## 4-се аҙым — Рекорд баш китабы яҙмаһы

`docs/source/sorafs/migration_ledger.md`-тағы баҫманы төшөрөү, әсирлеккә:

- Манифест CID, CAR distest, һәм совет ҡултамғаһы хеш.
- Статус (`Draft`, `Staging`, `Pinned`).
- Һылтанмалар CI йүгерә йәки идара итеү билеттары.

## 5-се аҙым — идара итеү инструменттары аша булавка (регистрация ҡасан йәшәй)

Бер тапҡыр булавка реестры йәйелдерелгән (Майлстоун М2 миграция юл картаһында),
манифест аша тапшырыу CLI:

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --plan=target/sorafs/docs.fetch_plan.json \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-in target/sorafs/docs.manifest_signatures.json \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --council-signature-file <signer_hex>:path/to/signature.bin

cargo run -p sorafs_cli --bin sorafs_pin -- propose \
  --manifest target/sorafs/docs.manifest \
  --manifest-signatures target/sorafs/docs.manifest_signatures.json
```

Тәҡдим идентификаторы һәм артабан раҫлау операцияһы хештары булырға тейеш
аудит өсөн миграция баш китабына инеүендә төшөрөлгән.

## таҙартыу

`target/sorafs/` буйынса артефакттар архивланған йәки төйөндәр ҡуйыу өсөн тейәп була.
Манифест, ҡултамғалар, CAR, һәм алыу планы бергә шулай аҫҡы ағым
операторҙары һәм SDK командалары детерминистик рәүештә таратыуҙы раҫлай ала.