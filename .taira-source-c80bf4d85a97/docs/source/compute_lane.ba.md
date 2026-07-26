---
lang: ba
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2025-12-29T18:16:35.929771+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# иҫәпләү һыҙаты (ССК-1)

Компью һыҙаты детерминистик HTTP стилендәге шылтыратыуҙарҙы ҡабул итә, уларҙы Kotodama-ға картаға төшөрә.
инеү нөктәләре, һәм яҙмалар иҫәпләү/квитанциялар өсөн биллинг һәм идара итеү тикшерергә.
Был RFC туңдырыу өсөн манифест схемаһы, шылтыратыу/квитанция конверттары, ҡом йәшниктәре ҡоршау,
һәм беренсе релиз өсөн конфигурация ғәҙәттәгесә.

## Беленергә

- Схема: `crates/iroha_data_model/src/compute/mod.rs` X (`ComputeManifest` /
  `ComputeRoute`).
- `abi_version` `1` тиклем ҡыҫтырылған; икенсе версияһы менән маскировкалар кире ҡағыла
  раҫлау ваҡытында.
— Һәр маршрут иғлан итә:
  - `id` (`service`, `method`)
  - `entrypoint` (Kotodama инеү нөктәһе исеме)
  - codec рөхсәт исемлеге (`codecs`)
  - TTL/газ/запрос/яуап ҡапҡастары (`ttl_slots`, `gas_budget`, `max_*_bytes`)
  - детерминизм/башҡарыу класы (`determinism`, `execution_class`)
  - SoraFS ингресс/модель дескрипторҙары (`input_limits`, факультатив `model`)
  - хаҡтар ғаиләһе (`price_family`) + ресурс профиле (`resource_profile`)
  - аутентификация сәйәсәте (`auth`)
- Sandbox ҡоршауҙары йәшәй манифест `sandbox` блок һәм бөтәһе лә бүлешә
  маршруттар (режим/осраҡлылыҡ/һаҡлау һәм детерминистик булмаған сискалл кире ҡағыу).

Миҫал: Kotodama.

## Шылтыратыуҙар, үтенестәр һәм квитанциялар

- Схема: `ComputeRequest`, `ComputeCall`, `ComputeCallSummary`, `ComputeReceipt`,
  `ComputeMetering`, `ComputeOutcome` 1990 йылда.
  `crates/iroha_data_model/src/compute/mod.rs`.
- `ComputeRequest::hash()` канон запрос хеш етештерә (башлыҡтар һаҡлана
  детерминистик `BTreeMap` һәм файҙалы йөк `payload_hash` тип алып барыла).
- `ComputeCall` исемдәр киңлеге/маршрут, кодек, TTL/газ/яуап ҡапҡасын тота,
  ресурс профиле + хаҡтар ғаиләһе, auth (`Public` йәки УАИД-бәйле
  `ComputeAuthn`), детерминизм (`Strict` vs `BestEffort`), башҡарыу класы
  Кәйефтәр (CPU/GPU/TEE), иғлан ителгән SoraFS индереү байт/өлөштәре, факультатив бағыусы
  бюджет, һәм канон запрос конверт. Запрос хеш өсөн ҡулланыла.
  реплей һаҡлау һәм маршрутлаштырыу.
- Маршруттар SoraFS моделе һылтанмалар һәм индереү сиктәре индереү мөмкин
  (рәт/урын ҡапҡастары); ҡом йәшник ҡағиҙәләрен күрһәтә ҡапҡа GPU/TEE кәңәштәре.
- `ComputePriceWeights::charge_units` X иҫәпләү мәғлүмәттәрен иҫәп-хисаплы иҫәпләүгә үҙгәртә.
  берәмектәр аша түбә-бүлек циклдар һәм эгресс байт.
- `ComputeOutcome` хәбәр итеүенсә, `Success`, `Timeout`, `OutOfMemory`,
  `BudgetExhausted`, йәки Norito һәм теләк буйынса яуап хештары инә/
  ҙурлыҡтары/кодек өсөн аудит.

