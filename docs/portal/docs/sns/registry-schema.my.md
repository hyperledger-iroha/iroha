---
lang: my
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

::: Canonical Source ကို သတိပြုပါ။
ဤစာမျက်နှာသည် `docs/source/sns/registry_schema.md` ကို ရောင်ပြန်ဟပ်ပြီး ယခုအခါ စာမျက်နှာအဖြစ် ဆောင်ရွက်လျက်ရှိသည်။
canonical portal ကော်ပီ။ ဘာသာပြန်မွမ်းမံမှုများအတွက် အရင်းအမြစ်ဖိုင်သည် ကျန်ရှိနေပါသည်။
:::

# Sora အမည် ဝန်ဆောင်မှု Registry Schema (SN-2a)

**အခြေအနေ-** မူကြမ်းရေးဆွဲထားသော 2026-03-24 - SNS ပရိုဂရမ်ကို ပြန်လည်သုံးသပ်ရန်အတွက် တင်သွင်းခဲ့သည်  
**လမ်းပြမြေပုံလင့်ခ်-** SN-2a “မှတ်ပုံတင်စနစ်နှင့် သိုလှောင်မှုပုံစံ”  
**Scope-** canonical Norito ဖွဲ့စည်းပုံများ၊ ဘဝသံသရာအခြေအနေများနှင့် Sora Name Service (SNS) အတွက် ထုတ်လွှတ်သော ဖြစ်ရပ်များကို သတ်မှတ်ပါ ထို့ကြောင့် မှတ်ပုံတင်ခြင်းနှင့် မှတ်ပုံတင်ခြင်း အကောင်အထည်ဖော်မှုများသည် စာချုပ်များ၊ SDKs နှင့် gateways များတစ်လျှောက်တွင် အဆုံးအဖြတ်ရှိနေပါသည်။

ဤစာတမ်းသည် SN-2a အတွက် ပေးပို့နိုင်သော schema ကို သတ်မှတ်ခြင်းဖြင့် ပြီးမြောက်သည်-

1. ခွဲခြားသတ်မှတ်မှုများနှင့် hashing စည်းမျဉ်းများ (`SuffixId`၊ `NameHash`၊ ရွေးချယ်သူ ဆင်းသက်လာခြင်း)။
2. Norito အမည်မှတ်တမ်းများ၊ နောက်ဆက်တွဲမူဝါဒများ၊ စျေးနှုန်းအဆင့်များ၊ ဝင်ငွေခွဲခြမ်းများနှင့် မှတ်ပုံတင်ခြင်းဖြစ်ရပ်များအတွက် Norito တည်ဆောက်မှု/စာရင်းများ။
3. အဆုံးအဖြတ်အတိုင်း ပြန်ဖွင့်ခြင်းအတွက် သိုလှောင်မှု အပြင်အဆင်နှင့် အညွှန်းရှေ့ဆက်များ။
4. မှတ်ပုံတင်ခြင်း၊ သက်တမ်းတိုးခြင်း၊ ကျေးဇူးတော်/ရွေးနှုတ်ခြင်း၊ အေးခဲခြင်း၊ နှင့် သင်္ချိုင်းကျောက်များ အကျုံးဝင်သော နိုင်ငံတော်စက်။
5. DNS/gateway အလိုအလျောက်စနစ်ဖြင့် အသုံးပြုထားသော Canonical ဖြစ်ရပ်များ။

## 1. Identifiers & Hashing

