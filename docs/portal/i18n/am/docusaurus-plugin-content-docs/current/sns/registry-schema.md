---
id: registry-schema
lang: am
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

::: ማስታወሻ ቀኖናዊ ምንጭ
ይህ ገጽ `docs/source/sns/registry_schema.md`ን ያንጸባርቃል እና አሁን እንደ የ
ቀኖናዊ ፖርታል ቅጂ. የምንጭ ፋይሉ ለትርጉም ማሻሻያ ይቀራል።
::

# የሶራ ስም የአገልግሎት መዝገብ እቅድ (SN-2a)

**ሁኔታ:** የተረቀቀው 2026-03-24 -- ለኤስኤንኤስ ፕሮግራም ግምገማ ገብቷል  
** የመንገድ ካርታ አገናኝ፡** SN-2a “የመዝገብ ቤት እቅድ እና የማከማቻ አቀማመጥ”  
** ወሰን፡** ቀኖናዊውን Norito አወቃቀሮችን፣የህይወት ዑደት ግዛቶችን እና ለሶራ ስም አገልግሎት (ኤስኤንኤስ) የተለቀቁ ክስተቶችን ይግለጹ ስለዚህ የመመዝገቢያ እና የመዝጋቢ አተገባበር በኮንትራቶች፣ ኤስዲኬዎች እና መግቢያ መንገዶች ላይ ቆራጥ ሆነው ይቆያሉ።

ይህ ሰነድ የሚከተሉትን በመግለጽ ለ SN-2a ሊደርስ የሚችለውን እቅድ ያጠናቅቃል፡-

1. መለያዎች እና የሃሽንግ ደንቦች (`SuffixId`, `NameHash`, የመራጭ አመጣጥ).
2. Norito የስም መዝገቦችን ፣ ቅጥያ ፖሊሲዎችን ፣ የዋጋ ደረጃዎችን ፣ የገቢ ክፍፍልን እና የመመዝገቢያ ዝግጅቶችን ያዋቅራል።
3. የማከማቻ አቀማመጥ እና ጠቋሚ ቅድመ-ቅጥያዎች ለዳግም አጫውት።
4. የምዝገባ፣የእድሳት፣የጸጋ/መቤዠት፣የበረዶ እና የመቃብር ድንጋዮችን የሚሸፍን የመንግስት ማሽን።
5. ቀኖናዊ ክንውኖች በዲኤንኤስ/ጌትዌይ አውቶማቲክ ፍጆታ።

## 1. Identifiers & Hashing

