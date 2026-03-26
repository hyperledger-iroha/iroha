---
id: registry-schema
lang: dz
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

:::དྲན་ཐོའི་འབྱུང་ཁུངས།
ཤོག་ངོས་འདི་ `docs/source/sns/registry_schema.md` ལུ་མེ་ལོང་དང་ ད་ལྟོ་འདི་ ༡ བཟུམ་སྦེ་ལཱ་འབདཝ་ཨིན།
ཀེ་ནོ་ནིག་ དྲྭ་ཚིགས་ཀྱི་འདྲ་བཤུས། སྐད་སྒྱུར་དུས་མཐུན་བཟོ་ནིའི་དོན་ལུ་ འབྱུང་ཁུངས་ཡིག་སྣོད་འདི་ལུས་ཡོདཔ་ཨིན།
:::

# སོ་ར་མིང་ཞབས་ཏོག་ཐོ་བཀོད་ལས་རིམ་ (SN-2a)

**Status:** ཟིན་བྲིས་ ༢༠༢༦-༠༣-༢༤ -- ཨེསི་ཨེན་ཨེསི་ལས་རིམ་བསྐྱར་ཞིབ་ཀྱི་དོན་ལུ་ ཕུལ་ཡོདཔ།  
**ལམ་གྱི་འབྲེལ་ལམ་:** SN-2a “ཐོ་བཀོད་ལས་རིམ་དང་ གསོག་འཇོག་སྒྲིག་བཀོད་”  
**Scope:** ཁྲིམས་ལུགས་ Norito གི་བཟོ་བཀོད་དང་ མི་ཚེ་གི་གནས་སྟངས་ དེ་ལས་ སོ་ར་མིང་ཞབས་ཏོག་ (SNS) གི་དོན་ལུ་ ཐོ་བཀོད་དང་ ཐོ་བཀོད་དང་ཐོ་བཀོད་ལག་ལེན་འཐབ་མི་ཚུ་གི་དོན་ལུ་ ལས་རིམ་ཚུ་ གན་རྒྱ་དང་ SDKs དེ་ལས་ སྒོ་སྒྲིག་ཚུ་ནང་ གཏན་འབེབས་བཟོཝ་ཨིན།

ཡིག་ཆ་འདི་གིས་ SN-2a གི་དོན་ལུ་ བཀྲམ་སྤེལ་འབད་ཚུགས་པའི་ ལས་རིམ་འདི་ གསལ་བཀོད་འབད་དེ་མཇུག་བསྡུཝ་ཨིན།

༡ ངོས་འཛིན་དང་ ཧ་ཤིང་ལམ་ལུགས་ (`SuffixId`, I18NI000000018X, གདམ་ཁའི་འབྱུང་ཁུངས་)།
༢ མིང་ཐོ་བཀོད་དང་ རྗེས་འཇུག་སྲིད་བྱུས་ གོང་ཚད་བཀོད་ནི་ རིམ་པ་འོང་འབབ་བགོ་བཤའ་རྐྱབ་ནི་ དེ་ལས་ ཐོ་བཀོད་ཀྱི་བྱུང་རིམ་ཚུ་གི་དོན་ལུ་ བཀོད་སྒྲིག་/ཨང་གྲངས་ཚུ་ བཀོད་སྒྲིག་འབདཝ་ཨིན།
༣ གཏན་འབེབས་བཟོ་ནིའི་དོན་ལུ་ བཀོད་སྒྲིག་དང་ ཟུར་ཐོ་སྔོན་འཇུག་ཚུ་ གསོག་འཇོག་འབད་ནི།
༤ མངའ་སྡེའི་འཕྲུལ་ཆས་ཁྱབ་ཁོངས་ཐོ་བཀོད་དང་ བསྐྱར་གསོ་ མཛེས་རྒྱན་/ སྐྱོབ་ཐབས་ གྱང་ཁོག་དང་ དུར་ཁྲོད་ཀྱི་རྡོ་ཚུ་ཨིན་པས།
༥ ཌི་ཨེན་ཨེསི་/གཱེཊ་རང་བཞིན་གྱིས་ བཀོལ་སྤྱོད་འབད་མི་ ཀེ་ནོ་ནིཀ་བྱུང་ལས་ཚུ།