| အမှတ်အသား | ဖော်ပြချက် | ဆင်းသက်လာခြင်း |
|--------------------|----------------|-----------------|
| `SuffixId` (`u16`) | ထိပ်တန်းအဆင့် နောက်ဆက်တွဲများအတွက် မှတ်ပုံတင်-ကျယ်ဝန်းသော အမှတ်အသား (`.sora`၊ `.nexus`၊ `.dao`)။ [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) ရှိ နောက်ဆက်ကက်တလောက်နှင့် ညှိထားသည်။ | အုပ်ချုပ်မှုမဲဖြင့် တာဝန်ပေးအပ်ခြင်း၊ `SuffixPolicyV1` တွင် သိမ်းဆည်းထားသည်။ |
| `SuffixSelector` | နောက်ဆက်တွဲ၏ Canonical string ပုံစံ (ASCII၊ စာလုံးသေး)။ | ဥပမာ- `.sora` → `sora`။ |
| `NameSelectorV1` | မှတ်ပုံတင်ထားသော အညွှန်းအတွက် ဒွိရွေးချယ်ကိရိယာ။ | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`။ အညွှန်းသည် Normv1 အတွက် NFC + စာလုံးအသေးဖြစ်သည်။ |
| `NameHash` (`[u8;32]`) | စာချုပ်များ၊ ဖြစ်ရပ်များနှင့် ကက်ရှ်များမှ အသုံးပြုသော အဓိကရှာဖွေသောကီး။ | `blake3(NameSelectorV1_bytes)`။ |

အဆုံးအဖြတ်လိုအပ်ချက်များ

- အညွှန်းများကို Normv1 (UTS-46 တင်းကျပ်သော၊ STD3 ASCII၊ NFC) မှတဆင့် ပုံမှန်ပြုလုပ်သည်။ Hashing မလုပ်မီ ဝင်လာသော အသုံးပြုသူ စာကြောင်းများကို ပုံမှန်ဖြစ်အောင် ပြုလုပ်ရပါမည်။
- Reserved labels (`SuffixPolicyV1.reserved_labels`) မှ registry ကို ဘယ်တော့မှ မထည့်ပါ။ အုပ်ချုပ်မှု-သပ်သပ် လွှမ်းမိုးမှုများသည် `ReservedNameAssigned` ဖြစ်ရပ်များကို ထုတ်လွှတ်သည်။

## 2. Norito ဖွဲ့စည်းပုံများ

### 2.1 NameRecordV1

| လယ် | ရိုက် | မှတ်စုများ |
|---------|------|-------|
| `suffix_id` | `u16` | ကိုးကား `SuffixPolicyV1`။ |
| `selector` | `NameSelectorV1` | စာရင်းစစ်/အမှားအယွင်းအတွက် အကြမ်းရွေးချယ်မှု ဘိုက်များ။ |
| `name_hash` | `[u8; 32]` | မြေပုံ/ဖြစ်ရပ်များအတွက်သော့။ |
| `normalized_label` | `AsciiString` | လူသားဖတ်နိုင်သော အညွှန်း (Normv1)။ |
| `display_label` | `AsciiString` | ဘဏ္ဍာစိုးပေးသော ဘူးခွံ၊ ရွေးချယ်နိုင်သောအလှကုန်။ |
| `owner` | `AccountId` | သက်တမ်းတိုးခြင်း/လွှဲပြောင်းမှုများကို ထိန်းချုပ်သည်။ |
| `controllers` | `Vec<NameControllerV1>` | ပစ်မှတ်အကောင့်လိပ်စာများ၊ ဖြေရှင်းသူများ သို့မဟုတ် အပလီကေးရှင်းမက်တာဒေတာကို ကိုးကားသည်။ |
| `status` | `NameStatus` | ဘဝသံသရာအလံ (အပိုင်း ၄ ကိုကြည့်ပါ)။ |
| `pricing_class` | `u8` | နောက်ဆက်တွဲစျေးနှုန်းအဆင့်များ (စံ၊ ပရီမီယံ၊ သီးသန့်) အညွှန်း။ |
| `registered_at` | `Timestamp` | ကနဦးအသက်သွင်းခြင်း၏ အချိန်တံဆိပ်တုံးကို ပိတ်ပါ။ |
| `expires_at` | `Timestamp` | အခကြေးငွေ သက်တမ်းကုန်ဆုံးခြင်း။ |
| `grace_expires_at` | `Timestamp` | အလိုအလျောက်သက်တမ်းတိုးခြင်း၏အဆုံး (မူလ +30 ရက်)။ |
| `redemption_expires_at` | `Timestamp` | ရွေးနုတ်ခြင်း ဝင်းဒိုး၏ အဆုံး (မူလ +60 ရက်)။ |
| `auction` | `Option<NameAuctionStateV1>` | ဒတ်ခ်ျပြန်ဖွင့်ချိန် သို့မဟုတ် ပရီမီယံလေလံပွဲများ အသက်ဝင်နေချိန်တွင် တင်ပြပါ။ |
| `last_tx_hash` | `Hash` | ဤဗားရှင်းကို ထုတ်လုပ်သည့် ငွေပေးငွေယူအတွက် အဆုံးအဖြတ်ညွှန်ပြချက်။ |
| `metadata` | `Metadata` | မတရား မှတ်ပုံတင်အရာရှိ မက်တာဒေတာ (စာသားမှတ်တမ်းများ၊ အထောက်အထားများ)။ |

အထောက်အကူပြုဖွဲ့စည်းပုံများ

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

| လယ် | ရိုက် | မှတ်စုများ |
|---------|------|-------|
| `suffix_id` | `u16` | အဓိကသော့; မူဝါဒဗားရှင်းများတွင် တည်ငြိမ်သည်။ |
| `suffix` | `AsciiString` | ဥပမာ၊ `sora`။ |
| `steward` | `AccountId` | အုပ်ချုပ်မှု ပဋိဉာဉ်တွင် သတ်မှတ်ထားသော ဘဏ္ဍာစိုး။ |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`။ |
| `payment_asset_id` | `AsciiString` | ပုံသေဖြေရှင်းမှုပိုင်ဆိုင်မှုသတ်မှတ်သူ (ဥပမာ `61CtjvNd9T3THAR65GsMVHr82Bjc`)။ |
| `pricing` | `Vec<PriceTierV1>` | အဆင့်အလိုက်စျေးနှုန်းဖော်ကိန်းများနှင့် ကြာချိန်စည်းမျဉ်းများ။ |
| `min_term_years` | `u8` | အထပ်ထပ်မခွဲခြားဘဲ ဝယ်ယူထားသော သက်တမ်းအတွက် အထပ်။ |
| `grace_period_days` | `u16` | ပုံသေ 30. |
| `redemption_period_days` | `u16` | ပုံသေ 60. |
| `max_term_years` | `u8` | အမြင့်ဆုံးကြိုတင် သက်တမ်းတိုးကာလ။ |
| `referral_cap_bps` | `u16` | <=1000 (10%) စင်းလုံးငှား။ |
| `reserved_labels` | `Vec<ReservedNameV1>` | စီမံအုပ်ချုပ်မှုတွင် ပေးအပ်ထားသော စာရင်းတွင် တာဝန်ညွှန်ကြားချက်များ။ |
| `fee_split` | `SuffixFeeSplitV1` | ဘဏ္ဍာတိုက်/ဘဏ္ဍာစိုး/ လွှဲပြောင်းပေးသည့်ရှယ်ယာများ (အခြေခံအချက်များ)။ |
| `fund_splitter_account` | `AccountId` | escrow ကိုင်ဆောင်ထားသောအကောင့် + ရန်ပုံငွေများဖြန့်ဝေသည်။ |
| `policy_version` | `u16` | အပြောင်းအလဲတိုင်းတွင် တိုးလာသည်။ |
| `metadata` | `Metadata` | တိုးချဲ့မှတ်စုများ (KPI ပဋိညာဉ်၊ လိုက်နာမှု ဒေါ့ရှ်များ)။ |

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

