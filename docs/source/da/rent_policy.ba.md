---
lang: ba
direction: ltr
source: docs/source/da/rent_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7cdc46bcd87af7924817a94900c8fad2c23570607f4065f19d8a42d259fe83f
source_last_modified: "2026-01-22T14:35:37.691079+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Мәғлүмәттәр булыуы аренда & стимул сәйәсәте (DA-7)

_Статус: Зыяны — Хужалар: Иҡтисад WG / ҡаҙна / Һаҡлау командаһы_

Юл картаһы әйбер **DA-7** асыҡ XOR-деноминацияланған аренда һәр блок .
`/v1/da/ingest` тапшырылған, плюс бонустар, улар бүләк PDP/PoTR башҡарыу һәм
эгресс клиенттарҙы алыу өсөн хеҙмәт иткән. Был документта башланғыс параметрҙар билдәләнә,
уларҙы мәғлүмәттәр-модель күрһәтеү, һәм иҫәпләү эш ағымы ҡулланылған Torii,
SDKs, һәм ҡаҙна таҡталары.

## Сәйәсәт структураһы

Сәйәсәт кодланған [`DaRentPolicyV1`] (/crates/iroha_data_model/src/da/types.rs)
мәғлүмәт моделе сиктәрендә. Torii һәм идара итеү инструменттары 2012 йылда сәйәсәтте һаҡлай.
Norito файҙалы йөктәр, шулай итеп, аренда цитаталар һәм дәртләндереүҙең леджерҙары ҡабаттан иҫәпләргә мөмкин
детерминистик яҡтан. Схема биш ручканы фашлай:

| Ялан | Тасуирлама | Ғәҙәттәгесә |
|------|-------------|---------|
| `base_rate_per_gib_month` | XOR ғәйепләнгән бер ГБ айына һаҡлау. | `250_000` микро-XOR (0,25 XOR) |
| `protocol_reserve_bps` | Протокол резервына (нигеҙендә мәрәйҙәр) маршрутлаштырылған аренда хаҡы менән бүлешегеҙ. | `2_000` (20%) |
| `pdp_bonus_bps` | Бонус проценты уңышлы ПДП баһалау өсөн. | `500` (5%) |
| `potr_bonus_bps` | Бонус проценты уңышлы PoTR баһалау өсөн. | `250` (2,5%) |
| `egress_credit_per_gib` | Кредит түләнгән, ҡасан провайдер хеҙмәт итә 1GiB DA мәғлүмәттәре. | `1_500` микро-XOR |

Бөтә нигеҙ-нөктә ҡиммәттәре `BASIS_POINTS_PER_UNIT` (10000) менән раҫлана.
Сәйәсәт яңыртыуҙары идара итеү аша сәйәхәт итергә тейеш, һәм һәр Torii төйөндәре фашлай
әүҙем сәйәсәт аша `torii.da_ingest.rent_policy` конфигурацияһы бүлеге
(`iroha_config` X). Операторҙар `config.toml`-тағы ғәҙәттәгесә өҫтөнлөк бирә ала:

```toml
[torii.da_ingest.rent_policy]
base_rate_per_gib_month_micro = 250000        # 0.25 XOR/GiB-month
protocol_reserve_bps = 2000                   # 20% protocol reserve
pdp_bonus_bps = 500                           # 5% PDP bonus
potr_bonus_bps = 250                          # 2.5% PoTR bonus
egress_credit_per_gib_micro = 1500            # 0.0015 XOR/GiB egress credit
```

CLI инструменттары (`iroha app da rent-quote`) ҡабул итә, шул уҡ Norito/JSON сәйәсәт индереүҙәре .
һәм артефакттар сығара, улар әүҙем `DaRentPolicyV1`-ты көҙгөләй.
кире Torii штатына. Тәьмин итеү сәйәсәт снимок ҡулланылған өсөн ingest йүгерә шулай
цитата ҡабатланған булып ҡала.

### Ҡаты цитата артефакттары

`iroha app da rent-quote --gib <size> --months <months> --quote-out <path>` йүгерергә
экрандағы резюме ла, һылыу JSON артефактын да сығарырға. Файл
`policy_source` рекордтары, инсульт `DaRentPolicyV1` снимок, иҫәпләүҙәр
`DaRentQuote`, һәм алынған `ledger_projection` (сериализация аша
[`DaRentLedgerProjection`](/crates/iroha_data_model/src/da/types.rs)) ҡаҙна таҡталары һәм леджер ISI өсөн яраҡлы итеү.
торбалар. Ҡасан `--quote-out` мәрәйҙәре оялы каталогта CLI теләһә ниндәй булдыра
юғалған ата-әсәләр, шуға күрә операторҙар стандартлаштырыу мөмкин урындар, мәҫәлән,
`artifacts/da/rent_quotes/<timestamp>.json` башҡа DA дәлилдәре менән бер рәттән өйөмдәр.
Ҡушымта артефакт аренда раҫлау йәки ярашыу йүгерә, шулай XOR
тарҡалыу (база аренда, запас, PDP/PoTR бонустар, һәм эгресс кредиттар) был
ҡабатлана. Pass `--policy-label "<text>"` автоматик рәүештә өҫтөнлөк бирергә
алынған `policy_source` тасуирлама (файл юлдары, индерелгән ғәҙәттәгесә, һ.б.).
кеше менән уҡыла торған тег, мәҫәлән, идара итеү билеты йәки асыҡ хеш; CLI өҙөктәре
был ҡиммәт һәм буш/аҡ киңлек-тик ептәрҙе кире ҡаға, шуға күрә теркәлгән дәлилдәр
аудитлы булып ҡала.