## 1. ངོས་འཛིན་དང་ཧ་ཤིང་།

| ངོས་འཛིན་འབད་མི་ | འགྲེལ་བཤད་ | འབྱུང་ཁུངས་ |
|------------------------------------------------ |
| `SuffixId` (I18NI0000020X) | མཐོ་རིམ་རྗེས་འཇུག་ (`.sora`, I18NI000000022X, `.dao`). [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md)ནང་རྗེས་འཇུག་ཐོ་གཞུང་དང་གཅིག་ཁར་ཕྲང་སྒྲིག་འབད་ཡོདཔ། | གཞུང་སྐྱོང་ཚོགས་རྒྱན་ཐོག་ལས་ འགན་སྤྲོད་འབད་ཡོདཔ། `SuffixPolicyV1` ནང་གསོག་འཇོག་འབད་ཡོདཔ། |
| `SuffixSelector` | རྗེས་འཇུག་ (ཨེ་ཨེསི་སི་ཨའི་, གནས་སྟངས་དམའ་བ།) གི་ ཀེ་ནོ་ནིག་ཡིག་རྒྱུན་རྣམ་པ་། | དཔེར་ན་: I18NI000000027X → I18NI0000028X. |
| I18NI0000029X | ཐོ་བཀོད་འབད་ཡོད་པའི་ཁ་ཡིག་གི་དོན་ལུ་ གཉིས་ལྡན་འདེམས་སྒྲུགས། | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. ཁ་ཡིག་འདི་ NFC + sorgor-case per Normv1 ཨིན། |
| I18NI0000031X (I18NI0000032X) | གན་ཡིག་དང་བྱུང་ལས་ དེ་ལས་ འདྲ་མཛོད་ཚུ་གིས་ ལག་ལེན་འཐབ་མི་ གཞི་རིམ་བལྟ་ཞིབ་ལྡེ་མིག་། | `blake3(NameSelectorV1_bytes)`. |

གཏན་འབེབས་རིང་ལུགས་ཀྱི་དགོས་མཁོ།

- ཁ་ཡིག་ཚུ་ Normv1 བརྒྱུད་དེ་ སྤྱིར་བཏང་བཟོ་ཡོདཔ་ཨིན། ནང་འོང་ལག་ལེན་པའི་ཡིག་རྒྱུན་ཚུ་ ཧ་ཤིང་གི་ཧེ་མ་ སྤྱིར་བཏང་བཟོ་དགོ།
- བཀག་བཞག་ཡོད་པའི་ཁ་ཡིག་ཚུ་ (`SuffixPolicyV1.reserved_labels` ལས་) ཐོ་བཀོད་ཐོ་བཀོད་ནང་ ནམ་ཡང་མི་བཙུགས། གཞུང་སྐྱོང་རྐྱངམ་གཅིག་གིས་ It I18NI000000035X བྱུང་རིམ་ཚུ་ བཀག་ཆ་འབདཝ་ཨིན།

## 2. Norito བཟོ་བཀོད།

### 2.1 མིང་ཐོབ།