Миҫалдар:
- Шылтыратыу: `fixtures/compute/call_compute_payments.json`
- Квитанция: `fixtures/compute/receipt_compute_payments.json`

## ҡом йәшник һәм ресурс профилдәре- SoraFS башҡарыу режимын `IvmOnly` XX стандарт буйынса бикләп ҡуя,
  орлоҡтар детерминистик осраҡлылыҡ запрос хеш, мөмкинлек бирә, тик уҡыу өсөн SoraFS
  инеү, һәм детерминистик булмаған сискаллдарҙы кире ҡаға. GPU/TEE кәңәштәре ҡапҡалы.
  `allow_gpu_hints`/`allow_tee_hints` башҡарма детерминистик һаҡлау өсөн.
- `ComputeResourceBudget` циклдарҙа, һыҙыҡлы хәтерҙә, стекала профилле ҡапҡастарға ҡуйыла
  күләме, IO бюджеты, һәм эгресс, плюс toggles өсөн GPU кәңәштәре һәм WASI-lite ярҙамсылары.
- Ғәҙәттәгесә ике профилде (`cpu-small`, `cpu-balanced`) 2000 йылға тиклем йөкмәтә.
  `defaults::compute::resource_profiles` детерминистик fallbacks менән.

## Хаҡтар һәм биллинг берәмектәре

- Хаҡ ғаиләләре (`ComputePriceWeights`) карта циклдары һәм эгресс байттар иҫәпләүгә
  берәмектәр; ғәҙәттәгесә `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` 2019 йылда 1990 й.
  `unit_label = "cu"`. Ғаиләләр `price_family` XX спектаклдәрендә һәм
  ҡабул итеү ваҡытында үтәлгән.
- Диңгеҙ яҙмалары йөрөтөү `charged_units` плюс сей цикл/концерт/электр/оҙайлылыҡ .
  ярашыу өсөн дөйөм һандар. Зарядтар башҡарыу-класс менән көсәйтелә һәм
  детерминизм множитель (`ComputePriceAmplifiers`X) һәм 2012 йылға тиклем 2000 й.
  `compute.economics.max_cu_per_call` X; сығарылыш ҡыҫыла.
  `compute.economics.max_amplification_ratio` бәйле яуап көсәйткес.
— Спонсор бюджеттары (`ComputeCall::sponsor_budget_cu`) ҡаршы үтәлә.
  пер-шылтыратыу/көндәлек ҡапҡастар; биллләштерелгән берәмектәр иғлан ителгән спонсор бюджетынан артмаҫҡа тейеш.
- Идара итеү хаҡтарын яңыртыу хәүеф-класс сиктәрен ҡуллана.
  Norito һәм 1990 йылда теркәлгән база ғаиләләре.
  `compute.economics.price_family_baseline`; файҙаланыу
  `ComputeEconomics::apply_price_update`, яңыртыу алдынан дельталарҙы раҫлау өсөн
  әүҙем ғаилә картаһы. Torii конфигурация яңыртыуҙары ҡулланыу
  `ConfigUpdate::ComputePricing`, ә кзо уны бер үк сиктәр менән ҡуллана.
  идара итеүҙе һаҡлау детерминистик.

## Конфигурация

Яңы иҫәпләү конфигурацияһы йәшәй `crates/iroha_config/src/parameters`:

- Ҡулланыусы ҡарашы: `Compute` (`user.rs`) env өҫтөнлөктәре менән:
  - `COMPUTE_ENABLED` X (`false` стандарты)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - ```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```.
- Хаҡтар/иҡтисад: `compute.economics` XX
  `max_cu_per_call`/`max_amplification_ratio`, түләү бүленә, бағыусылыҡ ҡапҡастары
  (шылтыратыу һәм көндәлек CU), хаҡтар ғаилә база һыҙыҡтары + хәүеф кластары/сиктәре өсөн
  идара итеүҙе яңыртыу, һәм башҡарыу-класс множитель (ГПУ/ТЭЭ/иң яҡшы тырышлыҡ).
