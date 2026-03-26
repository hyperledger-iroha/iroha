---
lang: kk
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

:::ескерту Канондық дереккөз
Бұл бет `docs/source/sns/registry_schema.md` бейнесін көрсетеді және қазір ол ретінде қызмет етеді
канондық портал көшірмесі. Бастапқы файл аударма жаңартулары үшін қалады.
:::

# Sora Name қызмет тізілімінің схемасы (SN-2a)

**Күйі:** Жоба 2026-03-24 жасалды -- SNS бағдарламасын қарауға жіберілді  
**Жол картасы сілтемесі:** SN-2a «Тіркеу схемасы және сақтау схемасы»  
**Қолдану аясы:** Sora Name Service (SNS) үшін канондық Norito құрылымдарын, өмірлік цикл күйлерін және шығарылған оқиғаларды анықтаңыз, осылайша тізілім мен тіркеушіні іске асыру келісім-шарттар, SDK және шлюздерде детерминистік болып қалады.

Бұл құжат SN-2a үшін жеткізілетін схеманы келесілерді көрсету арқылы аяқтайды:

1. Идентификаторлар және хэштеу ережелері (`SuffixId`, `NameHash`, селектордың туындысы).
2. Norito атау жазбаларына, суффикс саясаттарына, баға деңгейлеріне, кірістерді бөлуге және тізбе оқиғаларына арналған құрылымдар/сандар.
3. Детерминирленген қайталау үшін сақтау орны және индекс префикстері.
4. Тіркеуді, жаңартуды, жеңілдікті/өтеуді, қатуды және құлпытастарды қамтитын мемлекеттік машина.
5. DNS/шлюз автоматтандыруы тұтынатын канондық оқиғалар.

## 1. Идентификаторлар және хэштеу

| Идентификатор | Сипаттама | Туынды |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Жоғарғы деңгейлі жұрнақтар үшін тізілімнің кең идентификаторы (`.sora`, `.nexus`, `.dao`). [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) ішіндегі жұрнақ каталогымен тураланған. | Басқару мүшелерінің дауыс беруімен тағайындалады; `SuffixPolicyV1` ішінде сақталады. |
| `SuffixSelector` | Жұрнақтың канондық жолдық түрі (ASCII, кіші әріп). | Мысал: `.sora` → `sora`. |
| `NameSelectorV1` | Тіркелген белгі үшін екілік селектор. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Белгі NFC + Normv1 үшін кіші әріп. |
| `NameHash` (`[u8;32]`) | Келісімшарттар, оқиғалар және кэштер пайдаланатын негізгі іздеу кілті. | `blake3(NameSelectorV1_bytes)`. |

Детерминизмге қойылатын талаптар:

- Белгілер Normv1 (UTS-46 қатаң, STD3 ASCII, NFC) арқылы қалыпқа келтіріледі. Кіріс пайдаланушы жолдарын хэштеу алдында қалыпқа келтіру керек.
- Сақталған белгілер (`SuffixPolicyV1.reserved_labels` бастап) ешқашан тізілімге кірмейді; тек басқаруды қайта анықтау `ReservedNameAssigned` оқиғаларын шығарады.

## 2. Norito құрылымдары

### 2.1 NameRecordV1

| Өріс | |түрі Ескертпелер |
|-------|------|-------|
| `suffix_id` | `u16` | Анықтамалар `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Аудит/отлад үшін өңделмеген селекторлық байттар. |
| `name_hash` | `[u8; 32]` | Карталар/оқиғалар кілті. |
| `normalized_label` | `AsciiString` | Адам оқи алатын жапсырма (Normv1 посты). |
| `display_label` | `AsciiString` | Басқарушы қамтамасыз ететін корпус; қосымша косметика. |
| `owner` | `AccountId` | Жаңартуларды/аударуды бақылайды. |
| `controllers` | `Vec<NameControllerV1>` | Мақсатты тіркелгі мекенжайларына, шешушілерге немесе қолданба метадеректеріне сілтеме жасайды. |
| `status` | `NameStatus` | Өмірлік цикл жалаушасы (4-бөлімді қараңыз). |
| `pricing_class` | `u8` | Жұрнақ баға деңгейлеріне индекс (стандартты, премиум, резервтелген). |
| `registered_at` | `Timestamp` | Бастапқы белсендірудің уақыт белгісін блоктау. |
| `expires_at` | `Timestamp` | Төлем мерзімінің аяқталуы. |
| `grace_expires_at` | `Timestamp` | Автоматты түрде ұзарту мүмкіндігінің аяқталуы (әдепкі +30 күн). |
| `redemption_expires_at` | `Timestamp` | Өтеу терезесінің аяқталуы (әдепкі +60 күн). |
| `auction` | `Option<NameAuctionStateV1>` | Голландиялық қайта ашылғанда немесе премиум аукциондар белсенді болғанда. |
| `last_tx_hash` | `Hash` | Осы нұсқаны жасаған транзакцияға детерминистік көрсеткіш. |
| `metadata` | `Metadata` | Тіркеушінің ерікті метадеректері (мәтіндік жазбалар, дәлелдер). |

Қолдау құрылымдары:

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
```

