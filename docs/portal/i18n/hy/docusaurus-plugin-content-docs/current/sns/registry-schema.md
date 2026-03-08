---
id: registry-schema
lang: hy
direction: ltr
source: docs/portal/docs/sns/registry-schema.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Name Service Registry Schema
sidebar_label: Registry schema
description: Norito data structures, lifecycle rules, and event contracts for SNS registry smart contracts (SN-2a).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Կանոնական աղբյուր
Այս էջը արտացոլում է `docs/source/sns/registry_schema.md`-ը և այժմ ծառայում է որպես
կանոնական պորտալի պատճենը: Աղբյուրի ֆայլը մնում է թարգմանության թարմացումների համար:
:::

# Sora Name Service Registry Schema (SN-2a)

**Կարգավիճակ.** Նախագծված 2026-03-24 - ներկայացվել է SNS ծրագրի վերանայման  
**Ճանապարհային քարտեզի հղում.** SN-2a «Ռեեստրի սխեմա և պահեստավորման դասավորություն»  
**Ծավալը.** Սահմանեք կանոնական Norito կառուցվածքները, կյանքի ցիկլի վիճակները և թողարկված իրադարձությունները Sora Name Service-ի (SNS) համար, որպեսզի գրանցամատյանի և գրանցամատյանի իրականացումները մնան որոշիչ պայմանագրերի, SDK-ների և դարպասների միջև:

Այս փաստաթուղթը լրացնում է SN-2a-ի առաքման սխեման՝ նշելով.

1. Նույնացուցիչներ և հեշավորման կանոններ (`SuffixId`, `NameHash`, ընտրիչի ածանցում):
2. Norito structs/enums անունների գրառումների, վերջածանցների քաղաքականության, գնագոյացման մակարդակների, եկամուտների բաժանումների և ռեեստրի իրադարձությունների համար:
3. Պահպանման դասավորությունը և ինդեքսի նախածանցները դետերմինիստական ​​վերարտադրման համար:
4. Պետական ​​մեքենա, որը ծածկում է գրանցումը, նորացումը, շնորհը/փրկումը, սառեցումները և տապանաքարերը:
5. Կանոնական իրադարձություններ, որոնք սպառվում են DNS/gateway ավտոմատացման միջոցով:

## 1. Նույնացուցիչներ և հաշինգ