### 2.3 ဝင်ငွေနှင့် ဖြေရှင်းမှုမှတ်တမ်းများ

| ဖွဲ့စည်းပုံ | လယ်ကွင်းများ | ရည်ရွယ်ချက် |
|--------|--------|---------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `escrow_amount`, I18NI00000101013X. | အခြေချမှုကာလတစ်ခုလျှင် ဖြတ်သွားသော ငွေပေးချေမှုများ၏ အဆုံးအဖြတ်မှတ်တမ်း (အပတ်စဉ်)။ |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`။ | ငွေပေးချေမှုပို့စ်များ (မှတ်ပုံတင်ခြင်း၊ သက်တမ်းတိုးခြင်း၊ လေလံ) တစ်ကြိမ်စီထုတ်သည်။ |

`TokenValue` အကွက်များအားလုံးသည် ဆက်စပ် `SuffixPolicyV1` တွင် ကြေညာထားသော ငွေကြေးကုဒ်ဖြင့် Norito ၏ canonical fixed-point encoding ကို အသုံးပြုပါသည်။

### 2.4 မှတ်ပုံတင်ခြင်းဖြစ်ရပ်များ

Canonical ဖြစ်ရပ်များသည် DNS/gateway အလိုအလျောက်စနစ်နှင့် ခွဲခြမ်းစိတ်ဖြာမှုများအတွက် ပြန်လည်ဖွင့်ခြင်းမှတ်တမ်းကို ပေးဆောင်သည်။

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

ဖြစ်ရပ်များကို ပြန်လည်ဖွင့်နိုင်သောမှတ်တမ်း (ဥပမာ၊ `RegistryEvents` ဒိုမိန်း) တွင် ထည့်သွင်းပြီး gateway feeds များသို့ ရောင်ပြန်ဟပ်ထားသောကြောင့် DNS ကက်ရှ်များသည် SLA အတွင်း အသက်မဝင်ပါ။

## 3. Storage Layout & Index များ

| သော့ | ဖော်ပြချက် |
|-----|-------------|
| `Names::<name_hash>` | မူလမြေပုံ `name_hash` မှ `NameRecordV1`။ |
| `NamesByOwner::<AccountId, suffix_id>` | wallet UI အတွက် ဒုတိယအညွှန်း (pagination friendly)။ |
| `NamesByLabel::<suffix_id, normalized_label>` | ပဋိပက္ခများကို ထောက်လှမ်းခြင်း၊ အဆုံးအဖြတ်ပေးသော အာဏာကို ရှာဖွေခြင်း။ |
| `SuffixPolicies::<suffix_id>` | နောက်ဆုံးထွက် `SuffixPolicyV1`။ |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` မှတ်တမ်း။ |
| `RegistryEvents::<u64>` | တစ်ပုံတစ်ပုံ တိုးလာနေသော အစီအစဥ်များဖြင့် သော့ခတ်ထားသော နောက်ဆက်တွဲ-သာမှတ်တမ်း။ |