```json
{
  "policy_source": "policy JSON `configs/da/rent_policy.json`",
  "gib": 10,
  "months": 3,
  "policy": { "...": "DaRentPolicyV1 fields elided" },
  "quote": { "...": "DaRentQuote breakdown" },
  "ledger_projection": {
    "rent_due": { "micro": 7500000 },
    "protocol_reserve_due": { "micro": 1500000 },
    "provider_reward_due": { "micro": 6000000 },
    "pdp_bonus_pool": { "micro": 375000 },
    "potr_bonus_pool": { "micro": 187500 },
    "egress_credit_per_gib": { "micro": 1500 }
  }
}
```Баш китабы проекцияһы бүлеге туранан-тура DA аренда леджер ИСИ-ға туҡлана: ул
билдәләй XOR дельта өсөн тәғәйенләнгән протокол резерв, провайдер түләүҙәр, һәм
пер-иҫбатлау бонус бассейндар талап итмәйенсә, заказ буйынса оркестрлаштырыу коды.

### генерациялаусы аренда лидерҙары планы пландары

`iroha app da rent-ledger --quote <path> --payer-account <id> --treasury-account <id> --protocol-reserve-account <id> --provider-account <id> --pdp-bonus-account <id> --potr-bonus-account <id> --asset-definition 61CtjvNd9T3THAR65GsMVHr82Bjc` йүгерергә
үҙгәртеп ҡороу өсөн һаҡланған аренда цитатаһы башҡарыла торған баш кейеме күсермәләр. Команда
анализланған `ledger_projection`, Norito `Transfer` инструкцияларын сығара
тип йыя база ҡуртымға ҡаҙнаға, маршруттар запас/провайдер
өлөштәре, һәм алдан финанслай PDP/PoTR бонус пулдар туранан-тура түләүсе. 1990 й.
сығыу JSON көҙгө цитата метамағлүмәттәр шулай CI һәм ҡаҙна инструменттары сәбәп була ала
тураһында шул уҡ артефакт:

```json
{
  "quote_path": "artifacts/da/rent_quotes/2025-12-07/rent.json",
  "rent_due_micro_xor": 7500000,
  "protocol_reserve_due_micro_xor": 1500000,
  "provider_reward_due_micro_xor": 6000000,
  "pdp_bonus_pool_micro_xor": 375000,
  "potr_bonus_pool_micro_xor": 187500,
  "egress_credit_per_gib_micro_xor": 1500,
  "instructions": [
    { "Transfer": { "...": "payer -> treasury base rent instruction elided" }},
    { "Transfer": { "...": "treasury -> reserve" }},
    { "Transfer": { "...": "treasury -> provider payout" }},
    { "Transfer": { "...": "payer -> PDP bonus escrow" }},
    { "Transfer": { "...": "payer -> PoTR bonus escrow" }}
  ]
}
```

Һуңғы `egress_credit_per_gib_micro_xor` яланы приборҙар таҡтаһы һәм түләү мөмкинлеге бирә
графиктар тура килтереп сығарыу менән компенсация аренда сәйәсәте, тип етештерә
цитата скрипт йәбештереүҙә сәйәсәт математикаһын ҡабаттан иҫәпләмәйенсә.

## Миҫал цитатаһы

```rust
use iroha_data_model::da::types::DaRentPolicyV1;

// 10 GiB retained for 3 months.
let policy = DaRentPolicyV1::default();
let quote = policy.quote(10, 3).expect("policy validated");

assert_eq!(quote.base_rent.as_micro(), 7_500_000);      // 7.5 XOR total rent
assert_eq!(quote.protocol_reserve.as_micro(), 1_500_000); // 20% reserve
assert_eq!(quote.provider_reward.as_micro(), 6_000_000);  // Direct provider payout
assert_eq!(quote.pdp_bonus.as_micro(), 375_000);          // PDP success bonus
assert_eq!(quote.potr_bonus.as_micro(), 187_500);         // PoTR success bonus
assert_eq!(quote.egress_credit_per_gib.as_micro(), 1_500);
```

Цитата Torii төйөндәре, SDKs һәм ҡаҙна хәбәр итеүенсә, сөнки цитата, сөнки
ул махсус математика урынына детерминистик Norito структураларын ҡуллана. Операторҙар ала
беркетергә JSON/CBOR кодланған `DaRentPolicyV1` идара итеү тәҡдимдәре йәки аренда
аудит иҫбатлау өсөн, ниндәй параметрҙар өсөн ғәмәлдә булған теләһә ниндәй бирелгән тап.