| Նույնացուցիչ | Նկարագրություն | ածանցում |
|-------------|-----------------------------|
| `SuffixId` (`u16`) | Վերին մակարդակի վերջածանցների ռեեստրի ամբողջ նույնացուցիչ (`.sora`, `.nexus`, `.dao`): Հավասարեցված է վերջածանցների կատալոգի հետ [`sns_suffix_governance_charter.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md): | Նշանակվել է կառավարման քվեարկությամբ; պահվում է `SuffixPolicyV1`-ում: |
| `SuffixSelector` | Վերջածանցի կանոնական լարային ձև (ASCII, փոքրատառ): | Օրինակ՝ `.sora` → `sora`: |
| `NameSelectorV1` | Երկուական ընտրիչ գրանցված պիտակի համար: | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Պիտակը NFC + փոքրատառ է՝ ըստ Normv1-ի: |
| `NameHash` (`[u8;32]`) | Առաջնային որոնման բանալին օգտագործվում է պայմանագրերի, իրադարձությունների և քեշերի կողմից: | `blake3(NameSelectorV1_bytes)`. |

Դետերմինիզմի պահանջներ.

- Պիտակները նորմալացվում են Normv1-ի միջոցով (UTS-46 խիստ, STD3 ASCII, NFC): Մուտքային օգտատերերի տողերը ՊԵՏՔ Է նորմալացվեն նախքան հեշացումը:
- Վերապահված պիտակները (`SuffixPolicyV1.reserved_labels`-ից) երբեք գրանցամատյան չեն մտնում. Միայն կառավարման համար նախատեսված անտեսումները թողարկում են `ReservedNameAssigned` իրադարձություններ:

## 2. Norito կառուցվածքներ

### 2.1 NameRecordV1

| Դաշտային | Տեսակ | Ծանոթագրություններ |
|-------|------|-------|
| `suffix_id` | `u16` | Հղումներ `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Հում ընտրիչ բայթեր աուդիտի/վրիպազերծման համար: |
| `name_hash` | `[u8; 32]` | Քարտեզների/միջոցառումների բանալին: |
| `normalized_label` | `AsciiString` | Մարդկանց համար ընթեռնելի պիտակ (Post Normv1): |
| `display_label` | `AsciiString` | Ստյուարդի կողմից տրամադրված պատյան; ընտրովի կոսմետիկա: |
| `owner` | `AccountId` | Վերահսկում է նորացումները/փոխանցումները: |
| `controllers` | `Vec<NameControllerV1>` | Հղումներն ուղղված են հաշվի հասցեներին, լուծիչներին կամ հավելվածի մետատվյալներին: |
| `status` | `NameStatus` | Կյանքի ցիկլի դրոշը (տես Բաժին 4): |
| `pricing_class` | `u8` | Ինդեքս՝ ըստ վերջածանցների գնագոյացման մակարդակների (ստանդարտ, պրեմիում, վերապահված): |
| `registered_at` | `Timestamp` | Արգելափակել սկզբնական ակտիվացման ժամանակացույցը: |
| `expires_at` | `Timestamp` | Վճարովի ժամկետի ավարտը: |
| `grace_expires_at` | `Timestamp` | Ավտոմատ թարմացման շնորհի ավարտը (կանխադրված +30 օր): |
| `redemption_expires_at` | `Timestamp` | Հետգնման պատուհանի ավարտը (կանխադրված +60 օր): |
| `auction` | `Option<NameAuctionStateV1>` | Ներկայացնել, երբ հոլանդական վերաբացված կամ պրեմիում աճուրդներն ակտիվ են: |
| `last_tx_hash` | `Hash` | Դետերմինիստական ​​ցուցիչ գործարքի համար, որն արտադրել է այս տարբերակը: |
| `metadata` | `Metadata` | Կամայական ռեգիստրի մետատվյալներ (տեքստային գրառումներ, ապացույցներ): |

Աջակցող կառույցներ.

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

### 2.2 վերջածանցՔաղաքականությունV1

| Դաշտային | Տեսակ | Ծանոթագրություններ |
|-------|------|-------|
| `suffix_id` | `u16` | Առաջնային բանալին; կայուն է քաղաքականության տարբերակներում: |
| `suffix` | `AsciiString` | օրինակ՝ `sora`: |
| `steward` | `AccountId` | Ստյուարդը սահմանված է կառավարման կանոնադրությամբ։ |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`: |
| `payment_asset_id` | `AsciiString` | Կանխադրված հաշվարկային ակտիվի նույնացուցիչ (օրինակ՝ `xor#sora`): |
| `pricing` | `Vec<PriceTierV1>` | Շերտավոր գնագոյացման գործակիցներ և տևողության կանոններ: |
| `min_term_years` | `u8` | Հարկ գնված ժամկետի համար՝ անկախ մակարդակի վերացումից: |
| `grace_period_days` | `u16` | Կանխադրված 30. |
| `redemption_period_days` | `u16` | Կանխադրված 60. |
| `max_term_years` | `u8` | Առաջնային նորացման առավելագույն երկարությունը: |
| `referral_cap_bps` | `u16` | <=1000 (10%) մեկ կանոնադրության համար: |
| `reserved_labels` | `Vec<ReservedNameV1>` | Կառավարման տրամադրված ցուցակ՝ հանձնարարականների ցուցումներով: |
| `fee_split` | `SuffixFeeSplitV1` | Գանձապետական ​​/ կառավարիչ / ուղղորդման բաժնետոմսեր (հիմնական միավորներ): |
| `fund_splitter_account` | `AccountId` | Հաշիվ, որը պահում է պահուստ + միջոցները բաշխում է: |
| `policy_version` | `u16` | Ավելացել է յուրաքանչյուր փոփոխության վրա: |
| `metadata` | `Metadata` | Ընդլայնված նշումներ (KPI դաշնագիր, համապատասխանության փաստաթղթերի հեշեր): |

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

### 2.3 Եկամուտների և հաշվարկների գրառումներ

| Կառուցվածք | Դաշտեր | Նպատակը |
|--------|--------|---------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, I18NI000001810X, I18NI000001813X. | Մեկ հաշվարկային դարաշրջանի համար ուղղորդված վճարումների դետերմինիստիկ գրառում (շաբաթական): |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`: | Թողարկվել է ամեն անգամ, երբ վճարային գրառում է կատարվում (գրանցում, նորացում, աճուրդ): |

Բոլոր `TokenValue` դաշտերն օգտագործում են Norito-ի կանոնական ֆիքսված կետի կոդավորումը՝ արտարժույթի կոդով, որը հայտարարված է առնչվող `SuffixPolicyV1`-ում:

### 2.4 Գրանցման իրադարձություններ

Կանոնական իրադարձությունները ապահովում են DNS/gateway ավտոմատացման և վերլուծությունների կրկնօրինակման մատյան:

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

Իրադարձությունները պետք է կցվեն վերարտադրվող մատյանին (օրինակ՝ `RegistryEvents` տիրույթ) և արտացոլվեն դարպասների հոսքերին, որպեսզի DNS քեշերը անվավեր ճանաչվեն SLA-ում:

## 3. Պահպանման դասավորություն և ինդեքսներ

| Բանալի | Նկարագրություն |
|-----|-------------|
| `Names::<name_hash>` | Առաջնային քարտեզ `name_hash`-ից մինչև `NameRecordV1`: |
| `NamesByOwner::<AccountId, suffix_id>` | Երկրորդական ինդեքս դրամապանակի UI-ի համար (էջադրման համար հարմար): |
| `NamesByLabel::<suffix_id, normalized_label>` | Հայտնաբերել կոնֆլիկտներ, ուժի դետերմինիստական ​​որոնում: |
| `SuffixPolicies::<suffix_id>` | Վերջին `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` պատմություն. |
| `RegistryEvents::<u64>` | Միայն հավելվածի գրանցամատյանը՝ միապաղաղ աճող հաջորդականությամբ: |

Բոլոր ստեղները սերիականացվում են՝ օգտագործելով Norito բազմապատիկները՝ հոսթինգը որոշիչ պահելու համար: Ինդեքսների թարմացումները տեղի են ունենում ատոմային եղանակով առաջնային գրառումների հետ մեկտեղ:

## 4. Lifecycle State Machine

| Պետական ​​| Մուտքի պայմանները | Թույլատրված անցումներ | Ծանոթագրություններ |
|-------|----------------|--------------------|-------|
| Հասանելի | Ստացվում է, երբ `NameRecord` բացակայում է: | `PendingAuction` (պրեմիում), `Active` (ստանդարտ ռեգիստր): | Հասանելիության որոնումը կարդում է միայն ինդեքսները: |
| Սպասվող Աճուրդ | Ստեղծվել է, երբ `PriceTierV1.auction_kind` ≠ ոչ մեկը: | `Active` (աճուրդը մարվում է), `Tombstoned` (առանց հայտերի): | Աճուրդները թողարկում են `AuctionOpened` և `AuctionSettled`: |
| Ակտիվ | Գրանցումը կամ երկարաձգումը հաջողվեց: | `GracePeriod`, `Frozen`, `Tombstoned`: | `expires_at` կրիչներ անցում. |
| GracePeriod | Ավտոմատ, երբ `now > expires_at`: | `Active` (ժամանակին թարմացում), `Redemption`, `Tombstoned`: | Կանխադրված +30 օր; դեռ լուծում է, բայց դրոշակավորված: |
| Փրկում | `now > grace_expires_at` բայց `< redemption_expires_at`: | `Active` (ուշ թարմացում), `Tombstoned`: | Հրամանները պահանջում են տուգանք. |
| Սառեցված | Կառավարման կամ խնամակալի սառեցում: | `Active` (վերականգնումից հետո), `Tombstoned`: | Հնարավոր չէ փոխանցել կամ թարմացնել կարգավորիչները: |
| տապանաքար | Կամավոր հանձնում, մշտական ​​վեճի արդյունք կամ ժամկետանց մարում: | `PendingAuction` (հոլանդերեն վերաբացվել) կամ մնում է տապանաքար: | Իրադարձությունը `NameTombstoned` պետք է ներառի պատճառ: |

Պետությունների անցումները ՊԵՏՔ Է թողարկեն համապատասխան `RegistryEventKind`, որպեսզի հոսանքով ներքև գտնվող քեշերը մնան համահունչ: Հոլանդիայի վերաբացված աճուրդներում հայտնված շիրմաքար անունները կցում են `AuctionKind::DutchReopen` օգտակար բեռ:

## 5. Canonical Events & Gateway Sync

Դարպասները բաժանորդագրվում են `RegistryEventV1`-ին և համաժամացվում DNS/SoraFS-ի հետ՝

1. Վերցվում է վերջին `NameRecordV1`-ը, որը վկայակոչված է իրադարձությունների հաջորդականությամբ:
2. Վերականգնվող լուծիչների կաղապարներ (նախընտրելի IH58 + երկրորդ լավագույն սեղմված (`sora`) հասցեներ, տեքստային գրառումներ):
3. Գոտու թարմացված տվյալների ամրացում SoraDNS աշխատանքային հոսքի միջոցով, որը նկարագրված է [`soradns_registry_rfc.md`]-ում (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md):

Միջոցառումների առաքման երաշխիքներ.

- `NameRecordV1`-ի վրա ազդող յուրաքանչյուր գործարք *պետք է* կցել ուղիղ մեկ իրադարձություն՝ խիստ աճող `version`-ով:
- `RevenueSharePosted` իրադարձությունների հղման կարգավորումներ, որոնք թողարկվել են `RevenueShareRecordV1`-ի կողմից:
- Սառեցնել/ապասառեցնել/տապանաքարի իրադարձությունները ներառում են կառավարման արտեֆակտի հեշեր `metadata`-ի ներսում՝ աուդիտի վերարտադրման համար:

## 6. Օրինակ Norito Օգտակար բեռներ

### 6.1 NameRecord Օրինակ

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "ih58...",
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

### 6.2 վերջածանցՔաղաքականության օրինակ

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "ih58...",
    status: Active,
    payment_asset_id: "xor#sora",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("ih58..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "ih58...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. Հաջորդ քայլերը- **SN-2b (Գրանցող API և կառավարման կեռիկներ).** ցուցադրել այս կառուցվածքները Torii-ի (Norito և JSON կապեր) և կառավարման արտեֆակտների համար մուտքի հաղորդագրության ստուգումների միջոցով:
- **SN-3 (Աճուրդի և գրանցման շարժիչ):** նորից օգտագործեք `NameAuctionStateV1`-ը` կատարելագործելու/բացահայտելու և հոլանդական վերաբացման տրամաբանությունը:
- **SN-5 (Վճարում և հաշվարկ).** լծակ `RevenueShareRecordV1` ֆինանսական հաշտեցման և հաշվետվությունների ավտոմատացման համար:

Հարցերը կամ փոփոխության հարցումները պետք է ներկայացվեն SNS-ի ճանապարհային քարտեզի թարմացումների հետ մեկտեղ `roadmap.md`-ում և արտացոլվեն `status.md`-ում՝ միավորվելիս: