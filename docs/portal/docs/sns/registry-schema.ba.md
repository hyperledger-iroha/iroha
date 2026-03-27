---
lang: ba
direction: ltr
source: docs/portal/docs/sns/registry-schema.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5307c80eba9ab93d4522c3e88485fa4d24f7f04903b7aea30b05e880e2c096b0
source_last_modified: "2026-01-28T17:11:30.697638+00:00"
translation_last_reviewed: 2026-02-07
id: registry-schema
title: Sora Name Service Registry Schema
sidebar_label: Registry schema
description: Norito data structures, lifecycle rules, and event contracts for SNS registry smart contracts (SN-2a).
translator: machine-google-reviewed
---

:::иҫкәртергә канонлы сығанаҡ
Был бит көҙгө `docs/source/sns/registry_schema.md` һәм хәҙер хеҙмәт итә
канон портал күсермәһе. Сығанаҡ файлы тәржемә яңыртыу өсөн ҡала.
::: 1990 й.

# Сора исеме хеҙмәте реестр схемаһы (SN-2a)

**Статус:** 2026-03-24 - SNS программаһын тикшерергә тапшырылған.  
**Родка картаһы һылтанмаһы:** SN-2a “Зинһар схемаһы & һаҡлау макеты”  
**Сода:** Каноник Norito структураларын билдәләү, йәшәү циклы дәүләттәре, һәм Сора исеме хеҙмәте өсөн ваҡиғаларҙы сығарҙы (SNS) шулай реестр һәм регистратор тормошҡа ашырыуҙар контракттар буйынса детерминистик ҡала, SDKs, һәм шлюздар.

Был документ SN-2a өсөн схеманы тапшырыуҙы тамамлай: аныҡлап:

1. Идентификаторҙар һәм хешинг ҡағиҙәләре (`SuffixId`, I18NI000000018X, селектор сығарыу).
.
3. Детерминистик реплей өсөн һаҡлау макеты һәм индекс префикстары.
4. Дәүләт машинаһы ҡаплау теркәү, яңыртыу, мәрхәмәт/ҡотҡарыу, туңдырыу, һәм ҡәбер таштары.
5. DNS/ҡапҡа автоматлаштырыуы ҡулланған канонлы ваҡиғалар.

## 1. Идентификаторҙар & Хэшинг

| Идентификатор | Тасуирлама | Дериция |
|----------|--------------|----------- |
| `SuffixId` X (I18NI000000020X) | Юғары кимәлдәге ялғауҙар өсөн теркәү киңлеге идентификаторы (`.sora`, `.nexus`, `.dao`). [I18NI000000024X] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Идара итеү тауышы менән бирелгән; I18NI000000025X һаҡлана. |
| `SuffixSelector` | Канонлы струнный формаһы ялғау (ASCII, бәләкәй осраҡта). | Миҫал: `.sora` → `sora`. |
| `NameSelectorV1` | Теркәүсе ярлыҡ өсөн бинар селектор. | I18NI000000300Х. Ярлыҡ — NFC + Normv1 өсөн бәләкәй генә осраҡта. |
| I18NI000000031X (I18NI000000032X) | Беренсел эҙләү асҡысы ҡулланылған контракттар, ваҡиғалар, һәм кэш. | `blake3(NameSelectorV1_bytes)`. |

Детерминизм талаптары:

- Ярлыҡтар Normv1 аша нормалаштырылған (UTS-46 ҡәтғи, STD3 ASCII, NFC). Ҡулланыусы ептәрен хишлау алдынан нормалаштырырға кәрәк.
- Запас ярлыҡтар (`SuffixPolicyV1.reserved_labels`-тан) бер ҡасан да реестрға инмәй; идара итеү-тик өҫтөнлөктәре I18NI000000035X сараларын сығара.

## 2. I18NT000000002X структуралары

### 2.1 НамеерекордВ111.

