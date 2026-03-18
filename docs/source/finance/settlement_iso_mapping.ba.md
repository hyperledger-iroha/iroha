---
lang: ba
direction: ltr
source: docs/source/finance/settlement_iso_mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d1f1005d6a273ab732a7c7a7adca349c17569fe2e2755b8daccf2186724044f8
source_last_modified: "2026-01-22T16:26:46.568382+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Ҡасаба ↔ ISO 20022 Ялан картаһы

Был иҫкәрмә Iroha ҡасаба инструкциялары араһында канонлы карта төҙөүҙе ала.
(`DvpIsi`, `PvpIsi`, репо залог ағымдары) һәм ISO 20022 хәбәрҙәр ҡулланыла
күпер аша. Ул 1990 йылда тормошҡа ашырылған хәбәрҙе скафандрлауҙы сағылдыра.
Norito һәм етештереү йәки 2012 йылда белдергәндә һылтанма булып хеҙмәт итә.
Norito йй.

### Һылтанма мәғлүмәттәре сәйәсәте (иддентификаторҙар һәм раҫлау)

Был сәйәсәт идентификатор өҫтөнлөктәрен, валидация ҡағиҙәләрен һәм белешмә-мәғлүмәттәрҙе ҡаплай .
бурыстар, тип Norito ↔ ISO 20022 күпере хәбәрҙәр сығарыу алдынан үтәргә тейеш.

**Якорь ISO хәбәр эсендә күрһәтә:**
- **Приборҙар идентификаторҙары** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId` .
  (йәки эквивалентлы приборҙар яланы).
- **Яҡтар / агенттар** → `DlvrgSttlmPties/Pty` һәм `sese.*` өсөн `RcvgSttlmPties/Pty`,
  йәки агент структуралары `pacs.009`.
- **Иҫәптәр** → `…/Acct` элементтары һаҡлау/аҡса иҫәптәре өсөн; көҙгөһөн эй?
  `AccountId` Norito.
- **Проприетар идентификаторҙар** → `…/OthrId` Norito менән һәм 1990 йылда көҙгө.
  `SupplementaryData`. Бер ҡасан да көйләнгән идентификаторҙарҙы хужалары менән алмаштырмағыҙ.

#### Иғтибар итеүсе өҫтөнлөклө хәбәрҙәр ғаиләһе буйынса

#### `sese.023` X / `.024` / `.025` (ҡиммәтле ҡағыҙҙар ҡаласыҡ)

- **Инструмент (`FinInstrmId`)**
  - Өҫтөнлөк: **ИСИН** `…/ISIN` буйынса. Ул CSDs / T2S өсөн канон идентификаторы.[^анна]
  - Фолбектар:
    - **CUSIP** йәки башҡа NSIN `…/OthrId/Id` буйынса ISO тышҡы өлөшөнән ISO
      код исемлеге (мәҫәлән, `CUSP`); индереү эмитенты `Issr` ҡасан мандат.[^iso_mdr]
    - *Norito актив ID** проприетар булараҡ: `…/OthrId/Id`, `Tp/Prtry="NORITO_ASSET_ID"`, һәм
      `SupplementaryData` X-та шул уҡ ҡиммәтте теркәй.
  - Опциональ дескрипторҙар: **CFI** (`ClssfctnTp`) һәм **FISN** ҡайҙа ярҙам итеү өсөн ярҙам
    ярашыу.[^iso_cfi][^iso_fisn].
- **Яҡтар (`DlvrgSttlmPties`, `RcvgSttlmPties`)**
  - Өҫтөнлөк: **BIC** (`AnyBIC/BICFI`, ISO 9362).[^swift_bic]
  - Fallback: **LEI** ҡайҙа хәбәр версияһы махсус LEI яланын фашлай; әгәр
    18NI00000084X маркалары менән асыҡ IDs-тар юҡ, проприетарный идентификаторҙар һәм метамағлүмәттәрҙә БИК-ты үҙ эсенә ала.[^iso_cr]
- **Аҫабалыҡ / урыны** → **МИК** майҙансыҡ өсөн һәм **BIC** өсөн CSD.[^iso_mic]

#### `colr.010` / `.011` / `.012` һәм `colr.007` (залог идара итеү)

- `sese.*` XX (ISIN өҫтөнлөк) кеүек үк инструмент ҡағиҙәләрен үтәгеҙ.
- Партиялар **BIC** ҡулланыла, ғәҙәттәгесә; **LEI** ҡайҙа схема уны фашлай ҡабул ителә.[^swift_bic]
- Аҡса суммалары ҡулланырға тейеш **ISO 4217** валюта кодтары менән дөрөҫ бәләкәй берәмектәр.[^iso_4217]

#### `pacs.009` / `camt.054` (PvP финанслау һәм белдереүҙе)- **Агентс (`InstgAgt`, `InstdAgt`, бурыслы/кредитор агенттары)** → **BIC** менән факультатив .
  Ҡайҙа рөхсәт ителә.[^swift_bic]
- **Иҫәптәр**
  - Банк: **BIC** һәм эске иҫәп һылтанмалары буйынса асыҡлау.
  - Клиенттарға ҡаршы белдереүҙе (`camt.054`): унда бар һәм уны раҫлау өсөн **IBAN**
    (оҙонлоғо, ил ҡағиҙәләре, мод-97 тикшерелгән сумма).[^swift_iban].
- ** валюта** → **ISO 4217** 3 хәрефле код, бәләкәй генә берәмектәрҙе түңәрәкләүҙе хөрмәт итә.[^iso_4217]
- **Torii ингестия** → PvP финанслау аяҡтарын Norito аша тапшырырға; күпер
  `Purp=SECU` талап итә һәм хәҙер һылтанма мәғлүмәттәре конфигурацияланғанда BIC үткәүелдәрен үтәй.

#### раҫлау ҡағиҙәләре (эмиссия алдынан ҡулланыла)

| Идентификатор | Валидация ҡағиҙәһе | Иҫкәрмәләр |
|----------|-----------------|-------|
| **ИЙИН** | Регекс `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` һәм Лун (мод-10) ISO 6166 Ҡушымта С | Күпер эмиссияһына тиклем кире ҡағыу; өҫкә байытыуҙы өҫтөн күрә.[^анна_luhn] |
| **КУЗИП** | Regex `^[A-Z0-9]{9}$` һәм модул-10 2 үлсәү менән (һандар картаһы картаһы) | ИСИН ҡасан ғына доступный булмаған; карта аша ANNA/CUSIP үткәүеле бер тапҡыр сығанаҡ.[^күк] |
| **LEI** | Regex `^[A-Z0-9]{18}[0-9]{2}$` һәм мод-97 чек цифр (ISO 17442) | Ҡабул итеү алдынан GLEIF көн һайын дельта файлдарына ҡаршы раҫлау.[^gleif] |
| **БИК** | Регекс `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | Опциональ тармаҡ коды (һуңғы өс углерод). RA файлдарында әүҙем статус раҫлау.[^swift_bic] |
| **МИК** | ISO 10383 RA файлынан һаҡлау; тәьмин итеү майҙансыҡтары әүҙем (юҡ `!` туҡтатыу флагы) | Флагы эмиссияға тиклем МИК-тарҙы кире ҡаҡты.[^iso_mic] |
| **ИБАН** | Илгә хас оҙонлоҡ, ҙур хәрефле хәреф-һан, мод-97 = 1 | SWIFT тарафынан һаҡланған реестр ҡулланыу; структур яҡтан дөрөҫ булмаған ИБАН-дарҙы кире ҡаға.[^swift_iban] |
| **Приетар иҫәп/партия идентификаторҙары** | `Max35Text` (UTF-8, ≤35 символдар) менән ҡырҡылған аҡ шарлауыҡ | `GenericAccountIdentification1.Id` һәм `PartyIdentification135.Othr/Id` ятҡылыҡтарына ҡағыла. символдан ашыу яҙмаларҙы кире ҡаға, шуға күрә күпер файҙалы йөктәре ISO схемаларына ярашлы. |
| **Прокси иҫәп идентификаторҙары** | `…/Prxy/Id` буйынса `Max2048Text` `…/Prxy/Tp/{Cd,Prtry}`-та опциональ тип кодтары менән бушамай | Беренсел IBAN менән бергә һаҡлана; раҫлау һаман да IBANs талап итә, шул уҡ ваҡытта ҡабул итеү прокси-ручка (фактивный тип кодтары менән) көҙгө PvP рельстар. |
| **CFI** | Алты характеристика коды, ISO 10962 таксономияһы ярҙамында ҙур хәрефтәр | Опциональ байытыу; тәьмин итеү персонаждар тап килә инструмент класы.[^iso_cfi] |
| **ФИСН** | символға тиклем, ҙур хәрефле плюс сикләнгән тыныш билдәләре | Ихтыяри; ҡыҫҡартыу/нормализировать ISO 18774 етәкселек.[^iso_fisn] |
| **Ваҡлау** | ISO 4217 3-хәрефле код, масштабта бәләкәй берәмектәр менән билдәләнгән | Суммалар түңәрәк рөхсәт ителгән унлыҡтарға тиклем түңәрәк; Norito яғында үтәү.[^iso_4217] |

