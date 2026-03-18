---
id: account-address-status
lang: ba
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Account address compliance
description: Summary of the ADDR-2 fixture workflow and how SDK teams stay in sync.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Канон ADDR-2 өйөмө (`fixtures/account/address_vectors.json`) тота
I105 (өҫтөнлөк), ҡыҫылған (`sora`, икенсе-иң яҡшы; ярты/тулы киңлеге), күп ҡултамғалы һәм кире ҡорамалдар.
Һәр SDK + I18NT00000000004X өҫтө шул уҡ JSON-ға таяна, шуға күрә беҙ теләһә ниндәй кодекты асыҡлай алабыҙ.
дрейфы етештереүгә һуҡҡансы. Был бит эске статус брифын көҙгөләй
(I18NI000000010X тамыр һаҡлағысында) шулай портал
уҡыусылар эш ағымына һылтанма яһай ала, ҡаҙылмай ашамай моно-репо.

## Регенерация йәки раҫлау өйөм

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Флагтар:

- `--stdout` — JSON-ды реклама тикшерелеүе өсөн stdout ойошторорға сығара.
- `--out <path>` — икенсе юлға яҙығыҙ (мәҫәлән, локаль рәүештә үҙгәргән саҡта).
- `--verify` — эш күсермәһе менән яңы генерацияланған контентҡа ҡаршы сағыштырырға
  `--stdout` менән берләштерелә).

CI эш ағымы **Adress Вектор дрифт** эшләй I18NI000000015X
теләһә ниндәй ваҡытта ҡоролма, генератор, йәки docs үҙгәрә уяу рецензенттар шунда уҡ.

## Кем ҡуллана ҡоролма?

| Ер өҫтө | Валидация |
|--------|-------------|
| Кире мәғлүмәт-модель | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (сервер) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Свифт СДК | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Андроид СДК | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Һәр йүгән түңәрәк-трип канонлы байт + I105 + ҡыҫылған (`sora`, икенсе-иң яҡшы) кодлау һәм
тикшерә, тип I18NT00000000003Х-стиль хата кодтары рәткә рәткә ҡоролма өсөн кире осраҡтар.

## Автоматлаштырыу кәрәкме?

Релиз инструменттары сценарий ҡоролмаһы менән ярҙамсы менән яңыртыу мөмкин
I18NI000000022Х, был канонлы йәки раҫлаусы канонлы
күсермә/йәбештереү аҙымдары булмаған өйөм:

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

Ярҙам `--source` ҡабул итә йәки `IROHA_ACCOUNT_FIXTURE_URL` .
мөхит үҙгәртеүсән шулай SDK CI эш урындары уларҙы өҫтөнлөк көҙгө күрһәтә ала.
Ҡасан I18NI000000025X тәьмин итеү ярҙамсы яҙа
I18NI000000026X канон менән бергә
SHA-256 үҙләштереү (`account_address_fixture_remote_info`) шулай I18NT000000001X текст файлы
коллекционерҙар һәм I18NT0000000002X приборҙар таҡтаһы I18NI000000028X иҫбатлай ала
һәр ер өҫтө синхронлаштырыла. Иҫкәртмә ҡасан да булһа маҡсатлы хәбәр I18NI000000029X. Өсөн
күп ер өҫтө автоматлаштырыу ҡулланыу wrapper I18NI0000000030X
(ҡабул итеү ҡабатланған I18NI0000000031X) шулай шылтыратыу командалары баҫтырып сығара ала
бер консолидацияланған I18NI000000032X файл өсөн төйөн-экспортер текст файлы коллекторы.

## Кәрәк тулы ҡыҫҡаса?

Тулы ADDR-2 үтәү статусы (хужалар, мониторинг планы, асыҡ ғәмәлдәр пункттары)
йәшәй I18NI0000000033X репозиторий сиктәрендә 2019 йылда .
адрес структураһы менән RFC (`docs/account_structure.md`). Был битте ҡулланыу
тиҙ оператив иҫкәртмә; тәрән етәкселек өсөн репо docs кисектереп.