| Ялан | Тип | Иҫкәрмәләр |
|------|------|-------|
| `suffix_id` | `u16` | Һылтанмалар I18NI000000038X. |
| `selector` | `NameSelectorV1` | Сеймал һайлаусы байттар өсөн аудит/оставка. |
| `name_hash` | `[u8; 32]` | Карталар/ваҡиғалар өсөн төп. |
| `normalized_label` | `AsciiString` | Кеше уҡый торған ярлыҡ (Нормв1 пост). |
| `display_label` | `AsciiString` | Стюард менән тәьмин ителгән корпус; опциональ косметика. |
| `owner` | `AccountId` | Контроль яңыртыу/трансферҙар. |
| `controllers` | `Vec<NameControllerV1>` | Һылтанмалар маҡсатлы иҫәп-хисап адрестары, хәл итеүселәр, йәки ҡушымта метамағлүмәттәр. |
| `status` | `NameStatus` | Йәшәү циклы флагы (ҡара: 4-се бүлек). |
| `pricing_class` | `u8` | Индексы суффикс хаҡтар ярустары (стандарт, премиум, запас). |
| `registered_at` | `Timestamp` | Тәүге активацияны блоклау ваҡыт билдәһе. |
| `expires_at` | `Timestamp` | Түләүле сроктың аҙағы. |
| `grace_expires_at` | `Timestamp` | Авто-яңыртыу рәхмәтенең аҙағы (ғәҙәти +30days). |
| `redemption_expires_at` | `Timestamp` | Ҡотҡарыу тәҙрәһенең аҙағы (ғәҙәти +60 көн). |
| `auction` | `Option<NameAuctionStateV1>` | Хәҙерге ваҡытта голландтарҙы яңынан асҡанда йәки премиум-аукциондар әүҙем була. |
| `last_tx_hash` | `Hash` | Был версияны етештергән транзакцияға детерминистик күрһәткес. |
| `metadata` | `Metadata` | Арбитарий регистратор метамағлүмәттәр (текст яҙмалары, дәлилдәр). |

Ярҙамсы структуралар:

```text
Enum NameStatus {
    Available,          // derived, not stored on-ledger
    PendingAuction,
    Active,
    GracePeriod,
    Redemption,
    Frozen(NameFrozenStateV1),
    Tombstoned(NameTombstoneStateV1)
}

Struct NameFrozenStateV1 {
    reason: String,
    until_ms: u64,
}

Struct NameTombstoneStateV1 {
    reason: String,
}

Struct NameControllerV1 {
    controller_type: ControllerType,   // Account, ResolverTemplate, ExternalLink
    account_address: Option<AccountAddress>,   // Serialized as canonical `0x…` hex in JSON
    resolver_template_id: Option<String>,
    payload: Metadata,                 // Extra selector/value pairs for wallets/gateways
}

Struct TokenValue {
    asset_id: AsciiString,
    amount: u128,
}

Enum ControllerType {
    Account,
    Multisig,
    ResolverTemplate,
    ExternalLink
}

Struct NameAuctionStateV1 {
    kind: AuctionKind,             // Vickrey, DutchReopen
    opened_at_ms: u64,
    closes_at_ms: u64,
    floor_price: TokenValue,
    highest_commitment: Option<Hash>,  // reference to sealed bid
    settlement_tx: Option<Json>,
}

Enum AuctionKind {
    VickreyCommitReveal,
    DutchReopen
}
``` X

### 2.2 СуффиксПолициВ1.