#### Crosswalk һәм мәғлүмәттәрҙе хеҙмәтләндереүҙең бурыстары- **ИСИН ↔ Norito актив идентификаторы** һәм **CUSIP ↔ ИСИН** үткәүелдәр. Яңыртыу төндә .
  ANNA/DSB каналдары һәм версияһында CI ҡулланған снимоктар менән идара итә.
- Яңыртыу **BIC ↔ LEI** карталар GLEIF йәмәғәт мөнәсәбәттәре файлдарынан, шулай итеп, күпер ала
  кәрәк саҡта икеһен дә сығарырға.[^бик_лей]
- Магазин **МИК аныҡлауҙары** күпер метамағлүмәттәре менән бер рәттән, шуға күрә майҙансыҡ раҫлау
  детерминистик хатта RA файлдары көн уртаһында үҙгәргәндә лә.[^iso_mic]
- Яҙма мәғлүмәттәр провенанс (ваҡыт маркаһы + сығанаҡ) күпер метамағлүмәттәре өсөн аудит. Персик .
  снимок идентификаторы менән бер рәттән эксплуатацияланған күрһәтмәләр.
- `iso_bridge.reference_data.cache_dir` конфигурациялау өсөн һәр тейәлгән мәғлүмәттәр йыйылмаһының күсермәһен һаҡлау өсөн
  провенанс метамағлүмәттәре менән бер рәттән (версия, сығанаҡ, ваҡыт тамғаһы, тикшерелгән сумма). Был аудиторҙарға мөмкинлек бирә .
  һәм операторҙар тарихи каналдарҙы айырыу өсөн хатта өҫкө снимоктар әйләнгәндән һуң да.
- ISO үткәүеле снимоктары `iroha_core::iso_bridge::reference_data` X ярҙамында ингестироваться.
  `iso_bridge.reference_data` конфигурация блогы (юлдар + яңыртыу интервалы). Магазиндар
  `iso_reference_status`, `iso_reference_age_seconds`, `iso_reference_records`, һәм
  `iso_reference_refresh_interval_secs` йүләрлек ваҡыты һаулыҡты иҫкәрткән өсөн фашлай. Torii
  күпер кире ҡаға `pacs.008` тапшырыуҙары, улар агенты БИК-тар конфигурацияланған
  йәйәүлеләр үткәүеле, өҫтөнә детерминистик `InvalidIdentifier` хаталары, ҡасан контрагент .
  Билдәһеҙ.【крат/ироха_тории/срк/изо20022_ күпер.р#L1078】