### 2.2 SuffixPolicyV1

| Өріс | |түрі Ескертпелер |
|-------|------|-------|
| `suffix_id` | `u16` | Бастапқы кілт; саясат нұсқаларында тұрақты. |
| `suffix` | `AsciiString` | мысалы, `sora`. |
| `steward` | `AccountId` | Стюард басқару жарғысында анықталған. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Әдепкі есеп айырысу активінің идентификаторы (мысалы, `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | Деңгейлі баға коэффиценттері және ұзақтық ережелері. |
| `min_term_years` | `u8` | Деңгейді қайта анықтауға қарамастан сатып алынған мерзімге арналған еден. |
| `grace_period_days` | `u16` | Әдепкі 30. |
| `redemption_period_days` | `u16` | Әдепкі 60. |
| `max_term_years` | `u8` | Алдын ала жаңартудың максималды ұзақтығы. |
| `referral_cap_bps` | `u16` | <=1000 (10%) әр чартер. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Тапсырма нұсқаулары бар басқару тізімі. |
| `fee_split` | `SuffixFeeSplitV1` | Қазынашылық / стюард / реферал акциялары (базалық ұпайлар). |
| `fund_splitter_account` | `AccountId` | Эскроу + қаражатты тарататын шот. |
| `policy_version` | `u16` | Әр өзгеріс сайын ұлғаяды. |
| `metadata` | `Metadata` | Кеңейтілген ескертпелер (KPI келісімі, сәйкестік құжатының хэштері). |

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

### 2.3 Табыс және есеп айырысу жазбалары

| Құрылым | Өрістер | Мақсаты |
|--------|--------|---------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, I18NI0000011300, I18NI0000011310. | Есеп айырысу дәуірі бойынша бағдарланған төлемдердің детерминативті жазбасы (апта сайын). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Төлем жарияланған сайын шығарылады (тіркеу, жаңарту, аукцион). |

Барлық `TokenValue` өрістері байланысты `SuffixPolicyV1`-те жарияланған валюта коды бар Norito канондық тіркелген нүктелік кодтауын пайдаланады.

### 2.4 Тіркеу оқиғалары

Канондық оқиғалар DNS/шлюзді автоматтандыру және аналитика үшін қайталау журналын қамтамасыз етеді.

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

Оқиғалар қайта ойнатылатын журналға қосылуы керек (мысалы, `RegistryEvents` домені) және DNS кэштері SLA ішінде жарамсыз болуы үшін шлюз арналарына көшіру керек.

## 3. Сақтау орны және индекстері

| Негізгі | Сипаттама |
|-----|-------------|
| `Names::<name_hash>` | `name_hash` бастап `NameRecordV1` дейінгі негізгі карта. |
| `NamesByOwner::<AccountId, suffix_id>` | Әмиянның пайдаланушы интерфейсіне арналған қосымша индекс (беттеу ыңғайлы). |
| `NamesByLabel::<suffix_id, normalized_label>` | Қақтығыстарды анықтау, қуатты анықтау. |
| `SuffixPolicies::<suffix_id>` | Соңғы `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` тарихы. |
| `RegistryEvents::<u64>` | Бірыңғай ұлғайту ретімен кілттелген тек қосу журналы. |

Барлық кілттер хосттар бойынша детерминистикалық хэштеуді сақтау үшін Norito кортеждері арқылы серияланады. Индекс жаңартулары бастапқы жазбамен қатар атомдық түрде орын алады.

## 4. Өмірлік цикл күйінің машинасы

| Мемлекет | Кіру шарттары | Рұқсат етілген ауысулар | Ескертпелер |
|-------|-----------------|--------------------|-------|
| Қол жетімді | `NameRecord` болмаған кезде алынған. | `PendingAuction` (премиум), `Active` (стандартты регистр). | Қолжетімділікті іздеу тек индекстерді оқиды. |
| Аукционды күтуде | `PriceTierV1.auction_kind` ≠ жоқ болғанда жасалған. | `Active` (аукцион аяқталды), `Tombstoned` (өтінімдер жоқ). | Аукциондар `AuctionOpened` және `AuctionSettled` шығарады. |
| Белсенді | Тіркеу немесе жаңарту сәтті аяқталды. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` ауысуды басқарады. |
| Жеңілдік кезеңі | `now > expires_at` кезінде автоматты түрде. | `Active` (уақытында жаңарту), `Redemption`, `Tombstoned`. | Әдепкі +30 күн; әлі де шешіледі, бірақ белгіленеді. |
| Өтеу | `now > grace_expires_at`, бірақ `< redemption_expires_at`. | `Active` (кеш жаңарту), `Tombstoned`. | Командалар айыппұлды талап етеді. |
| Мұздатылған | Басқару немесе қамқоршы қатып қалады. | `Active` (жөндеуден кейін), `Tombstoned`. | Контроллерлерді тасымалдау немесе жаңарту мүмкін емес. |
| Құлпытасқа қойылған | Өз еркімен тапсыру, тұрақты дау нәтижесі немесе мерзімі өтіп кеткен өтеу. | `PendingAuction` (голландиялық қайта ашылады) немесе құлпытас күйінде қалады. | `NameTombstoned` оқиғасында себеп болуы керек. |

Күйлік ауысулар сәйкес `RegistryEventKind` шығаруы КЕРЕК, сондықтан төменгі ағындағы кэштер когерентті болып қалады. Голландиялық қайта ашылған аукциондарға кіретін құлпытас атаулары `AuctionKind::DutchReopen` пайдалы жүктемесін қосады.

## 5. Canonical Events & Gateway Sync

Шлюздер `RegistryEventV1` жазылады және DNS/SoraFS арқылы синхрондалады:

1. Оқиғалар реті арқылы сілтеме жасалған ең соңғы `NameRecordV1` алынуда.
2. Қалпына келтіруші шешуші үлгілер (таңдаулы i105 + екінші ең жақсы қысылған (`sora`) мекенжайлар, мәтіндік жазбалар).
3. Жаңартылған аймақ деректерін [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) бөлімінде сипатталған SoraDNS жұмыс процесі арқылы бекіту.

Оқиғаны жеткізу кепілдіктері:

- `NameRecordV1` әсерін тигізетін әрбір транзакцияға `version` қатаң ұлғаюымен бір оқиғаны *қосу керек*.
- `RevenueSharePosted` оқиғалары `RevenueShareRecordV1` шығарған есеп айырысуларға сілтеме жасайды.
- Мұздату/ашу/құлпытас оқиғалары аудитті қайталау үшін `metadata` ішіндегі басқару артефакті хэштерін қамтиды.

## 6. Norito Пайдалы жүктемелер мысалы

### 6.1 Атау жазбасының мысалы

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "soraカタカナ...",
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

### 6.2 SuffixPolicy мысалы

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "soraカタカナ...",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("soraカタカナ..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "soraカタカナ...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. Келесі қадамдар- **SN-2b (Registrar API және басқару ілгектері):** бұл құрылымдарды Torii (Norito және JSON байланыстырулары) және басқару артефактілеріне сымды қабылдау тексерулері арқылы көрсетіңіз.
- **SN-3 (Аукцион және тіркеу механизмі):** орындау/анықтау және голландиялық қайта ашу логикасын жүзеге асыру үшін `NameAuctionStateV1` қайта пайдаланыңыз.
- **SN-5 (Төлем және есеп айырысу):** қаржыны салыстыру және есеп беруді автоматтандыру үшін `RevenueShareRecordV1` левереджі.

Сұрақтар немесе өзгерту сұраулары `roadmap.md` жүйесінде SNS жол картасы жаңартуларымен бірге берілуі және біріктірілген кезде `status.md` ішінде көшірілуі керек.