| ཕིལཌ་ | དབྱེ་བ་ | དྲན་ཐོ། |
|--------|-----------|--------------------------------------------------------
| `suffix_id` | `u16` | གཞི་བསྟུན་ `SuffixPolicyV1`. |
| I18NI0000039X | I18NI0000040X | རྩིས་ཞིབ་/རྐྱེན་སེལ་གྱི་དོན་ལུ་ གདམ་ཁའི་བཱའིཊི། |
| I18NI0000041X | I18NI0000042X | ས་ཁྲ་/བྱུང་འབྲེལ། |
| I18NI0000043X | I18NI0000044X | མི་གིས་ལྷག་ཚུགས་པའི་ ཁ་ཡིག་ (post Normv1)། |
| `display_label` | I18NI0000046X | སི་ཊི་ཝརཌ་བྱིན་ཡོད་པའི་རྩོད་གཞི་; གདམ་ཁའི་ མཛེས་བཟོས། |
| I18NI0000047X | I18NI0000048X | བསྐྱར་གསོ་ཚུ་ ཚད་འཛིན་/སྤོ་བཤུད་ཚུ། |
| I18NI0000049X | `Vec<NameControllerV1>` | གཞི་བསྟུན་ཚུ་དམིགས་གཏད་རྩིས་ཐོ་ཁ་བྱང་ཚུ་དང་ ཐག་གཅོད་འབད་མི་ ཡང་ན་ གློག་རིམ་མེ་ཊ་ཌེ་ཊ་ཚུ་ཨིན། |
| `status` | I18NI0000002X | མི་ཚེའི་འཁོར་ལོའི་རྒྱལ་དར་ (དོན་ཚན་༤) |
| I18NI0000003X | `u8` | རྗེས་འཇུག་གོང་ཚད་ཀྱི་རིམ་པ་ (ཚད་ལྡན་ མཐོ་ཚད། གསོག་འཇོག་འབད་ཡོདཔ།) ཟུར་ཐོ། |
| I18NI0000005X | I18NI0000005X | འགོ་བཙུགས་ཤུགས་ལྡན་གྱི་བཀག་ཆའི་དུས་ཚོད་མཚོན་རྟགས། |
| I18NI0000007X | I18NI0000008X | གླ་ཆ་སྤྲོད་པའི་དུས་ཡུན་མཇུག་བསྡུ། |
| `grace_expires_at` | `Timestamp` | རང་བཞིན་བསྐྱར་གསོ་འབད་ནིའི་བྱིན་རླབས་ཀྱི་མཇུག་ (སྔོན་སྒྲིག་+༣༠ཉིན)། |
| `redemption_expires_at` | `Timestamp` | བསྐྱར་གསོའི་སྒོ་སྒྲིག་ (སྔོན་སྒྲིག་ +༦༠ ཉིན་གྲངས་) མཇུག་བསྡུ། |
| `auction` | `Option<NameAuctionStateV1>` | ཌཆ་ལོག་ཁ་ཕྱེ་མི་ཡང་ན་ གོང་ཚད་མཐོ་ཤོས་རིན་བསྡུར་ཚུ་ ཤུགས་ལྡན་ཡོད་པའི་སྐབས་ལུ་ བཀྲམ་སྟོན་འབད། |
| `last_tx_hash` | I18NI0000006X | ཐོན་རིམ་འདི་བཟོ་མི་ ཚོང་འབྲེལ་ལུ་ གཏན་འབེབས་བརྡ་སྟོན་འབད། |
| `metadata` | `Metadata` | བར་འདུམ་གྱི་ registrar མེ་ཊ་ཌེ་ཊ་ (ཚིག་ཡིག་དྲན་ཐོ་ བདེན་དཔང་།) |

སྒྲིག་བཀོད་ལུ་རྒྱབ་སྐྱོར་འབད་ནི།

I18NF0000009X

### 2.2 སུ་ཕིག་པོ་ལི་སི་V1