- IBAN һәм ISO 4217 бәйләүҙәр бер ҡатламда үтәлә: pacs.008/pacs.009 ағымдары хәҙер
  `InvalidIdentifier` хаталар сығарыу, ҡасан бурыслы/кредитор IBANs етмәй конфигурацияланған псевдоним йәки ҡасан
  ҡасаба валютаһы `currency_assets`-тан юғала, был боҙоҡ күперҙе иҫкәртергә
  инструкциялар баш китабына барып етеү. IBAN раҫлау шулай уҡ ил-конкрет ҡулланыла
  оҙонлоҡ һәм һанлы тикшерелгән һандар ISO 7064 мод‐97 үткәнсе, шулай структур дөрөҫ түгел
  Ҡиммәттәре иртә кире ҡағыла.【крат/ироха_тории/scrii/so2022_ күпер.р#L75】【краттар/ироха_тории/srii/so2022_bridg
- CLI ҡасаба ярҙамсылары шул уҡ һаҡсы рельстарын мираҫҡа ала: үткәреү
  `--iso-reference-crosswalk <path>` X`--delivery-instrument-id` менән бер рәттән DvP-ға эйә булыу өсөн
  алдан ҡарау инструмент идентификаторҙары `sese.023` XML снимокты сығарыр алдынан.
- `cargo xtask iso-bridge-lint` (һәм CI урау `ci/check_iso_reference_data.sh`) линт
  йәйәүлеләр үткәүеле снимоктары һәм ҡоролмалары. Команда `--isin`, `--bic-lei`, `--mic`, һәм ҡабул итә.
  `--fixtures` флагтары һәм `fixtures/iso_bridge/`-та өлгө мәғлүмәттәр йыйылмаһына кире төшә, ҡасан эшләй
  аргументтарһыҙ.【xtask/src/main.rs#L146】【ци/чек_исо_мәғлүмәттәр.ш#L1】
- IVM ярҙамсыһы хәҙер реаль ISO 20022 XML конверттары (баш.001 + `DataPDU`X + `Document`) ашау.
  һәм раҫлай бизнес-ҡушымта башы аша `head.001` схемаһы шулай Norito,
  `MsgDefIdr`, `CreDt`, һәм BIC/ClrSysMmbId агенттары детерминистик яҡтан һаҡлана; XMLDSig/XAdES
  блоктар аңлы рәүештә һикереп ҡала. Регрессия анализдары өлгөләрҙе һәм яңы ҡулланаБашлыҡ конверт ҡоролмаһы карталарҙы һаҡлау өсөн.【крат/срк/изо20022.rs:265】【краттар/vm/scr2022.rs:3301【крет/вм/срк/изо20022.rs:3703】

#### Регулятив һәм баҙар-структура ҡараштары

- **Т+1 иҫәп-хисап**: АҠШ/Канада акциялары баҙарҙары 2024 йылда T+1-гә күсте; Norito көйләү
  график һәм СЛА тейешле рәүештә иҫкәртмәләр.[^sec_t1][^csa_t1]
- **CSDR штрафтар**: Ҡасаба дисциплина ҡағиҙәләре аҡса штрафтарын үтәй; тәьмин итеү Norito
  метамағлүмәттәр ярашыу өсөн штраф һылтанмаларын тота.[^csdr]
- **Шул көнлөк ҡасаба осоусылары**: Һиндостандың көйләүсеһе Т0/Т+0 ҡасабаһында фазлана; һаҡларға
  күпер календарҙары яңыртылған, сөнки осоусылар киңәйә.[^india_t0]
- **Баллы һатып алыу-ин / тота**: ESMA яңыртыуҙары мониторы һатып алыу-ваҡыт һыҙыҡтары һәм факультатив тота
  шулай шартлы тапшырыу (`HldInd`) һуңғы йүнәлештәр менән тура килә.[^csdr]

[^anna]: ANNA ISIN Guidelines, December 2023. https://anna-web.org/wp-content/uploads/2024/01/ISIN-Guidelines-Version-22-Dec-2023.pdf
[^iso_mdr]: ISO 20022 external code list (CUSIP `CUSP`) and MDR Part 2. https://www.iso20022.org/milestone/22048/download
[^iso_cfi]: ISO 10962 (CFI) taxonomy. https://www.iso.org/standard/81140.html
[^iso_fisn]: ISO 18774 (FISN) format guidance. https://www.iso.org/standard/66153.html
[^swift_bic]: SWIFT business identifier code (ISO 9362) guidance. https://www.swift.com/standards/data-standards/bic-business-identifier-code
[^iso_cr]: ISO 20022 change request introducing LEI options for party identification. https://www.iso20022.org/milestone/16116/download
[^iso_mic]: ISO 10383 Market Identifier Code maintenance agency. https://www.iso20022.org/market-identifier-codes
[^iso_4217]: ISO 4217 currency and minor-units table (SIX). https://www.six-group.com/en/products-services/financial-information/market-reference-data/data-standards.html
[^swift_iban]: IBAN registry and validation rules. https://www.swift.com/swift-resource/22851/download
[^anna_luhn]: ISIN checksum algorithm (Annex C). https://www.anna-dsb.com/isin/
[^cusip]: CUSIP format and checksum rules. https://www.iso20022.org/milestone/22048/download
[^gleif]: GLEIF LEI structure and validation details. https://www.gleif.org/en/organizational-identity/introducing-the-legal-entity-identifier-lei/iso-17442-the-lei-code-structure
[^anna_crosswalk]: ISIN cross-reference (ANNA DSB) feeds for derivatives and debt instruments. https://www.anna-dsb.com/isin/
[^bic_lei]: GLEIF BIC-to-LEI relationship files. https://www.gleif.org/en/lei-data/lei-mapping/download-bic-to-lei-relationship-files
Norito.
[^csa_t1]: CSA amendments for Canadian institutional trade matching (T+1). https://www.osc.ca/en/securities-law/instruments-rules-policies/2/24-101/csa-notice-amendments-national-instrument-24-101-institutional-trade-matching-and-settlement-and
[^csdr]: ESMA CSDR settlement discipline / penalty mechanism updates. https://www.esma.europa.eu/sites/default/files/2024-11/ESMA74-2119945925-2059_Final_Report_on_Technical_Advice_on_CSDR_Penalty_Mechanism.pdf
[^india_t0]: SEBI circular on same-day settlement pilot. https://www.reuters.com/sustainability/boards-policy-regulation/india-markets-regulator-extends-deadline-same-day-settlement-plan-brokers-2025-04-29/