| መለያ | መግለጫ | መነሻ |
|--------|------------|--------|
| `SuffixId` (`u16`) | ለከፍተኛ ደረጃ ቅጥያ (`.sora`፣ `.nexus`፣ `.dao`) የመመዝገቢያ-ሰፊ መለያ። በ[`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) ውስጥ ካለው ቅጥያ ካታሎግ ጋር የተስተካከለ። | በአስተዳደር ድምጽ የተመደበ; በ `SuffixPolicyV1` ውስጥ ተከማችቷል. |
| `SuffixSelector` | የቀኖናዊ ሕብረቁምፊ ቅጽ የቅጥያ (ASCII፣ ንዑስ ሆሄ)። | ምሳሌ፡- `.sora` → `sora`። |
| `NameSelectorV1` | ለተመዘገበው መለያ ሁለትዮሽ መራጭ። | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. መለያው NFC + ንዑስ ሆሄ በእያንዳንዱ Normv1 ነው። |
| `NameHash` (`[u8;32]`) | በኮንትራቶች፣ ክስተቶች እና መሸጎጫዎች ጥቅም ላይ የዋለ ዋና ፍለጋ ቁልፍ። | `blake3(NameSelectorV1_bytes)`. |

የውሳኔ መስፈርቶች፡-

- መለያዎች በ Normv1 በኩል መደበኛ ናቸው (UTS-46 ጥብቅ፣ STD3 ASCII፣ NFC)። ገቢ የተጠቃሚ ሕብረቁምፊዎች ከመጥለፍዎ በፊት መደበኛ መሆን አለባቸው።
- የተያዙ መለያዎች (ከ `SuffixPolicyV1.reserved_labels`) በጭራሽ ወደ መዝገቡ ውስጥ አይገቡም; አስተዳደር-ብቻ የ `ReservedNameAssigned` ክስተቶችን ይሽራል።

## 2. Norito መዋቅሮች

### 2.1 ስም መዝገብ ቪ1| መስክ | አይነት | ማስታወሻ |
|-------|------|------|
| `suffix_id` | `u16` | ማጣቀሻዎች `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | ለኦዲት/ለማረም ጥሬ መራጭ ባይት። |
| `name_hash` | `[u8; 32]` | ለካርታዎች/ክስተቶች ቁልፍ። |
| `normalized_label` | `AsciiString` | ሰው ሊነበብ የሚችል መለያ (ፖስት Normv1)። |
| `display_label` | `AsciiString` | በመጋቢ የቀረበ መያዣ; አማራጭ መዋቢያዎች. |
| `owner` | `AccountId` | እድሳት/ማስተላለፎችን ይቆጣጠራል። |
| `controllers` | `Vec<NameControllerV1>` | ማጣቀሻዎች መለያ አድራሻዎችን፣ ፈላጊዎችን ወይም የመተግበሪያ ዲበ ውሂብን ያነጣጠሩ። |
| `status` | `NameStatus` | የሕይወት ዑደት ባንዲራ (ክፍል 4 ይመልከቱ)። |
| `pricing_class` | `u8` | ወደ ቅጥያ የዋጋ ደረጃዎች (መደበኛ፣ ፕሪሚየም፣ የተያዘ) ማውጫ። |
| `registered_at` | `Timestamp` | የመጀመሪያውን ማግበር የጊዜ ማህተምን አግድ። |
| `expires_at` | `Timestamp` | የሚከፈልበት ጊዜ ማብቂያ። |
| `grace_expires_at` | `Timestamp` | በራስ የመታደስ ጸጋ መጨረሻ (ነባሪ +30 ቀናት)። |
| `redemption_expires_at` | `Timestamp` | የቤዛ መስኮት መጨረሻ (ነባሪ +60 ቀናት)። |
| `auction` | `Option<NameAuctionStateV1>` | ደች እንደገና ሲከፈቱ ወይም ፕሪሚየም ጨረታዎች ንቁ ሲሆኑ ያቅርቡ። |
| `last_tx_hash` | `Hash` | ይህንን እትም ላዘጋጀው ግብይት ቆራጥ ጠቋሚ። |
| `metadata` | `Metadata` | የዘፈቀደ የመዝጋቢ ዲበዳታ (የጽሑፍ መዝገቦች፣ ማረጋገጫዎች)። |

ድጋፍ ሰጪ መዋቅሮች፡-

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

| መስክ | አይነት | ማስታወሻ |
|-------|------|------|
| `suffix_id` | `u16` | ዋና ቁልፍ; በመመሪያ ስሪቶች ላይ የተረጋጋ። |
| `suffix` | `AsciiString` | ለምሳሌ `sora`. |
| `steward` | `AccountId` | መጋቢ በአስተዳደር ቻርተር ውስጥ ተገልጿል. |
| `status` | `SuffixStatus` | `Active`፣ `Paused`፣ `Revoked`። |
| `payment_asset_id` | `AsciiString` | ነባሪ የሰፈራ ንብረት መለያ (ለምሳሌ `xor#sora`)። |
| `pricing` | `Vec<PriceTierV1>` | ደረጃውን የጠበቀ የዋጋ አወጣጥ እና የቆይታ ጊዜ ህጎች። |
| `min_term_years` | `u8` | ደረጃ መሻር ምንም ይሁን ምን ወለል ለተገዛው ጊዜ። |
| `grace_period_days` | `u16` | ነባሪ 30. |
| `redemption_period_days` | `u16` | ነባሪ 60. |
| `max_term_years` | `u8` | ከፍተኛው የፊት እድሳት ርዝመት። |
| `referral_cap_bps` | `u16` | <=1000 (10%) በቻርተር። |
| `reserved_labels` | `Vec<ReservedNameV1>` | አስተዳደር ከምደባ መመሪያዎች ጋር የቀረበ ዝርዝር። |
| `fee_split` | `SuffixFeeSplitV1` | የግምጃ ቤት / መጋቢ / ሪፈራል ማጋራቶች (መሰረታዊ ነጥቦች). |
| `fund_splitter_account` | `AccountId` | ኤስክሮው + ገንዘብ የሚያከፋፍል መለያ |
| `policy_version` | `u16` | በእያንዳንዱ ለውጥ ላይ ተጨምሯል. |
| `metadata` | `Metadata` | የተራዘሙ ማስታወሻዎች (KPI ቃል ኪዳን፣ ተገዢነት ሰነድ hashes)። |

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

### 2.3 የገቢ እና የሰፈራ መዝገቦች| መዋቅር | መስኮች | ዓላማ |
|--------|--------|-----|
| `RevenueShareRecordV1` | `suffix_id`፣ `epoch_id`፣ `treasury_amount`፣ `steward_amount`፣ `referral_amount`፣ `escrow_amount`፣ Norito፣ Norito፣ | በየመቋቋሚያ ዘመን (በሳምንት) የተላለፉ ክፍያዎች ቆራጥነት። |
| `RevenueAccrualEventV1` | `name_hash`፣ `suffix_id`፣ `event`፣ `gross_amount`፣ `net_amount`፣ `referral_account`። | በእያንዳንዱ ጊዜ የክፍያ ልጥፎች (ምዝገባ፣ እድሳት፣ ጨረታ) ተለቀቀ። |

ሁሉም የ`TokenValue` መስኮች የNorito ቀኖናዊ ቋሚ ነጥብ ኢንኮዲንግ በተዛማጅ `SuffixPolicyV1` ውስጥ ከተገለጸው የምንዛሪ ኮድ ጋር ይጠቀማሉ።

### 2.4 የመመዝገቢያ ክስተቶች

ቀኖናዊ ክስተቶች ለዲ ኤን ኤስ/ጌትዌይ አውቶሜሽን እና ትንታኔዎች የመልሶ ማጫወት ምዝግብ ማስታወሻን ይሰጣሉ።

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

የዲ ኤን ኤስ መሸጎጫዎች በኤስኤልኤ ውስጥ ዋጋ እንዳይኖራቸው ክስተቶች እንደገና ሊጫወት በሚችል ምዝግብ ማስታወሻ (ለምሳሌ `RegistryEvents` ጎራ) መታከል እና ወደ መግቢያ በር መግጠም አለባቸው።

## 3. የማከማቻ አቀማመጥ እና ኢንዴክሶች

| ቁልፍ | መግለጫ |
|-------------|
| `Names::<name_hash>` | ዋና ካርታ ከ `name_hash` እስከ `NameRecordV1`። |
| `NamesByOwner::<AccountId, suffix_id>` | ሁለተኛ ደረጃ መረጃ ጠቋሚ ለኪስ ዩአይ (ገጽ ተስማሚ)። |
| `NamesByLabel::<suffix_id, normalized_label>` | ግጭቶችን ፈልግ, የኃይል መወሰኛ ፍለጋ. |
| `SuffixPolicies::<suffix_id>` | የቅርብ ጊዜ `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` ታሪክ። |
| `RegistryEvents::<u64>` | አባሪ-ብቻ ምዝግብ ማስታወሻ በተናጥል እየጨመረ በቅደም ተከተል። |

ሃሺንግ ቆራጥነት በሁሉም አስተናጋጆች ለመቀጠል ሁሉም ቁልፎች Norito tuples በመጠቀም ተከታታይነት አላቸው። የመረጃ ጠቋሚ ዝማኔዎች ከዋናው መዝገብ ጋር በአቶሚክ ይከሰታሉ።

## 4. Lifecycle ግዛት ማሽን

| ግዛት | የመግቢያ ሁኔታዎች | የተፈቀዱ ሽግግሮች | ማስታወሻ |
|-------------
| ይገኛል | `NameRecord` በማይኖርበት ጊዜ የተገኘ። | `PendingAuction` (ፕሪሚየም)፣ `Active` (መደበኛ መዝገብ)። | የተገኝነት ፍለጋ ኢንዴክሶችን ብቻ ያነባል። |
| በመጠባበቅ ላይ ያለ ጨረታ | `PriceTierV1.auction_kind` ≠ ምንም ሲፈጠር የተፈጠረ። | `Active` (ጨረታ ተሽጧል)፣ `Tombstoned` (ጨረታዎች የሉም)። | ጨረታዎች `AuctionOpened` እና `AuctionSettled` ያወጣሉ። |
| ንቁ | መመዝገብ ወይም መታደስ ተሳክቷል። | `GracePeriod`፣ `Frozen`፣ `Tombstoned`። | `expires_at` ድራይቮች ሽግግር. |
| GracePeriod | በራስ-ሰር `now > expires_at`. | `Active` (በጊዜ እድሳት)፣ `Redemption`፣ `Tombstoned`። | ነባሪ +30 ቀናት; አሁንም ይፈታል ግን ምልክት ተደርጎበታል። |
| ቤዛነት | `now > grace_expires_at` ግን `< redemption_expires_at`። | `Active` (ዘግይቶ መታደስ), `Tombstoned`. | ትዕዛዞች የቅጣት ክፍያ ያስፈልጋቸዋል። |
| የቀዘቀዘ | አስተዳደር ወይም ሞግዚት ይቀዘቅዛል። | `Active` (ከማስተካከል በኋላ), `Tombstoned`. | መቆጣጠሪያዎችን ማስተላለፍ ወይም ማዘመን አይቻልም። |
| የመቃብር ድንጋይ | በፈቃደኝነት እጅ መስጠት፣ የቋሚ አለመግባባት ውጤት ወይም ጊዜው ያለፈበት ቤዛነት። | `PendingAuction` (ደች እንደገና ተከፈተ) ወይም የመቃብር ድንጋይ ሆኖ ይቀራል። | ክስተት `NameTombstoned` ምክንያት ማካተት አለበት. |

የስቴት ሽግግሮች ተዛማጁን `RegistryEventKind` መልቀቅ አለባቸው ስለዚህ የታችኛው ተፋሰስ መሸጎጫዎች ወጥነት ይኖራቸዋል። ወደ ደች የተከፈቱ ጨረታዎች የሚገቡ የመቃብር ድንጋይ ስሞች የ`AuctionKind::DutchReopen` ጭነትን አያይዘዋል።

## 5. ቀኖናዊ ክስተቶች እና ጌትዌይ ማመሳሰልጌትዌይስ ለ`RegistryEventV1` ደንበኝነት ይመዝገቡ እና ከ DNS/SoraFS ጋር ያመሳስሉ በ፡

1. በክስተቱ ቅደም ተከተል የተጠቀሰውን የቅርብ ጊዜውን `NameRecordV1` በማምጣት ላይ።
2. የመፍታት አብነቶችን እንደገና ማመንጨት (የተመረጡ IH58 + ሁለተኛ-ምርጥ የታመቁ (`sora`) አድራሻዎች ፣ የጽሑፍ መዝገቦች)።
3. በ[`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) ውስጥ በተገለጸው በሶራዲኤንኤስ የስራ ፍሰት በኩል የዘመነውን የዞን መረጃ ማሰር።

የክስተት ማቅረቢያ ዋስትናዎች;

- በ `NameRecordV1` * የግድ* የሚነካ እያንዳንዱ ግብይት በትክክል አንድ ክስተት ከ `version` ጋር መጨመር አለበት።
- በ `RevenueSharePosted` ክስተቶች የማጣቀሻ ሰፈራዎች በ `RevenueShareRecordV1`.
- የቀዝቃዛ/የማይዝግ/የመቃብር ድንጋይ ዝግጅቶች በ`metadata` ውስጥ የኦዲት ድጋሚ ለማጫወት የአስተዳደር ቅርስ ሃሽዎችን ያካትታሉ።

## 6. ምሳሌ Norito ክፍያ

### 6.1 የስም መዝገብ ምሳሌ

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

### 6.2 SuffixPolicy ምሳሌ

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

## 7. ቀጣይ ደረጃዎች

- ** SN-2b (የሬጅስትራር ኤፒአይ እና የአስተዳደር መንጠቆዎች):** እነዚህን መዋቅሮች በTorii (Norito እና JSON ማሰሪያ) እና የሽቦ የመግቢያ ቼኮችን ለአስተዳደር ቅርሶች ያጋልጡ።
- ** SN-3 (ጨረታ እና መመዝገቢያ ሞተር):** `NameAuctionStateV1` ቁርጠኝነት/መግለጥ እና ደች እንደገና ለመክፈት አመክንዮ እንደገና ይጠቀሙ።
- ** SN-5 (ክፍያ እና ማቋቋሚያ):** ለፋይናንስ ማስታረቅ እና ለሪፖርት አውቶሜሽን `RevenueShareRecordV1` leverage.

ጥያቄዎች ወይም የለውጥ ጥያቄዎች ከኤስኤንኤስ የመንገድ ካርታ ዝመናዎች ጋር በ`roadmap.md` እና በ `status.md` ውስጥ ሲዋሃዱ መንጸባረቅ አለባቸው።