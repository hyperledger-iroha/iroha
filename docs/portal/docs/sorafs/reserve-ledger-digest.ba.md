---
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f0e832438aa843ed1ce6d018055a67bd1688ccb4cead9f73b87033469256015d
source_last_modified: "2026-01-22T16:26:46.526983+00:00"
translation_last_reviewed: 2026-02-07
title: Reserve Ledger Digest & Dashboards
description: How to turn `sorafs reserve ledger` output into telemetry, dashboards, and alerts for the Reserve+Rent policy.
translator: machine-google-reviewed
---

Резерв+ аренда сәйәсәте (юл картаһы пункты **SFM‐6**) хәҙер `sorafs reserve` судноһын ебәрә.
CLI ярҙамсылары плюс I18NI000000010X тәржемәсе шулай
ҡаҙна йүгерә ала детерминистик аренда/резерв күсермәләр сығара ала. Был бит көҙгөләй
I18NI000000011X-та билдәләнгән эш ағымы һәм аңлата
нисек сым яңы трансфер ем I18NT0000000003X + Алтертмайнер шулай иҡтисад һәм
идара итеү рецензенттары һәр биллинг циклын аудитлай ала.

## Эш ағымы осона тиклем

1. **Цитат + баш кейеме проекцияһы**
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

  sorafs reserve ledger \
    --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
    --provider-account i105... \
    --treasury-account i105... \
    --reserve-account i105... \
    --asset-definition xor#sora \
    --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   Баш һөйәге ярҙамсыһы I18NI000000012X блокын беркетә (аренда тейешле, запас
   етешһеҙлек, өҫкө-өҫкә дельта, андеррайтинг бул) плюс I18NT0000000006X `Transfer`
   ИСИ-ға ҡаҙна һәм запас иҫәптәре араһында XOR күсерергә кәрәк ине.

2. **Душымта генерациялау + I18NT0000000000000000000000000.
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   Дисджест ярҙамсыһы микро-XOR дөйөм XOR-ға нормалләштерә, был теркәләме,
   проекция андеррайтинг менән осраша, һәм ** тапшырыу ем** метрикаһын сығара
   `sorafs_reserve_ledger_transfer_xor` һәм
   `sorafs_reserve_ledger_instruction_total`. Ҡасан бер нисә баш китабы булырға кәрәк .
   эшкәртелгән (мәҫәлән, провайдерҙар партияһы), ҡабатлау `--ledger`/`--label` парҙары һәм
   ярҙамсы яҙа бер NDJSON/I18NT0000000001X файл, унда һәр дигест шулай
   приборҙар таҡталары бөтә циклды интерпретациялай, елемһеҙ елемһеҙ. I18NI000000018X
   файл маҡсатлы төйөн-экспортер тексты коллектор — ташларға I18NI0000000019X файл .
   экспортер’s ҡараған каталог йәки уны тейәп телеметрия биҙрә
   Ҡулланылған Резерв приборҙар таҡтаһы эше — шул уҡ
   файҙалы йөкләмәләр мәғлүмәт торбаларына.

3. **Башҡа артефакттар + дәлилдәр**
   - I18NI000000021X һәм һылтанма буйынса һеңдереүҙең матдәһе.
     һеҙҙең аҙналыҡ иҡтисад отчетынан Markdown резюмеһы.
   - Беркетергә JSON дигест аренда яндырыу-аҫҡа (шулай итеп, аудиторҙар реплей мөмкин
     математика) һәм идара итеү дәлилдәр пакеты эсендә тикшерелгән сумманы үҙ эсенә ала.
   - Әгәр ҙә һеңдереүҙең йәки андеррайтингты боҙоу тураһында һеңдерһә, иҫкәртмәгә һылтанма .
     IDs (I18NI000000022X,
     I18NI000000023X) һәм ниндәй күсермә ИСИ-лар булыуын иҫәпкә ала
     ҡулланыла.

## Метрика → приборҙар таҡталары → иҫкәртмәләр

| Сығанаҡ метрикаһы | I18NT000000004X панелендә | Иҫкәртмә / сәйәси ҡармаҡ | Иҫкәрмәләр |
|-------------|----------------------------------|---------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | “ДА аренда бүлеү (XOR/сәғәт)” I18NI000000027X | Аҙна һайын ҡаҙна һеңдерергә; запас ағымында шпиктар `SoraFSCapacityPressure` (I18NI000000029X X). |
| I18NI000000030X | “Ҡунаҡ ҡулланыу (GiB-ай)” (шул уҡ приборҙар таҡтаһы) | Пар менән баш кейеме distest иҫбатлау өсөн счет-фактуралы һаҡлау тап килгән XOR күсермәләре. |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, I18NI000000333Х | “Беренсе Snapshot (XOR)” + статус карталары I18NI000000034X | `SoraFSReserveLedgerTopUpRequired` уттары ҡасан I18NI000000036X; I18NI0000037X уттары ҡасан I18NI000000038X. |
| I18NI000000039X, I18NI000000040X | “Күсереүселәр изгелек”, “Һуңғы тапшырыу өҙөлгән”, һәм ҡаплау карталары `dashboards/grafana/sorafs_reserve_economics.json` | I18NI000000042X, `SoraFSReserveLedgerRentTransferMissing`, һәм I18NI000000044X иҫкәртә, ҡасан күсермәһе каналы юҡ йәки нулгә тиклем, хатта аренда/өҫтөндә кәрәк булһа ла; ҡаплау карталары шул уҡ осраҡтарҙа 0% тиклем төшә. |

Ҡасан аренда циклы тамамлана, яңыртыу I18NT00000000002X/NDJSON снимоктар, раҫлау .
тип I18NT0000000005X панелдәре яңы I18NI0000000045X алыу, һәм скриншоттар беркетергә +
Иҫкәртмәнсе идентификаторҙары аренда менән идара итеү пакетына. Был CLI проекцияһын иҫбатлай,
телеметрия, һәм идара итеү артефакттары бөтәһе лә **бер ** күсермәһен һәм
юл картаһын һаҡлай’иҡтисад приборҙар таҡталары менән тура килтерелгән Запас+аренда
автоматлаштырыу. 100% (йәки 1.0) һәм яңы иҫкәртмәләр уҡырға тейеш.
тейеш, бер тапҡыр аренда һәм запас өҫтәмә күсермәләр бар, тип distest.