### тапшырыу-ҡаршы-түләү → `sese.023` XX| DvP яланы | ISO 20022 юл | Иҫкәрмәләр |
|-------------------------------------------------------------------------------------------------|
| `settlement_id` | `TxId` | Тотороҡло тормош циклы идентификаторы |
| `delivery_leg.asset_definition_id` (хәүефһеҙлек) | `SctiesLeg/FinInstrmId` | Канон идентификаторы (ИСИН, КУЗИП, ...) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | Ун еп; почетлы активтар теүәллеге |
| `payment_leg.asset_definition_id` (валюта) | `CashLeg/Ccy` | ISO валюта коды |
| `payment_leg.quantity` | `CashLeg/Amt` | Ун еп; түңәрәкләнгән бер һанлы спец |
| `delivery_leg.from` (һатыусы / тапшырыу партияһы) | `DlvrgSttlmPties/Pty/Bic` | БИК тапшырыу ҡатнашыусы *(иҫәп канон идентификаторы әлеге ваҡытта экспортҡа метамағлүмәттәр)* |
| `delivery_leg.from` иҫәбе идентификаторы | `DlvrgSttlmPties/Acct` | Ирекле форма; Norito метамағлүмәттәр теүәл иҫәп идентификаторы |
| `delivery_leg.to` (һатып алыусы / ҡабул итеү яҡ) | `RcvgSttlmPties/Pty/Bic` | Ҡатнашыусы ҡабул итеү БИК |
| `delivery_leg.to` иҫәп идентификаторы | `RcvgSttlmPties/Acct` | Ирекле форма; матчтар алыу иҫәп яҙмаһы идентификаторы |
| `plan.order` | `Plan/ExecutionOrder` | Anum: `DELIVERY_THEN_PAYMENT` йәки `PAYMENT_THEN_DELIVERY` X |
| `plan.atomicity` | `Plan/Atomicity` | Anum: `ALL_OR_NOTHING`, `COMMIT_FIRST_LEG`, `COMMIT_SECOND_LEG` |
| **Хәбәр маҡсаты** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (тапшырыу) йәки `RECE` (ҡабул итеү); көҙгөләр ниндәй аяҡ тапшырыусы яҡ башҡара. |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (түләүгә ҡаршы) йәки `FREE` (түләүһеҙ). |
| `delivery_leg.metadata`, `payment_leg.metadata` | `SctiesLeg/Metadata`, `CashLeg/Metadata` | Norito JSON UTF‐8 тип кодланған |

> * settlement квалификаторҙар** – күпер көҙгөләре баҙар практикаһы күсермәһен күсереп иҫәп-хисап шарт кодексы (`SttlmTxCond`), өлөшләтә иҫәп-хисап күрһәткестәре (`PrtlSttlmInd`), һәм башҡа факультатив квалификаторҙар Norito метамәғлүмәттәр `sese.023/025` тиклем, ҡасан бар. ISO тышҡы код исемлектәрендә баҫылған иҫәп-хисаптарҙы үтәү, шулай итеп, тәғәйенләнеше CSD ҡиммәттәрҙе таный.

### Түләү-ҡаршы-түләү финанслау → `pacs.009`

Аҡса-аҡса өсөн аяҡтар, тип финанслау PvP инструкцияһы FI-FI кредит булараҡ бирелә .
күсерә. Күпер был түләүҙәрҙе аннотациялай шулай аҫҡы системаларҙы таный
улар ҡиммәтле ҡағыҙҙар иҫәпләшеүен финанслай.| PvP финанслау өлкәһе | ISO 20022 юл | Иҫкәрмәләр |
|------------------------------------------------------------------------------------------------------|
| `primary_leg.quantity` / {сүмәлә, валюта} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` | Күләм/валюта инициаторҙан дебет. |
| Контраст агент идентификаторҙары | `InstgAgt`, `InstdAgt` | BIC/LEI ебәреп һәм ҡабул итеү агенттары. |
| Ҡасаба маҡсаты | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` | Ҡиммәтле ҡағыҙҙар менән бәйле PvP финанслау өсөн `SECU` өсөн ҡуйылған. |
| Norito метамағлүмәттәр (иҫәп ids, FX мәғлүмәттәре) | `CdtTrfTxInf/SplmtryData` | Тулы CountionId йөрөтә, FX ваҡыт маркалары, башҡарыу планы кәңәштәре. |
| Инструкция идентификаторы / йәшәү циклы бәйләү | `CdtTrfTxInf/PmtId/InstrId`, `CdtTrfTxInf/RmtInf` | Norito `settlement_id` матчтары шулай аҡса аяҡтарын ҡиммәтле ҡағыҙҙар яғы менән яраштырыу. |

JavaScript SDK’s ISO күпере был талап менән тура килә, ғәҙәттәгесә,
`pacs.009` категорияһы маҡсаты `SECU` тиклем; шылтыратыусылар уны икенсеһе менән өҫтөнә ала ала
дөрөҫ ISO коды ҡиммәтле ҡағыҙҙар булмаған кредит күсермәләрен сығара, әммә дөрөҫ түгел
ҡиммәттәре алдан кире ҡағыла.

Әгәр инфраструктура асыҡ ҡиммәтле ҡағыҙҙар раҫлау талап итә, күпер .
дауам итә, `sese.025` сығарыу, әммә был раҫлау ҡиммәтле ҡағыҙҙар аяҡ сағылдыра
статусы (мәҫәлән, `ConfSts = ACCP`) түгел, ә PvP “маҡсат”.