သော့များအားလုံးသည် Norito tuples များကို အသုံးပြု၍ host များတစ်လျှောက် အဆုံးအဖြတ်ကို ဖြတ်တောက်ထားခြင်းဖြစ်သည်။ အညွှန်းမွမ်းမံမှုများသည် မူလမှတ်တမ်းနှင့်အတူ အက်တမ်တွင် ဖြစ်ပေါ်သည်။

## 4. Lifecycle State Machine

| ပြည်နယ် | ဝင်ခွင့်အခြေအနေများ | ခွင့်ပြုထားသော အကူးအပြောင်းများ | မှတ်စုများ |
|---------|-----------------|---------------------|---------|
| ရနိုင် | `NameRecord` မရှိသည့်အခါ ဆင်းသက်လာသည်။ | `PendingAuction` (ပရီမီယံ)၊ `Active` (စံမှတ်ပုံ)။ | ရရှိနိုင်မှု ရှာဖွေမှုသည် အညွှန်းများကိုသာ ဖတ်သည်။ |
| PendingAuction | `PriceTierV1.auction_kind` ≠ မရှိသောအခါ ဖန်တီးခဲ့သည်။ | `Active` (လေလံအောင်သည်)၊ `Tombstoned` (လေလံမရှိ)။ | လေလံပွဲတွင် `AuctionOpened` နှင့် `AuctionSettled` ကို ထုတ်လွှတ်သည်။ |
| သက်ဝင် | မှတ်ပုံတင်ခြင်း သို့မဟုတ် သက်တမ်းတိုးခြင်း အောင်မြင်ခဲ့သည်။ | `GracePeriod`, `Frozen`, `Tombstoned`။ | `expires_at` သည် အသွင်ကူးပြောင်းမှုကို တွန်းအားပေးသည်။ |
| GracePeriod | `now > expires_at` က အလိုအလျောက် ရောက်သွားပါတယ်။ | `Active` (အချိန်မှန် သက်တမ်းတိုးခြင်း), `Redemption`, `Tombstoned`။ | ပုံသေ +30 ရက်; ဖြေရှင်းနေဆဲဖြစ်သော်လည်း အလံပြထားသည်။ |
| ရွေးနှုတ်ခြင်း | `now > grace_expires_at` ဒါပေမယ့် `< redemption_expires_at`။ | `Active` (သက်တမ်းတိုးနောက်ကျ), `Tombstoned`။ | အမိန့်ပေးလျှင် ဒဏ်ကြေးငွေ လိုအပ်သည်။ |
| အေးခဲ | အုပ်ချုပ်မှု သို့မဟုတ် အုပ်ထိန်းသူ အေးခဲခြင်း။ | `Active` (ပြုပြင်ပြီးနောက်), `Tombstoned`။ | ထိန်းချုပ်ကိရိယာများကို လွှဲပြောင်းခြင်း သို့မဟုတ် အပ်ဒိတ်လုပ်၍မရပါ။ |
| အုတ်ဂူ | ဆန္ဒအလျောက် လက်နက်ချခြင်း၊ အမြဲတမ်းအငြင်းပွားမှုရလဒ် သို့မဟုတ် သက်တမ်းကုန်ဆုံးသော ရွေးနှုတ်ခြင်း | `PendingAuction` (ဒတ်ခ်ျပြန်ဖွင့်သည်) သို့မဟုတ် သင်္ချိုင်းဂူတွင် ကျန်နေသေးသည်။ | Event `NameTombstoned` တွင် အကြောင်းအရင်း ပါဝင်ရပါမည်။ |