| ཕིལཌ་ | དབྱེ་བ་ | དྲན་ཐོ། |
|--------|-----------|--------------------------------------------------------
| `suffix_id` | `u16` | གཞི་རྟེན་ལྡེ་མིག་; སྲིད་བྱུས་ཐོན་རིམ་ནང་ལུ་ བརྟན་ཏོག་ཏོ་ཨིན། |
| `suffix` | `AsciiString` | དཔེར་ན་ `sora`. |
| `steward` | `AccountId` | གཞུང་སྐྱོང་ཆིངས་ཡིག་ནང་ Staward གིས་ ངེས་ཚིག་བཀོད་ཡོདཔ་ཨིན། |
| `status` | I18NI0000007X | I18NI000000078X, `Paused`, I18NI000000080X. |
| I18NI0000081X | I18NI0000082X | སྔོན་སྒྲིག་གཞིས་ཆགས་རྒྱུ་དངོས་ངོས་འཛིན་ (དཔེར་ན་ I18NI0000083X). |
| I18NI0000084X | `Vec<PriceTierV1>` | བསྡམས་པའི་གོང་ཚད་འཇོན་ཚད་དང་དུས་ཡུན་གྱི་ལམ་ལུགས། |
| I18NI0000086X | `u8` | ཉོ་བའི་དུས་ཡུན་གྱི་ རིམ་པ་ གང་ལྟར་ཡང་ མ་ལྟོས་པར་ འཛོལ་བ། |
| I18NI0000008X | I18NI0000089X | སྔོན་སྒྲིག་ 30. |
| `redemption_period_days` | I18NI0000091X | སྔོན་སྒྲིག་ ༦༠. |
| I18NI0000092X | `u8` | གདོང་ཕྱོགས་ཀྱི་བསྐྱར་གསོ་རིང་ཚད་མཐོ་ཤོས་འདི། |
| I18NI0000094X | I18NI0000095X | <=༡༠༠༠ (༡༠%) ཆིངས་ཡིག། |
| I18NI0000096X | `Vec<ReservedNameV1>` | གཞུང་སྐྱོང་གིས་ ལས་འགན་བཀོད་རྒྱ་ཚུ་དང་གཅིག་ཁར་ བཀྲམ་སྤེལ་འབད་ཡོདཔ། |
| I18NI0000098X | I18NI000009X | དངུལ་ཁང་/ བདག་འཛིན་/ བརྡ་སྤྲོད་ཀྱི་བགོ་བཤའ་ (གཞི་རྟེན་ས་ཚིག)། |
| `fund_splitter_account` | I18NI0000010101 མ་དངུལ་ + བཀྲམ་སྤེལ་འབད་མི་ རྩིས་ཁྲ་ + བཀྲམ་སྤེལ་འབདཝ་ཨིན། |
| `policy_version` | `u16` | བསྒྱུར་བཅོས་ག་ར་ལུ་ ཡར་སེང་། |
| `metadata` | `Metadata` | རྒྱ་བསྐྱེད་འབད་ཡོད་པའི་དྲན་ཐོ་ (ཀེ་པི་ཨའི་ཆིངས་ཡིག་ བསྟར་སྤྱོད་ཀྱི་ ཌོག་ཧེ་ཤེ)། |

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

### 2.3 ཡོང་འབབ་དང་ གཞིས་ཆགས་དྲན་ཐོ།

| སྒྲིག་བཀོད་ | ཕིལཌ་ | དམིགས་ཡུལ། |
|-------------------------------------- |
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | གཞིས་ཆགས་ཀྱི་ གླ་ཆ་གི་ གཏན་འཁེལ་གྱི་ ཐོ་བཀོད་ཚུ་ དུས་རབས་རེ་ལུ་ (བདུན་ཕྲག་རེ) |
| `RevenueAccrualEventV1` | I18NI000000116X, I18NI0000000118X, I18NI0000000119X, I18NI000000120X, I18NI0000000000000121X. | དུས་ཚོད་རེ་ལུ་ དངུལ་སྤྲོད་ཀྱི་བརྡ་འཕྲིན་ (ཐོ་བཀོད་དང་ བསྐྱར་གསོ་ རིན་བསྡུར་) ཚུ་ བཏོན་གཏང་། |