| Ялан | Тип | Иҫкәрмәләр |
|------|------|-------|
| `suffix_id` | `u16` | Беренсел асҡыс; сәйәси версиялары буйынса тотороҡло. |
| `suffix` | `AsciiString` | мәҫәлән, `sora`. |
| `steward` | `AccountId` | Стюард идара итеү уставында билдәләнгән. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| I18NI0000081X | `AsciiString` | Ғәҙәттән тыш хәл активы идентификаторы (мәҫәлән, `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | Балалы хаҡтар коэффициенттары һәм оҙайлылыҡ ҡағиҙәләре. |
| `min_term_years` | `u8` | Иҙән өсөн һатып алынған срокҡа ҡарамаҫтан, ярус өҫтөнлөктәре. |
| `grace_period_days` | `u16` | Ғәҙәттәгесә 30. |
| `redemption_period_days` | `u16` | Ғәҙәттәгесә 60. |
| `max_term_years` | `u8` | Максималь алдан яңыртыу оҙонлоғо. |
| `referral_cap_bps` | `u16` | <=1000 (10%) бер устав. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Идара итеү исемлеге менән тәьмин итеү күрһәтмәләре. |
| `fee_split` | `SuffixFeeSplitV1` | Ҡаҙна / стеуард / йүнәлтмә өлөшө (нигеҙендә нөктәләр). |
| `fund_splitter_account` | `AccountId` | Эскроу тотҡан иҫәп + аҡса тарата. |
| `policy_version` | `u16` | Һәр үҙгәрешкә өҫтәмә. |
| `metadata` | `Metadata` | Киңәйтелгән ноталар (КПИ килешүе, үтәү doc хештар). |

```text
Struct PriceTierV1 {
    tier_id: u8,
    label_regex: String,       // RE2-syntax pattern describing eligible labels
    base_price: TokenValue,    // Price per one-year term before suffix coefficient
    auction_kind: AuctionKind, // Default auction when the tier triggers
    dutch_floor: Option<TokenValue>,
    min_duration_years: u8,
    max_duration_years: u8,
}

Struct ReservedNameV1 {
    normalized_label: AsciiString,
    assigned_to: Option<AccountId>,
    release_at_ms: Option<u64>,
    note: String,
}

Struct SuffixFeeSplitV1 {
    treasury_bps: u16,     // default 7000 (70%)
    steward_bps: u16,      // default 3000 (30%)
    referral_max_bps: u16, // optional referral carve-out (<= 1000)
    escrow_bps: u16,       // % routed to claw-back escrow
}
```

### 2.3 Килем һәм ҡасаба яҙмалары

| Струк | Яландар | Маҡсат |
|-------|--------|---------|
| `RevenueShareRecordV1` | I18NI000000107X, I18NI000000108X, `treasury_amount`, I18NI000000110111X, I18NI000000111X, I18NI000000112X, I18NI000000113X, `tx_hash`. | Детерминистик яҙма маршрутлаштырылған түләүҙәр бер иҫәп-хисап эпохаһы (аҙналыҡ). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, I18NI000000121X. | Түләү яҙмалары (теркәү, яңыртыу, аукцион) һәр тапҡыр сығарылған. |

Бөтә I18NI000000122X яландары I18NT000000003X’s канонлы стационар стационар-нөктә кодлау менән валюта коды иғлан ителгән I18NI000000123X.

### 2.4 Теркәү ваҡиғалары

Канонлы ваҡиғалар DNS/шлюз автоматлаштырыу һәм аналитика өсөн реплей журналын тәьмин итә.

```text
Struct RegistryEventV1 {
    name_hash: [u8; 32],
    suffix_id: u16,
    selector: NameSelectorV1,
    version: u64,               // increments per NameRecord update
    timestamp: Timestamp,
    tx_hash: Hash,
    actor: AccountId,
    event: RegistryEventKind,
}

Enum RegistryEventKind {
    NameRegistered { expires_at: Timestamp, pricing_class: u8 },
    NameRenewed { expires_at: Timestamp, term_years: u8 },
    NameTransferred { previous_owner: AccountId, new_owner: AccountId },
    NameControllersUpdated { controller_count: u16 },
    NameFrozen(NameFrozenStateV1),
    NameUnfrozen,
    NameTombstoned(NameTombstoneStateV1),
    AuctionOpened { kind: AuctionKind },
    AuctionSettled { winning_account: AccountId, clearing_price: TokenValue },
    RevenueSharePosted { epoch_id: u64, treasury_amount: TokenValue, steward_amount: TokenValue },
    SuffixPolicyUpdated { policy_version: u16 },
}
```

Ваҡиғалар реплейлы журналға ҡушылырға тейеш (мәҫәлән, `RegistryEvents` домен) һәм шлюз каналдарына көҙгө шулай DNS кэштары SLA эсендә юҡҡа сыға.

## 3. Һаҡлау макеты & Индекс

| Асҡыс | Тасуирлама |
|-----|-------------|
| `Names::<name_hash>` | Беренсел карта `name_hash` тиклем I18NI000000127X тиклем. |
| `NamesByOwner::<AccountId, suffix_id>` | Икенсел индексы өсөн янсыҡ UI (сәғ. дуҫ). |
| `NamesByLabel::<suffix_id, normalized_label>` | Конфликттарҙы асыҡлау, власть детерминистик эҙләү. |
| `SuffixPolicies::<suffix_id>` | Һуңғы `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` тарихы. |
| `RegistryEvents::<u64>` | Ҡушымта-тик лог клавиатура бер монотоник арттырыу эҙмә-эҙлеклелек. |

Бөтә асҡыстар сериялы ҡулланып I18NT00000000004X кортеждары хеширование детерминистик һаҡлау өсөн хужалар. Индекс яңыртыуҙары беренсел яҙма менән бер рәттән атомик рәүештә барлыҡҡа килә.

## 4. Йәшәү циклы дәүләт машинаһы

| Дәүләт | Яҙыу шарттары | Рөхсәт ителгән күсештәр | Иҫкәрмәләр |
|------|------------------------------------|------- |
| Доступный | Ҡасан алынған I18NI000000135X юҡ. | `PendingAuction` (премиум), I18NI000000137X (стандарт реестр). | Доступность эҙләү индекстарҙы ғына уҡый. |
| ПендингАкция | `PriceTierV1.auction_kind` ≠ бер ниндәй ҙә булмағанда булдырылған. | `Active` X (аукцион төпләнә), `Tombstoned` (заявкалар юҡ). | Аукциялар `AuctionOpened` һәм I18NI000000142X сыға. |
| Әүҙем | Теркәү йәки яңыртыу уңышлы булды. | `GracePeriod`, https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md, `Tombstoned`. | `expires_at` диск күсеү. |
| ГрейсПериод | Автоматик яҡтан I18NI000000147X. | `Active` (ваҡытында яңыртыу), I18NI000000149X, `Tombstoned`. | Ғәҙәттәгесә +30days; һаман да хәл итә, әммә флаглы. |
| Ҡотҡарыу | `now > grace_expires_at`, әммә I18NI000000152X. | `Active` (һуң яңыртыу), `Tombstoned`. | Командалар өсөн штраф түләү талап ителә. |
| Туңдырылған | Идара итеү йәки опекун туңдырыу. | `Active` (ремедиациянан һуң), `Tombstoned`. | Контроллерҙарҙы күсерергә йәки яңырта алмай. |
| Ҡәберле | Ирекмәндәр капитуляция, даими бәхәс һөҙөмтәһе, йәки ҡотҡарыу ваҡыты үткән. | `PendingAuction` (нидерланд яңынан асылған) йәки ҡәбер таштары ҡала. | Ваҡиға I18NI000000158X аҡылды үҙ эсенә алырға тейеш. |

Дәүләт күсештәре тейешле `RegistryEventKind` X шулай итеп, аҫҡы кэштар когерент ҡала. Голландияны яңынан асыу аукциондарын индереүсе ҡәберле исемдәр `AuctionKind::DutchReopen` файҙалы йөкләмәһен беркетә.

## 5. Канонлы ваҡиғалар & ҡапҡа синхронизацияһы

Ҡапҡалар `RegistryEventV1`-ҡа яҙыла һәм DNS/SoraFS менән синхронлаштыра:

1. Һуңғы `NameRecordV1` ваҡиға эҙмә-эҙлегенә һылтанма яһалған.
.
3. Pinning яңыртылған зона мәғлүмәттәре аша SoraDNS эш ағымы һүрәтләнгән [`soradns_registry_rfc.md`](I18NU0000015X).

Ваҡиғалар тапшырыу гарантиялары:

- `NameRecordV1` *тейешле * тейеш һәр транзакция * тейеш теүәл бер ваҡиға менән ҡәтғи арттырыу `version`X.
- `RevenueSharePosted` ваҡиғалар һылтанмалы ҡасабалар сығарылған I18NI000000168X.
- Туңдырыу/туңдырмау/ҡәбер ташы ваҡиғалары I18NI000000169XX эсендә идара итеү артефакт хештары инә.

## 6. Миҫал I18NT000000005X Түләүҙәр

### 6.1 Исем-шәрифе Миҫал

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "<i105-account-id>",
    controllers: [
        NameControllerV1 {
            controller_type: Account,
            account_address: Some(AccountAddress("0x020001...")),
            resolver_template_id: None,
            payload: {}
        }
    ],
    status: Active,
    pricing_class: 0,
    registered_at: 1_776_000_000,
    expires_at: 1_807_296_000,
    grace_expires_at: 1_809_888_000,
    redemption_expires_at: 1_815_072_000,
    auction: None,
    last_tx_hash: 0xa3d4...c001,
    metadata: { "resolver": "wallet.default", "notes": "SNS beta cohort" },
}
```

### 6.2 СуффиксПолитика Миҫал

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "<i105-account-id>",
    status: Active,
    payment_asset_id: "61CtjvNd9T3THAR65GsMVHr82Bjc",
    pricing: [
        PriceTierV1 { tier_id:0, label_regex:"^[a-z0-9]{3,}$", base_price:"120 XOR", auction_kind:VickreyCommitReveal, dutch_floor:None, min_duration_years:1, max_duration_years:5 },
        PriceTierV1 { tier_id:1, label_regex:"^[a-z]{1,2}$", base_price:"10_000 XOR", auction_kind:DutchReopen, dutch_floor:Some("1_000 XOR"), min_duration_years:1, max_duration_years:3 }
    ],
    min_term_years: 1,
    grace_period_days: 30,
    redemption_period_days: 60,
    max_term_years: 5,
    referral_cap_bps: 500,
    reserved_labels: [
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("<i105-account-id>"), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "<i105-account-id>",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. Киләһе аҙымдар- **SN-2b (Регистратор API & идара итеү ҡармаҡтары):** был структураларҙы I18NT000000008X аша фашлай (Norito һәм JSON бәйләүҙәре) һәм идара итеү артефакттарына сым ҡабул итеү тикшерелеүе.
- **SN-3 (Аукция & теркәү двигателе):** ҡабаттан ҡулланыу I18NI0000000170X тормошҡа ашырыу өсөн коммит/асыу һәм голланд яңынан асылған логика.
- **SN-5 (Түләү һәм иҫәп-хисап):** рычаг I18NI0000000171X өсөн финанс ярашыу һәм отчет автоматлаштырыу отчет.

Һорауҙар йәки үҙгәрештәр тураһында запростар SNS юл картаһы яңыртыуҙары менән бер рәттән тапшырырға тейеш I18NI000000172X һәм көҙгө I18NI000000173X берләшкәндә.