- Ысын/подъезд: `actual.rs` / `defaults.rs::compute` анализ фашланды
  `Compute` параметрҙары (исемдәре, профилдәре, хаҡтар ғаиләләре, ҡом йәшник).
- Дөрөҫ булмаған конфигурациялар (буш исемдәр киңлеге, профиль/ғаилә юҡ, TTL ҡапҡасы
  инверсиялар) анализлау ваҡытында `InvalidComputeConfig` тип атала.

## Һынауҙар һәм ҡорамалдар

- Детерминистик ярҙамсылар (SoraFS, хаҡтар) һәм ҡоролма routtrips 1990 йылда йәшәй.
  `crates/iroha_data_model/src/compute/mod.rs` X (ҡара: `fixtures_round_trip`,
  `request_hash_is_stable`, Kotodama).
- JSON ҡоролмалары йәшәй `fixtures/compute/` һәм мәғлүмәттәр-модель тарафынан ҡулланыла
  регрессия ҡаплауы өсөн һынауҙар.

## СЛО йүгән һәм бюджеттар- `compute.slo.*` конфигурацияһы шлюз СЛО ручкаларын фашлай (осоуҙа сират
  тәрәнлек, РПС ҡапҡасы, һәм латентлыҡ маҡсаттары) .
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`. Ғәҙәттәгесә: 32
  осоуҙа, 512 сират буйынса маршрут, 200 RPS, p50 25мс, p95 75мс, p99 120мс.
- Йүгереп еңел эскәмйә жгут SLO резюме һәм үтенес/ҡоролма тотоу өсөн
  снимок: `йөк йүгерә -р xtask --бин компьютер ҡапҡаһы -- эскәмйә [manifest_path]
  [икерациялар] [ҡағиҙә] [out_dir] ` (defaults: `fixtures/компью/манфест_payments.json`,
  128 итерацион, бер конкурентлыҡ 16, сығыштар буйынса .
  `artifacts/compute_gateway/bench_summary.{json,md}`). Эскәмйә ҡуллана
  детерминистик файҙалы йөктәр (`fixtures/compute/payload_compute_payments.json`) һәм
  пер-запрокты башлыҡтары ҡотолоу өсөн реплей бәрелештәр, шул уҡ ваҡытта күнекмәләр
  `echo`/`uppercase`/`sha3` XX XX быуат.

## SDK/CLI паритет ҡорамалдары

- Canonical ҡорамалдар `fixtures/compute/` аҫтында йәшәй: асыҡ, шылтыратыу, файҙалы йөк, һәм
  шлюз стилендәге яуап/квитанция планировкаһы. Стрейк хештар шылтыратыуға тап килергә тейеш
  `request.payload_hash`; ярҙамсы файҙалы йөк 1990 йылда йәшәй.
  `fixtures/compute/payload_compute_payments.json`.
- CLI суднолары `iroha compute simulate` һәм `iroha compute invoke`:

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- Й.С.: `loadComputeFixtures`/SoraFS/`buildGatewayRequest` 1990 йылда йәшәй.
  `javascript/iroha_js/src/compute.js` регрессия һынауҙары менән
  `javascript/iroha_js/test/computeExamples.test.js`.
- Swift: `ComputeSimulator` йөктәр шул уҡ ҡоролма, раҫлай файҙалы йөк хештары,
  һәм 1990 йылда һынауҙар менән инеү урындарын имитациялай.
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`.
- CLI/JS/Swift ярҙамсылары бөтәһе лә бер үк Norito ҡоролмалары менән бүлешә, шуға күрә SDKs ала
  раҫлау запрос төҙөү һәм хеш офлайн менән эш итеү һуғылмай
  йүгереп шлюз.