I18NI00000000122X ས་སྒོ་ཆ་མཉམ་གྱིས་ Norito གི་ ཀེ་ནོ་ནིག་གཏན་འཇགས་ཨེན་ཀོ་ཌིང་འདི་ འབྲེལ་མཐུན་ `SuffixPolicyV1` ནང་གསལ་བསྒྲགས་འབད་ཡོད་པའི་ དངུལ་གྱི་ཨང་རྟགས་དང་གཅིག་ཁར་ ལག་ལེན་འཐབ་ཨིན།

### 2.4 ཐོ་འགོད་ལས་རིམ།

ཀེ་ནོ་ནིག་བྱུང་ལས་ཚུ་གིས་ ཌི་ཨེན་ཨེསི་/སྒོ་སྒྲིག་རང་བཞིན་དང་དབྱེ་དཔྱད་ཀྱི་དོན་ལུ་ བསྐྱར་རྩེད་དྲན་ཐོ་བྱིནམ་ཨིན།

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

ལས་རིམ་ཚུ་ བསྐྱར་རྩེད་འབད་བཏུབ་པའི་དྲན་ཐོ་ (དཔེར་ན་ I18NI000000124X domain) དང་ འཛུལ་སྒོ་ཕིཌི་ཚུ་ལུ་ མེ་ལོང་ནང་བཙུགས་དགོཔ་ལས་ ཌི་ཨེན་ཨེསི་ འདྲ་མཛོད་ཚུ་ ཨེསི་ཨེལ་ཨེ་ནང་འཁོད་ལུ་ ཆ་མེད་བཏང་དགོ།

## 3. བགོ་སྒྲིག་དང་ཟུར་ཐོ།

| ལྡེ་མིག་ | འགྲེལ་བཤད་ |
|-----|-------------------------------------------------------------------------------------------------
| `Names::<name_hash>` | `name_hash` ལས་ I18NI000000127X ལས་ གཞི་རྩ། |
| `NamesByOwner::<AccountId, suffix_id>` | དངུལ་ཁུག་ཡུ་ཨའི་ (pagination མཐུན་སྒྲིག་) གི་དོན་ལུ་ གཞི་རིམ་ཟུར་ཐོ། |
| `NamesByLabel::<suffix_id, normalized_label>` | འཁྲུག་རྩོད་དང་ ནུས་ཤུགས་གཏན་འབེབས་འཚོལ་ཞིབ་འབད་ནི། |
| `SuffixPolicies::<suffix_id>` | `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` རྒྱུ། |
| `RegistryEvents::<u64>` | ཟུར་ཐོ་རྐྱངམ་ཅིག་གིས་ ལྡེ་མིག་འདི་ གཅིག་མཚུངས་སྦེ་ཡར་སེང་འགྱོ་བའི་གོ་རིམ་ཐོག་ལས་ ལྡེ་མིག་བརྐྱབ། |

ལྡེ་མིག་ཆ་མཉམ་ Norito ཊུ་པལ་ཚུ་ལག་ལེན་འཐབ་སྟེ་ ཧ་ཤིང་གཏན་འབེབས་འདི་ ཧོསིཊི་ཚུ་ནང་ལས་ཕར་བཞག་དགོ། ཟུར་ཐོ་དུས་མཐུན་བཟོ་ནི་ཚུ་ གཞི་རིམ་དྲན་ཐོ་དང་གཅིག་ཁར་ རྡུལ་ཕྲན་གྱི་ཐོག་ལས་འབྱུངམ་ཨིན།

## 4. འཚོ་འཁོར་གྱི་མངའ་སྡེའི་འཕྲུལ་རིས།