### Түләү-ҡаршы-түләү раҫлау → `sese.025`

| PvP яланы | ISO 20022 юл | Иҫкәрмәләр |
|------------------------------------------------------------------|----------------|
| `settlement_id` | `TxId` | Тотороҡло тормош циклы идентификаторы |
| `primary_leg.asset_definition_id` | `SttlmCcy` | Валюта коды өсөн беренсел аяҡ |
| `primary_leg.quantity` | `SttlmAmt` | Инициатор тарафынан тапшырылған сумма |
| `counter_leg.asset_definition_id` | `AddtlInf` (JSON файҙалы йөк) | Ҡаршы валюта коды өҫтәмә мәғлүмәткә индерелгән |
| `counter_leg.quantity` | `SttlmQty` | Ҡаршы сумма |
| `plan.order` | `Plan/ExecutionOrder` | Шул уҡ enum DvP тип ҡуйылған |
| `plan.atomicity` | `Plan/Atomicity` | Шул уҡ enum DvP тип ҡуйылған |
| `plan.atomicity` статусы (`ConfSts`) | `ConfSts` | `ACCP` тап килгәндә; күпер етешһеҙлектәр кодтарын кире ҡағыу тураһында сығара |
| Контраст идентификаторҙары | `AddtlInf` JSON | Ағымдағы күпер сериализациялары тулы Иҫәп яҙмаһы/БИК кортеждары метамағлүмәттәрҙә |

Миҫал (CLI ISO алдан ҡарау менән бәйләнештәр, тотоу, һәм баҙар МИК):

```sh
iroha app settlement dvp \
  --settlement-id DVP-FIXTURE-1 \
  --delivery-asset security#equities \
  --delivery-quantity 500 \
  --delivery-from i105... \
  --delivery-to i105... \
  --payment-asset usd#fi \
  --payment-quantity 1050000 \
  --payment-from i105... \
  --payment-to i105... \
  --delivery-instrument-id US0378331005 \
  --place-of-settlement-mic XNAS \
  --partial-indicator npar \
  --hold-indicator \
  --settlement-condition NOMC \
  --linkage WITH:PACS009-CLS \
  --linkage BEFO:SUBST-PAIR-B \
  --iso-xml-out sese023_preview.xml
```