## Бонустар һәм запастар

- **Протокол резервы:** `protocol_reserve_bps` XOR резервын финанслау, тип артҡа .
  ғәҙәттән тыш хәлдәрҙе ҡабаттан репликациялау һәм ҡайтарыуҙы ҡыҫҡартыу. Был биҙрәгә ҡаҙна күҙәтә
  айырым тәьмин итеү өсөн леджер баланстары тап килгән конфигурацияланған ставкаһы.
- **PDP/PoTR бонустар:** Һәр уңышлы иҫбатлаусы баһалау өҫтәмә ала .
  түләү `base_rent × bonus_bps`-тан алынған. Ҡасан DA планлаштырыусы иҫбатлау сығара
  квитанцияларҙа ул нигеҙ-нөктә тегтарын үҙ эсенә ала, шуға күрә стимулдарҙы яңынан уйнатырға мөмкин.
- **Емсес кредит:** Провайдерҙар яҙма GiB хеҙмәт итә асыҡ, ҡабатлау өсөн .
  `egress_credit_per_gib`, һәм квитанцияларҙы `iroha app da prove-availability` аша тапшыра.
  Аренда сәйәсәте идара итеү менән синхронлаштырыуҙа пер-ГиБ суммаһын һаҡлай.

## Оператив ағым

1. **Ингест:** `/v1/da/ingest` әүҙем `DaRentPolicyV1` әүҙем йөкләй, цитаталар аренда
   нигеҙендә блоб ҙурлығы һәм һаҡлау, һәм цитата индереү Norito .
   беленергә. Таҡтатор ҡул ҡуя, тип һылтанмалар аренда хеш һәм
   һаҡлау билет id.
2. **Ике бухгалтерия:** Ҡаҙна нәфис сценарийҙары асыҡтан-асыҡ, саҡырыуҙы расшифровка
   `DaRentPolicyV1::quote`, һәм халыҡ ҡуртымға леджерҙары (база арендаһы, запас,
   бонустар, һәм көтөлгән эгресс кредиттар). Теләһә ниндәй тап килмәү араһында теркәлгән аренда
   һәм ҡабаттан иҫәпләнгән цитаталар CI-ны уңышһыҙлыҡҡа килтерә.
3. **Дуҫлау бүләктәр:** Ҡасан PDP/PoTR планлаштырыусылар уңышты билдәләй, улар квитанция сығара
   содержащий асыҡ distest, иҫбатлау төрө, һәм XOR бонус алынған .
   сәйәсәт. Идара итеү шул уҡ цитатаны ҡабаттан иҫәпләп, түләүҙәрҙе аудитлай ала.
4. **Исермә компенсация:** Фетч оркестрҙары ҡул ҡуйылған эгресс резюмеларын тапшыра.
   Torii ГБ һанын `egress_credit_per_gib`-ға ҡабатлай һәм түләү сығара.
   аренда эскроуына ҡаршы күрһәтмәләр.

## ТелеметрияTorii төйөндәре аренда ҡулланыу аша фашлау аша түбәндәге Prometheus (маркировкалар:
`cluster`, `storage_class`):

- `torii_da_rent_gib_months_total` — GiB-айҙар Norito тарафынан цитатала.
- `torii_da_rent_base_micro_total` — база арендаһы (микро XOR) ингестта тупланған.
- `torii_da_protocol_reserve_micro_total` — протокол запас иғәнәләре.
- `torii_da_provider_reward_micro_total` — провайдер яғында аренда түләү.
- `torii_da_pdp_bonus_micro_total` һәм `torii_da_potr_bonus_micro_total` —
  PDP/PoTR бонус пулдары сығанаҡтарҙан ингест цитата.

Иҡтисад приборҙар таҡталары был иҫәпләүселәргә таяна, тип тәьмин итеү өсөн леджер ИСИ, запас крандар,
һәм PDP/PoTR бонус графиктары бөтә тап килә сәйәсәт параметрҙары өсөн ғәмәлдә һәр береһе .
кластер һәм һаҡлау класы. SoraFS ҡәҙерле һаулыҡ Grafana идаралығы
(`dashboards/grafana/sorafs_capacity_health.json`) хәҙер махсус панелдәр күрһәтә
өсөн аренда бүлергә, PDP/PoTR бонус туп, һәм GiB-ай тотоу, рөхсәт
Ҡаҙна фильтр өсөн Torii кластер йәки һаҡлау класы тикшергәндә negest
күләме һәм түләүҙәр.

## Киләһе аҙымдар

- ✅ `/v1/da/ingest` квитанциялары хәҙер `rent_quote` һәм CLI/SDK өҫтө цитаталар күрһәтелә.
  база аренда, запас өлөшө, һәм PDP/PoTR бонустар шулай тапшырыусылар ҡарап ала XOR бурыстары алдынан .
  файҙалы йөктәр ҡылыу.
- Интеграция аренда леджер менән буласаҡ DA репутация/заказ-китап каналдары .
  иҫбатлау өсөн, юғары доступный провайдерҙар дөрөҫ түләү ала.