| མངའ་སྡེ་ | འཛུལ་ཞུགས་གནས་སྟངས་ | བསྒྱུར་ཆོག་ཆོག་པའི་ཆོག་ཐམ། | དྲན་ཐོ། |
|-------|-----------------|---------------------|-------|
| འཐོབ་ཚུགསཔ་ | I18NI000000135X མེད་པའི་སྐབས་ལས་ཐོན་ཡོདཔ། | `PendingAuction` (premium), I18NI000000137X (ཚད་ལྡན་གྱི་ཐོ་ཡིག)། | འཐོབ་ཚུགས་པའི་འཚོལ་ཞིབ་འཚོལ་ཞིབ་འདི་གིས་ ཟུར་ཐོ་ཚུ་རྐྱངམ་ཅིག་ཨིན། |
| Pending རིན་བསྡུར་ | `PriceTierV1.auction_kind` མེད་པའི་སྐབས་གསར་བསྐྲུན་འབད་ཡོདཔ། | I18NI000000139X (རིན་བསྡུར་གཞི་བཅོལ་) I18NI000000140X (མེད་པ)། | རིན་བསྡུར་ཚུ་གིས་ `AuctionOpened` དང་ I18NI000000142X ཚུ་བཏོན་ཡོདཔ་ཨིན། |
| ཤུགས་ལྡན་ | ཐོ་བཀོད་ཡང་ན་ བསྐྱར་གསོ་མཐར་འཁྱོལ་བྱུང་ཡོདཔ། | `GracePeriod`, `Frozen`, `Tombstoned`. | I18NI000000146X འདྲེན་བྱེད་འཕོ་འགྱུར་. |
| གེ་རེསི་པེ་རིའོཌ | རང་བཞིན་གྱིས་ `now > expires_at` སྐབས། | I18NI00000000148X (དུས་རྒྱུན་བསྐྱར་གསོ་) `Redemption`, I18NI000000150X. | སྔོན་སྒྲིག་ +༣༠ ; ད་ལྟོ་ཡང་སེལ་ཡོད་རུང་ དར་ཁྱབ་བཏང་ཡོདཔ་ཨིན། |
| བསྐྱར་གསོ་ | `now > grace_expires_at` ཡིན་ནའང་ `< redemption_expires_at`. | `Active` (མཇུག་བསྡུའི་བསྐྱར་གསོ) `Tombstoned`. | བརྡ་བཀོད་ཚུ་ལུ་ ཉེས་ཆད་ཀྱི་འཐུས་དགོཔ་ཨིན། |
| འཁྱགས་རོམ། | གཞུང་སྐྱོང་ ཡང་ན་ ལྟ་རྟོག་པ་ འཁྱགས་རོམ། | `Active` (བཅོས་སྒྲིག་རྗེས་) `Tombstoned`. | ཚད་འཛིན་ཚུ་ སྤོ་བཤུད་འབད་ནི་དང་ དུས་མཐུན་བཟོ་མི་ཚུགས། |
| དུར་ཁྲོད་རྡོ་བཟོ། | ཁས་བླངས་མགོ་སྐོར་ གཏན་འཇགས་རྩོད་རྙོགས་ཀྱི་གྲུབ་འབྲས་ ཡང་ན་ དུས་ཡུན་ཚང་བའི་ བསྐྱར་གསོས། | `PendingAuction` (Dutch reopen) ཡང་ན་ དུར་ཁྲོད་ཀྱི་རྡོ་བཟོས་ཡོད། | བྱུང་ལས་ `NameTombstoned` ནང་ རྒྱུ་མཚན་ཚུད་དགོ། |

མངའ་སྡེ་གི་འགྱུར་བ་ཚུ་ མཐུན་སྒྲིག་ཡོད་པའི་ `RegistryEventKind` འདི་བཏོན་དགོཔ་ལས་ མར་གྱི་འདྲ་མཛོད་ཚུ་ གཅིག་མཐུན་སྦེ་སྡོད་དགོ། ཌཆ་ཁ་ཕྱེ་མི་ འདྲུད་པའི་མིང་ཚུ་གིས་ I18NI000000160X པེ་ལོཌ་ཅིག་ མཉམ་སྦྲགས་འབདཝ་ཨིན།

## 5. ཏན་ཏན་བྱུང་རིམ་དང་སྒོ་ལམ།