ပြည်နယ်အကူးအပြောင်းများသည် သက်ဆိုင်ရာ `RegistryEventKind` ကို ထုတ်လွှတ်ရမည်ဖြစ်ပြီး ရေအောက်ပိုင်းရှိ ကက်ရှ်များသည် စည်းလုံးနေမည်ဖြစ်သည်။ ဒတ်ခ်ျလေလံပွဲများ ပြန်ဖွင့်သောအခါတွင် ဝင်ရောက်သည့် သင်္ချိုင်းကျောက်တုံးအမည်များသည် `AuctionKind::DutchReopen` ပါ၀င်ငွေကို ပူးတွဲပါရှိသည်။

## 5. Canonical Events & Gateway Sync

Gateways သည် `RegistryEventV1` သို့ စာရင်းသွင်းပြီး DNS/SoraFS ဖြင့်-

1. ဖြစ်ရပ်အစီအစဥ်အလိုက် ကိုးကားထားသော နောက်ဆုံးပေါ် `NameRecordV1` ကို ရယူခြင်း။
2. ဖြေရှင်းသူ နမူနာပုံစံများကို ပြန်လည်ထုတ်ပေးခြင်း (နှစ်သက်သော i105 + ဒုတိယအကောင်းဆုံး ချုံ့ထားသော (`sora`) လိပ်စာများ၊ စာသားမှတ်တမ်းများ)။
3. [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) တွင် ဖော်ပြထားသော SoraDNS အလုပ်အသွားအလာမှတဆင့် မွမ်းမံထားသော ဇုန်ဒေတာကို ပင်ထိုးခြင်း။

ပွဲပေးပို့မှု အာမခံချက်များ-

- `NameRecordV1` ကို သက်ရောက်သည့် ငွေပေးငွေယူတိုင်း * တင်းကြပ်စွာ တိုးလာသော `version` နှင့်အတူ ဖြစ်ရပ်တစ်ခု အတိအကျ ထပ်ဖြည့်ရပါမည်။
- `RevenueSharePosted` ဖြစ်ရပ်များ `RevenueShareRecordV1` မှ ထုတ်လွှတ်သော အကိုးအကား အခြေချမှုများ။
- အေးခဲခြင်း/အအေးခံခြင်း/အုတ်ဂူကျောက်တုံးဖြစ်ရပ်များသည် စာရင်းစစ်ပြန်ဖွင့်ရန်အတွက် `metadata` အတွင်း အုပ်ချုပ်မှုဆိုင်ရာ သင်္ကေတများပါ၀င်သည်။

## 6. ဥပမာ Norito Payloads

### 6.1 NameRecord နမူနာ

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

### 6.2 SuffixPolicy ဥပမာ

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

## 7. နောက်အဆင့်များ- **SN-2b (မှတ်ပုံတင်အရာရှိ API နှင့် အုပ်ချုပ်မှုချိတ်များ):** Torii (Norito နှင့် JSON စည်းနှောင်မှုများ) နှင့် အုပ်ချုပ်မှုဆိုင်ရာပစ္စည်းများသို့ ကြေးနန်းဝင်ရောက်မှုစစ်ဆေးမှုများမှတစ်ဆင့် ဤဖွဲ့စည်းပုံကို ဖော်ထုတ်ပါ။
- **SN-3 (လေလံနှင့်မှတ်ပုံတင်ခြင်းအင်ဂျင်)-** ကတိကဝတ်/ထုတ်ဖော်ခြင်းနှင့် ဒတ်ခ်ျယုတ္တိကိုပြန်ဖွင့်ရန် `NameAuctionStateV1` ကို ပြန်သုံးပါ။
- **SN-5 (ငွေပေးချေမှုနှင့် အခြေချမှု):** ဘဏ္ဍာရေးပြန်လည်သင့်မြတ်ရေးနှင့် အစီရင်ခံခြင်းအလိုအလျောက်လုပ်ဆောင်မှုအတွက် `RevenueShareRecordV1` ကို အသုံးချပါ။

မေးခွန်းများ သို့မဟုတ် ပြောင်းလဲမှုတောင်းဆိုချက်များကို `roadmap.md` ရှိ SNS လမ်းပြမြေပုံမွမ်းမံမှုများနှင့်အတူ ပူးတွဲတင်ပြပြီး ပေါင်းစည်းလိုက်သောအခါ `status.md` တွင် ရောင်ပြန်ဟပ်နေရပါမည်။