### Репо залог алмаштырыу → `colr.007`| Репо яланы / контекст | ISO 20022 юл | Иҫкәрмәләр |
|--------------------------------------------------------------------------|------------------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` | Репо контракт идентификаторы |
| Залог алмаштырыу Tx идентификаторы | `TxId` | Алмаштырыу өсөн генерацияланған |
| Оригиналь коллатераль күләм | `Substitution/OriginalAmt` | Матчтар алмаштырыу алдынан залог вәғәҙә итте |
| Оригиналь коллатераль валюта | `Substitution/OriginalCcy` | Валюта коды |
| Алмаштырыу залог күләме | `Substitution/SubstituteAmt` | Алмаштырыу суммаһы |
| Алмаштырыу залог валютаһы | `Substitution/SubstituteCcy` | Валюта коды |
| Һөҙөмтәле дата (идара итеү маржаһы графигы) | `Substitution/EffectiveDt` | ISO дата (ЙЫЙ-ММ-ДД) |
| Стрижка классификацияһы | `Substitution/Type` | Әлеге ваҡытта идара итеү сәйәсәтенә нигеҙләнеп, `FULL` йәки `PARTIAL` XX |
| Идара итеү сәбәбе / сәс-ҡырҡылған иҫкәрмә | `Substitution/ReasonCd` | Опциональ, идара итеү рационализацияһын йөрөтә |
| Стрижный ҙурлыҡ | `Substitution/Haircut` | Һан; карталарын алмаштырыу ваҡытында ҡулланылған сәс ҡырҡыу |
| Төп/алмаштырыу приборы идентификаторҙары | `Substitution/OriginalFinInstrmId`, `Substitution/SubstituteFinInstrmId` | Һәр аяҡ өсөн махсус рәүештә ИСИН/КУЗИП |

### Финанслау һәм белдереүҙе

| Iroha контекст | ISO 20022 хәбәр | Картинг урыны |
|-------------------------------------------------------------------- |
| Репо аҡса аяҡ тоҡандырыу / сүкеш | `pacs.009` | `IntrBkSttlmAmt`, `IntrBkSttlmCcy`, `IntrBkSttlmDt`, `InstgAgt`, `InstdAgt` DvP/PvP аяҡтарынан халыҡ йәшәгән |
| Ҡалғандан һуңғы белдереүҙе | `camt.054` | Түләү аяҡ хәрәкәттәре теркәлгән буйынса `Ntfctn/Ntry[*]`; күпер инъекциялары баш китабы/иҫәп метамағлүмәттәре `SplmtryData` X |

### Ҡулланыу иҫкәрмәләре* Бөтә суммалар Norito һанлы ярҙамсылары ярҙамында сериялаштырыла (`NumericSpec`)
  активтарҙы билдәләү буйынса масштаблы тура килгән тәьмин итеү өсөн.
* `TxId` ҡиммәттәре `Max35Text` — UTF-8 оҙонлоғон ≤35 символдарҙы үтәй.
  экспортлау ISO 20022 хәбәрҙәр.
* БИК 8 йәки 11 ҙур хәреф-һанлы символдар булырға тейеш (ISO9362); ҡабул итмәҫкә
  Norito метамағлүмәттәр, был чекты үтәмәй, түләүҙәр йәки иҫәп-хисап сығарыр алдынан
  раҫлауҙары.
* Иҫәп идентификаторҙары (ScountId / ChainId) өҫтәмәгә экспортлана
  метамағлүмәттәр шулай ҡабул итеү ҡатнашыусылар үҙҙәренең урындағы баш китабына ҡаршы яраштырырға мөмкин.
* `SupplementaryData` канонлы JSON булырға тейеш (UTF‐8, сорттарға асҡыстар, JSON-тыуған
  ҡасыу). SDK ярҙамсылары был шулай ҡултамғаларҙы үтәй, телеметрия хештары, һәм ISO .
  файҙалы йөк архивтары тергеҙеүҙәр буйынса детерминистик булып ҡала.
* Валюта суммалары ISO4217 фракция һандарын үтәй (мәҫәлән, JPY 0 бар.
  унлыҡ, АҠШ доллары 2); күпер ҡыҫҡыстары Norito һанлы теүәллек ярашлы.
* CLI ҡасаба ярҙамсылары (`iroha app settlement ... --atomicity ...`) хәҙер сыға
  Norito инструкциялары, уларҙы башҡарыу пландары 1:1 картаһы `Plan/ExecutionOrder` һәм
  `Plan/Atomicity` өҫтә.
* ISO ярҙамсыһы (`ivm::iso20022`) өҫтә күрһәтелгән һәм кире ҡағыу яландарын раҫлай
  хәбәрҙәр, унда DvP/PvP аяҡтары һанлы характеристикалар йәки контрагент үҙ-ара мөнәсәбәтен боҙа.

### SDK Төҙөүсе ярҙамсылары

- JavaScript SDK хәҙер `buildPacs008Message` /
  `buildPacs009Message` X (ҡара: `javascript/iroha_js/src/isoBridge.js`) шулай клиент
  автоматлаштырыу структуралы ҡасаба метамағлүмәттәрен үҙгәртә ала (BIC/LEI, IBANs,
  маҡсатлы кодтар, өҫтәмә Norito яландар) детерминистик пач XML
  был ҡулланманан картаға төшөрөү ҡағиҙәләрен ҡабаттан тормошҡа ашырыуһыҙ.
- Ике ярҙамсы ла асыҡтан-асыҡ `creationDateTime` (ISO‐8601 ваҡыт бүлкәт менән) талап итә.
  шулай итеп, операторҙар тейеш еп детерминистик ваҡыт маркаһы уларҙы эш ағымы урынына
  рөхсәт итеү тураһында SDK стандарт стена сәғәт ваҡыт.
- `recipes/iso_bridge_builder.mjs` күрһәтә, нисек был ярҙамсыларҙы сымға 2012 йылға 1800 й.
  а CLI, тип берләштерә мөхит үҙгәртеүселәр йәки JSON конфиг файлдарын, баҫтырып сығара
  генерацияланған XML, һәм теләһәгеҙ, уны Torii (`ISO_SUBMIT=1`), ҡабаттан ҡулланыуға тапшыра
  шул уҡ көтөү каденцияһы кеүек ISO күпер рецепты.


### Һылтанмалар

- LuxCSD / Clerstrestrem ISO 20022 иҫәп-хисап миҫалдары күрһәтеү `SttlmTpAndAddtlParams/SctiesMvmntTp` (Norito) һәм `Pmt` . (`APMT`/`FREE` X).
- Clearstrestrest DCP спецификацияһы ҡаплаған иҫәп-хисап квалификаторҙары (`SttlmTxCond`, `PrtlSttlmInd`).
- SWIFT PMPG етәкселеге кәңәш `pacs.009` менән `CtgyPurp/Cd = SECU` өсөн ҡиммәтле ҡағыҙҙар менән бәйле PvP финанслау.
- ISO 20022 хәбәрҙе билдәләү өсөн отчеттар өсөн идентификатор оҙонлоғо сикләүҙәре (БИК, Max35Текст).[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)
- АННА DSB ISIN форматында һәм чемпионат суммаһы буйынса етәкселек.[5](https://www.anna-dsb.com/isin/)

### Кәңәштәр ҡулланыу- Һәр ваҡыт тейешле Norito өҙөк йәки CLI командаһы йәбештереү, шулай итеп, LLM тикшерергә мөмкин .
  теүәл ялан исемдәре һәм һанлы шкалалар.
- Һорау цитаталары (`provide clause references`) өсөн ҡағыҙ эҙҙәрен һаҡлау өсөн .
  үтәү һәм аудитор тикшерергә.
- Яуап резюмеһын `docs/source/finance/settlement_iso_mapping.md`-та тотоу
  (йәки бәйләнгән ҡушымталар) шуға күрә буласаҡ инженерҙарға эҙләүҙе ҡабатларға кәрәкмәй.

## Ваҡиғалар заказы плейбуктары (ISO 20022 ↔ Norito күпере)

### Сценарио А — Залог алмаштырыу (Репо / Залог)

**Ҡатнашыусылар:** залог биргән/алыусы (һәм/йәки агенттар), һаҡлаусы(s), CSD/T2S  
**Ваҡыт:** баҙар өҙөлгән һәм T2S көнө/төн циклдары; ике аяҡты оркестрлаштырыу, шуға күрә улар бер үк ҡасаба тәҙрәһе эсендә тамамлана.

#### Хәбәр хореографияһы
.  
2. `colr.011` Залог алмаштырыу яуап → ҡабул итеү/кире кире ҡағыу (факультатив кире ҡағыу сәбәбе).  
3. `colr.012` залог алмаштырыу раҫлау → алмаштырыу килешүен раҫлай.  
4. `sese.023` инструкциялары (ике аяҡ):  
   - Ҡайтарыу төп залог (`SctiesMvmntTp=DELI`, `Pmt=FREE`, Norito).  
   - тапшырыу алмаштырыусы залог (`SctiesMvmntTp=RECE`, `Pmt=FREE`, `SctiesTxTp=COLI`).  
   Парҙы бәйләгеҙ (аҫта ҡарағыҙ).  
5. `sese.024` статус кәңәштәре (ҡабул ителгән, тап килгән, көтөп, уңышһыҙлыҡҡа осраған, кире ҡағылған).  
6. `sese.025` раҫлауҙары бер тапҡыр заказ бирелгән.  
7. Опциональ касса дельта (гонорар/стрижка) → `pacs.009` FI-FI кредит тапшырыу менән `CtgyPurp/Cd = SECU`; `pacs.002` аша статус, `pacs.004` аша ҡайтарыуҙар.

#### Кәрәкле таныу / статустар
- Транспорт кимәле: шлюздар `admi.007` X йәки бизнес эшкәрткәнгә тиклем кире ҡаға ала.  
- Ҡасаба йәшәү циклы: `sese.024` (эшкәртергә статус + сәбәп кодтары), `sese.025` (һуңғы).  
- Аҡса яғы: `pacs.002` X (`PDNG`, `ACSC`, Norito.

#### Шартлылыҡ / разредка ялан
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) сылбырлы ике күрһәтмә.  
- `SttlmParams/HldInd` критерийҙары ҡәнәғәтләнгәнгә тиклем үткәреү өсөн; `sese.030` аша сығарыу (`sese.031` статусы).  
- `SttlmParams/PrtlSttlmInd` өлөшләтә ҡасабаны контролдә тотоу өсөн (`NPAR`, `PART`X, Norito, `PARQ`).  
- `SttlmParams/SttlmTxCond/Cd` баҙарға хас шарттар өсөн (`NOMC` һ.б.).  
- Һөҙөмтәле T2S шартлы ҡиммәтле ҡағыҙҙар тапшырыу (CoSD) ҡағиҙәләре ҡасан ярҙам итә.

#### Һылтанмалар
- SWIFT коллатераль идара итеү МДР (`colr.010/011/012`).  
- CSD/T2S ҡулланыу етәкселәре (мәҫәлән, DNB, ECB Insights) бәйләү һәм статустар өсөн.  
- SMPG иҫәп-хисап практикаһы, Clearstream DCP ҡулланмалары, ASX ISO оҫтаханалары.

### Сценарио В — FX тәҙрә боҙоу (PvP финанслау етешһеҙлеге)

**Ҡатнашыусылар:** контрагент һәм аҡса агенттары, ҡиммәтле ҡағыҙҙар һаҡсыһы, CSD/T2S  
**Ваҡыт:** FX PvP тәҙрәләр (CLS/ике яҡлы) һәм CSD өҙөктәре; ҡиммәтле ҡағыҙҙар аяҡтарын һаҡлау өсөн көтөп аҡса раҫлау.#### Хәбәр хореографияһы
1. Norito FI-FI кредит тапшырыу өсөн валюта менән `CtgyPurp/Cd = SECU`; статус аша `pacs.002`; иҫкә төшөрөп/Cancel аша `camt.056`/`camt.029`; әгәр инде төпләнгән, `pacs.004` ҡайтарыу.  
2. [^iso_mdr]: ISO 20022 external code list (CUSIP `CUSP`) and MDR Part 2. https://www.iso20022.org/milestone/22048/download DvP инструкцияһы(s) менән `HldInd=true` шулай ҡиммәтле ҡағыҙҙар аяҡ аҡса раҫлау өсөн көтә.  
3. Йәшәү циклы `sese.024` иҫкәртмәләр (ҡабул ителгән/тапшырыу/күҙаллау).  
4. Әгәр ҙә икеһе лә `pacs.009` аяҡтары `ACSC` еткәнсе тәҙрә → `sese.030` менән сығарыу ваҡыты үткәнсе → `sese.031` (мод статусы) → `sese.025` (раҫлау) менән сыға.  
5. Әгәр FX тәҙрә боҙолған → отмена/иҫәпкә аҡса (`camt.056/029` йәки `pacs.004`) һәм ҡиммәтле ҡағыҙҙарҙы юҡҡа сығарыу (`sese.020` + `sese.027`, йәки `sese.026` кире ҡайтарыу, әгәр инде раҫланһа, баҙар ҡағиҙәһе).

#### Кәрәкле таныу / статустар
- Cash: `pacs.002` (`PDNG`, `ACSC`, `RJCT`), `pacs.004` ҡайтарыу өсөн.  
- Ҡиммәтле ҡағыҙҙар: Norito ( `NORE` кеүек көтөлгән/уңышһыҙлыҡ сәбәптәре, Norito), `sese.025`.  
- Транспорт: `admi.007` / шлюз бизнес эшкәрткәнсе кире ҡаға.

#### Шартлылыҡ / разредка ялан
- `SttlmParams/HldInd` + `sese.030` релиз/аманат уңыш/уңышһыҙлыҡҡа бәйле.  
- [^iso_mic]: ISO 10383 Market Identifier Code maintenance agency. https://www.iso20022.org/market-identifier-codesX ҡиммәтле ҡағыҙҙарҙы аҡса аяҡҡа бәйләү өсөн бәйләү өсөн.  
- T2S CoSD ҡағиҙәһе, әгәр шартлы тапшырыу ҡулланыу.  
- [^iso_4217]: ISO 4217 currency and minor-units table (SIX). https://www.six-group.com/en/products-services/financial-information/market-reference-data/data-standards.html, ниәтләнмәгән өлөштәрҙе булдырмау өсөн.  
- `pacs.009`, `CtgyPurp/Cd = SECU` флагтары ҡиммәтле ҡағыҙҙар менән бәйле финанслау.

#### Һылтанмалар
- PMPG / CBPR+ ҡиммәтле ҡағыҙҙар процестарында түләүҙәр өсөн етәкселек.  
- SMPG иҫәп-хисап практикаһы, T2S тураһында төшөнсә бәйләү/тотоу.  
- Clearstream DCP ҡулланмалары, ECMS документацияһы өсөн хеҙмәтләндереүҙе хәбәрҙәр.

### pacs.004 ҡайтарыу картаһы иҫкәрмәләр

- ҡайтарыу ҡорамалдары хәҙер нормаль `ChrgBr` (Norito/Norito) һәм 18NI0000000345X тип фашланған милекселек сәбәптәре. атрибуция һәм оператор кодтары XML конвертын ҡабаттан анализламайынса.
- `DataPDU` конверттары эсендә ApCHdr ҡултамғаһы блоктары ашау өҫтөндә иғтибарға алынмай ҡала; аудиттар канал провенансҡа таянырға тейеш, ә XMLDSIG яландарында түгел.

### Күпер өсөн оператив тикшерелгән исемлек
- Үрҙәге хореографияны үтәү (залог: `colr.010/011/012 → sese.023/024/025`; FX боҙоу: `pacs.009 (+pacs.002) → sese.023 held → release/cancel`).  
- `sese.024`/`sese.025` статустары һәм `pacs.002` һөҙөмтәләрен ҡапҡа сигналдары булараҡ дауалау; `ACSC` триггерҙары сығарыу, `RJCT` көстәре өҙөлә.  
- `HldInd`, `Lnkgs`, `PrtlSttlmInd`, `SttlmTxCond` һәм CoSD ҡағиҙәләре аша шартлы шартлы тапшырыуҙы код.  
- Ҡулланыу `SupplementaryData` тышҡы идентификаторҙарҙы корреляциялау өсөн (мәҫәлән, UETR `pacs.009` өсөн) кәрәк булғанда.  
- Параметризациялау тотоп/ялған ваҡыт баҙар календары буйынса/ҡырҡыу-офф; `sese.030` мәс. 18NI000000361X-ты юҡҡа сығарыу срогы алдынан, кәрәк саҡта кире ҡайтарыуҙарға өҙгөс.

### Өлгө ISO 20022 Түләүҙәр (Аннот)

#### Залог алмаштырыу пары (`sese.023`) инструкция бәйләнеше менән

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>SUBST-2025-04-001-A</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>FREE</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
      <sese:SttlmTxCond>
        <sese:Cd>NOMC</sese:Cd>
      </sese:SttlmTxCond>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>SUBST-2025-04-001-B</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Original collateral FoP back to giver -->
    <sese:FctvSttlmDt>2025-04-03</sese:FctvSttlmDt>
    <sese:SctiesMvmntDtls>
      <sese:SctiesId>
        <sese:ISIN>XS1234567890</sese:ISIN>
      </sese:SctiesId>
      <sese:Qty>
        <sese:QtyChc>
          <sese:Unit>1000</sese:Unit>
        </sese:QtyChc>
      </sese:Qty>
    </sese:SctiesMvmntDtls>
  </sese:SctiesSttlmTxInstr>
</sese:Document>
````SUBST-2025-04-001-B` (FoP алыу өсөн алмаштырыу коллатеры) менән `SctiesMvmntTp=RECE`, `Pmt=FREE`, һәм Norito бәйләнеше Norito-ҡа тиң. Ике аяҡты ла тап килгән `sese.030` менән сығарыу бер тапҡыр алмаштырыу раҫланды.

