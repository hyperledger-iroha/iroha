---
lang: ba
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Local → Global Address Toolkit
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Был бит көҙгө [`docs/source/sns/local_to_global_toolkit.md`] (../../../source/sns/local_to_global_toolkit.md)
моно-репонан. Ул CLI ярҙамсыларын һәм юл картаһы әйбере талап иткән runbooks-ты пакеттар **ADDR-5c**.

## Обзор

- I18NI000000006X I18NI000000007X CLI етештереү өсөн урап:
  - `audit.json` — структуралы сығыш `iroha tools address audit --format json`.
  - `normalized.txt` — IH58 / икенсе иң яҡшы ҡыҫылған үҙгәртелгән (I18NI0000000011X) һәр урындағы домен селекторы өсөн тура һүҙлеләр.
- Сценарийҙы парлы приборҙар таҡтаһы (`dashboards/grafana/address_ingest.json`) менән парлаштырырға
  һәм иҫкәртмәндәр ҡағиҙәләре (`dashboards/alerts/address_ingest_rules.yml`) урындағы-8 /
  Урындағы-12 өҙөклөк хәүефһеҙ. Урындағы-8 һәм урындағы-12 бәрелеш панелдәрен ҡарағыҙ плюс
  I18NI000000014X, `AddressLocal12Collision`, һәм I18NI000000016X иҫкәртмәләргә тиклем.
  асыҡ үҙгәрештәрҙе пропагандалау.
- Һылтанма [Адресс дисплей йүнәлештәре] (I18NU000000003X) һәм
  [Adress Manifest runbook](../../../source/runbooks/address_manifest_ops.md) өсөн UX һәм инциденттар-яуап контекста.

## Ҡулланыу

```bash
scripts/address_local_toolkit.sh \
  --input fixtures/address/local_digest_examples.txt \
  --output-dir artifacts/address_migration \
  --network-prefix 753 \
  --format ih58
```

Варианттар:

- IH58 урынына I18NI0000000018X өсөн I18NI000000017X.
- I18NI000000019X X Xea mare литералдар сығарыу өсөн.
- `--audit-only` конверсия аҙымын үткәреү өсөн.
- `--allow-errors` сканерлауҙы һаҡлау өсөн, ҡасан дөрөҫ формалаштырылған рәттәр барлыҡҡа килә (CLI тәртибенә тап килә).

Сценарий йүгерә аҙағында артефакт юлдарын яҙа. Ике файлды ла беркетергә
һеҙҙең үҙгәрештәр-идара итеү билеты менән бергә I18NT000000000000000Х скриншот, тип иҫбатлай нуль
Урындағы-8 асыҡлау һәм нуль урындағы-12 бәрелештәр өсөн ≥30 көн.

## CI интеграцияһы

1. Сценарийҙы махсус эштә эшләтеп, уның сығыштарын тейәгеҙ.
2. Блок берләшә, ҡасан `audit.json` урындағы селекторҙар тураһында хәбәр итә (`domain.kind = local12`).
   ҡиммәте I18NI00000000024X ҡиммәте (тик өҫтөндә йөрөү I18NI0000000025X өҫтөндә dev/һынау кластерҙары ҡасан
   регрессияларҙы диагностикалау) һәм өҫтәү
   I18NI000000026X CI шулай регрессия
   тырышлыҡтар етештереүгә һуҡҡансы уңышһыҙлыҡҡа осрай.

Ҡарағыҙ сығанаҡ документы өсөн тулыраҡ мәғлүмәт, өлгө дәлилдәр тикшерелгән исемлектәр, һәм релиз-иҫкәрмә өҙөк һеҙ ҡабаттан ҡулланырға мөмкин, ҡасан иғлан өҙөү клиенттарға.