སྒོ་སྒྲིག་ཚུ་གིས་ `RegistryEventV1` ལུ་ མིང་རྟགས་བཀོདཔ་ཨིནམ་དང་ DNS/I18N1NT000000007X ལུ་ མཉམ་འབྱུང་འབདཝ་ཨིན།

1. བྱུང་ལས་རིམ་པ་གིས་ གཞི་བསྟུན་འབད་མི་ `NameRecordV1` གསརཔ་འདི་ ལེན་དོ།
2. ཐག་གཅོད་པ་ ཊེམ་པེལེཊི་ཚུ་ སླར་གསོ་འབད་ (དགའ་གདམ་འབད་ཡོད་པའི་ I105 + གཉིས་པ་ བསྡམ་བཞག་ཡོད་པའི་ (`sora`) ཁ་བྱང་ཚུ་, ཚིག་ཡིག་དྲན་ཐོ་ཚུ་)།
3. པིན་ནིང་གིས་ [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) ནང་གསལ་བཀོད་འབད་ཡོད་པའི་ SoraDNS ལཱ་གི་རྒྱུན་རིམ་བརྒྱུད་དེ་ དུས་མཐུན་བཟོ་ཡོདཔ་ཨིན།

བྱུང་རིམ་སྤྲོད་ལེན་འགན་ལེན་ཚུ།

- `NameRecordV1` *must* ལུ་གནོད་པ་ཡོད་པའི་ ཚོང་འབྲེལ་ག་ར་གིས་ `version` དམ་དམ་སྦེ་ཡར་སེང་འབད་དེ་ བྱུང་རིམ་གཅིག་ལུ་ ཐོ་ཕོག་ཡོདཔ་ཨིན།
- I18NI000000167X `RevenueShareRecordV1` གིས་བཏོན་མི་ གཞི་བསྟུན་གཞི་ཚོགས་ཚུ།
- རྩིས་ཞིབ་འབད་ནི་གི་དོན་ལུ་ `metadata` ནང་ལུ་ ཉམས་ཆག་/མེད་པའི་/གཤོག་སྒྲོ་རྡོ་གི་བྱུང་རིམ་ཚུ་ཚུདཔ་ཨིན།

## 6. དཔེ་ Norito དངུལ་སྤྲོད་པ།

### ༦.༡ མིང་ཐོབ།

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

### ༦.༢ རྗེས་འཇུག་པོ་ལི་སི་དཔེ།

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

## 7. རྗེས་འབྲེལ།- **SN-2b (Registrar API & གཞུང་སྐྱོང་ཧུཀ་):** བཀོད་སྒྲིག་འདི་ཚུ་ Torii (Norito དང་ JSON བཅིངས་པ) བརྒྱུད་དེ་ ཕྱིར་བཏོན་འབདཝ་ཨིན།
- **SN-3 (རིན་མེད་དང་ཐོ་བཀོད་འཕྲུལ་ཆ):* བསྐྱར་དུ་ལག་ལེན་འཐབ་ཨིན།
- **SN-5 (Payment & defectment):** དངུལ་འབྲེལ་མཐུན་སྒྲིག་དང་ འཕྲུལ་ཆས་སྙན་ཞུ་འབད་ནིའི་དོན་ལུ་ `RevenueShareRecordV1` གི་ ལྕགས་ཐག་ `RevenueShareRecordV1`.

དྲི་བ་ཡང་ན་བསྒྱུར་བཅོས་ཀྱི་ཞུ་བ་ཚུ་ I18NI000000172X ནང་ SNS ལམ་གྱི་ས་ཁྲ་དུས་མཐུན་ཚུ་དང་གཅིག་ཁར་ བཙུགས་དགོཔ་དང་ མཉམ་བསྡོམས་འབད་བའི་སྐབས་ I18NI000000173X ནང་ མེ་ལོང་བཟོ་དགོ།