### Ҡиммәтле ҡағыҙҙар аяҡ өҫтөндә көтөп FX раҫлау (`sese.023`XX + `sese.030`)

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>APMT</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>PACS009-USD-CLS01</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Remaining settlement details omitted for brevity -->
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```

Бер тапҡыр ҙа `pacs.009` аяҡтары Norito-ға барып етә:

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.030.001.04">
  <sese:SctiesSttlmCondModReq>
    <sese:ReqDtls>
      <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
      <sese:ChngTp>
        <sese:Cd>RELE</sese:Cd>
      </sese:ChngTp>
    </sese:ReqDtls>
  </sese:SctiesSttlmCondModReq>
</sese:Document>
```

`sese.031` раҫлай, был релиз, унан һуң `sese.025` ҡиммәтле ҡағыҙҙар аяҡ заказ бирелгән.

### PvP финанслау аяҡ (`pacs.009` ҡиммәтле ҡағыҙҙар маҡсаты менән)

```xml
<pacs:Document xmlns:pacs="urn:iso:std:iso:20022:tech:xsd:pacs.009.001.08">
  <pacs:FinInstnCdtTrf>
    <pacs:GrpHdr>
      <pacs:MsgId>PACS009-USD-CLS01</pacs:MsgId>
      <pacs:IntrBkSttlmDt>2025-05-07</pacs:IntrBkSttlmDt>
    </pacs:GrpHdr>
    <pacs:CdtTrfTxInf>
      <pacs:PmtId>
        <pacs:InstrId>DVP-2025-05-CLS01-USD</pacs:InstrId>
        <pacs:EndToEndId>SETTLEMENT-CLS01</pacs:EndToEndId>
      </pacs:PmtId>
      <pacs:PmtTpInf>
        <pacs:CtgyPurp>
          <pacs:Cd>SECU</pacs:Cd>
        </pacs:CtgyPurp>
      </pacs:PmtTpInf>
      <pacs:IntrBkSttlmAmt Ccy="USD">5000000.00</pacs:IntrBkSttlmAmt>
      <pacs:InstgAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKUS33XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstgAgt>
      <pacs:InstdAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKGB22XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstdAgt>
      <pacs:SplmtryData>
        <pacs:Envlp>
          <nor:NoritoBridge xmlns:nor="urn:norito:settlement">
            <nor:SettlementId>DVP-2025-05-CLS01</nor:SettlementId>
            <nor:Atomicity>ALL_OR_NOTHING</nor:Atomicity>
          </nor:NoritoBridge>
        </pacs:Envlp>
      </pacs:SplmtryData>
    </pacs:CdtTrfTxInf>
  </pacs:FinInstnCdtTrf>
</pacs:Document>
```.

`pacs.002` түләү статусы күҙәтә (Norito = раҫланған, `RJCT` = кире ҡағыу). Әгәр тәҙрә боҙолһа, `camt.056`/`camt.029` аша иҫкә төшөрөп йәки `pacs.004` ебәрегеҙ, уларҙы кире ҡайтарыу өсөн аҡса ҡайтарыу өсөн.