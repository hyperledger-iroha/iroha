---
lang: ja
direction: ltr
source: docs/portal/docs/sns/registry-schema.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sns/registry_schema.md` کی عکاسی کرتا ہے اور پورٹل کی کینونیکل کاپی کے طور پر کام کرتا ہے۔ سورس فائل ترجمہ اپ ڈیٹس کے لیے برقرار رہے گی۔
:::

# Sora Name Service رجسٹری اسکیمہ (SN-2a)

**حیثیت:** مسودہ 2026-03-24 -- SNS پروگرام ریویو کے لیے جمع کیا گیا  
**روڈمیپ لنک:** SN-2a "Registry schema & storage layout"  
**دائرہ:** Sora Name Service (SNS) کے لیے Norito اسٹرکچرز، لائف سائیکل اسٹیٹس اور اخراجی ایونٹس متعین کرنا تاکہ registry اور registrar کی implementations کنٹریکٹس، SDKs اور gateways میں deterministic رہیں۔

یہ دستاویز SN-2a کے اسکیمہ ڈیلیورایبل کو مکمل کرتی ہے، جس میں درج ذیل شامل ہیں:

1. شناختیں اور hashing قواعد (`SuffixId`, `NameHash`, selector derivation)۔
2. نام ریکارڈز، suffix پالیسیز، قیمت tiers، ریونیو سپلٹس اور رجسٹری ایونٹس کے لیے Norito structs/enums۔
3. deterministic replay کے لیے storage layout اور index prefixes۔
4. رجسٹریشن، تجدید، grace/redemption، freeze اور tombstone پر مشتمل state machine۔
5. DNS/gateway automation کے لیے canonical events۔

## 1. شناختیں اور hashing

| شناخت | وضاحت | اخذ |
|------------|-------------|------------|
| `SuffixId` (`u16`) | ٹاپ لیول suffixes (`.sora`, `.nexus`, `.dao`) کے لیے رجسٹری شناخت۔ [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) کے suffix catalog کے مطابق۔ | governance vote سے مختص; `SuffixPolicyV1` میں محفوظ۔ |
| `SuffixSelector` | suffix کی canonical string شکل (ASCII, lower-case)۔ | مثال: `.sora` -> `sora`. |
| `NameSelectorV1` | رجسٹر شدہ لیبل کے لیے بائنری selector۔ | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. لیبل Norm v1 کے مطابق NFC + lower-case۔ |
| `NameHash` (`[u8;32]`) | کنٹریکٹس، ایونٹس اور caches کے لیے بنیادی سرچ key۔ | `blake3(NameSelectorV1_bytes)`. |

Determinism کی ضروریات:

- لیبلز Norm v1 (UTS-46 strict, STD3 ASCII, NFC) کے ذریعے normalize ہوتے ہیں۔ صارف کی strings کو hashing سے پہلے normalize کرنا لازم ہے۔
- Reserved labels (`SuffixPolicyV1.reserved_labels`) کبھی registry میں داخل نہیں ہوتے؛ governance-only overrides `ReservedNameAssigned` ایونٹس خارج کرتے ہیں۔

## 2. Norito ساختیں

### 2.1 NameRecordV1

| فیلڈ | قسم | نوٹس |
|-------|------|-------|
| `suffix_id` | `u16` | `SuffixPolicyV1` کی ریفرنس۔ |
| `selector` | `NameSelectorV1` | audit/debug کے لیے raw selector bytes۔ |
| `name_hash` | `[u8; 32]` | maps/events کے لیے key۔ |
| `normalized_label` | `AsciiString` | انسانی قابلِ پڑھائی لیبل (Norm v1 کے بعد)۔ |
| `display_label` | `AsciiString` | steward کی casing؛ اختیاری cosmetics۔ |
| `owner` | `AccountId` | renewals/transfers کنٹرول کرتا ہے۔ |
| `controllers` | `Vec<NameControllerV1>` | اکاؤنٹ ایڈریسز، resolvers یا ایپ metadata کے حوالہ جات۔ |
| `status` | `NameStatus` | لائف سائیکل فلیگ (سیکشن 4 دیکھیں)۔ |
| `pricing_class` | `u8` | suffix کے price tiers کا index (standard, premium, reserved)۔ |
| `registered_at` | `Timestamp` | ابتدائی activation کا بلاک ٹائم۔ |
| `expires_at` | `Timestamp` | ادا شدہ مدت کا اختتام۔ |
| `grace_expires_at` | `Timestamp` | auto-renew grace کا اختتام (default +30 دن)۔ |
| `redemption_expires_at` | `Timestamp` | redemption window کا اختتام (default +60 دن)۔ |
| `auction` | `Option<NameAuctionStateV1>` | Dutch reopen یا premium auctions کی صورت میں موجود۔ |
| `last_tx_hash` | `Hash` | اس ورژن کو پیدا کرنے والی ٹرانزیکشن کا deterministic pointer۔ |
| `metadata` | `Metadata` | registrar کی arbitrarily metadata (text records, proofs)۔ |

معاون structs:

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
    account_address: Option<AccountAddress>,   // Serialized as canonical `0x...` hex in JSON
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

| فیلڈ | قسم | نوٹس |
|-------|------|-------|
| `suffix_id` | `u16` | بنیادی key؛ پالیسی ورژنز میں مستحکم۔ |
| `suffix` | `AsciiString` | مثال کے طور پر `sora`۔ |
| `steward` | `AccountId` | governance charter میں متعین steward۔ |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | default settlement asset identifier (مثلا `xor#sora`)۔ |
| `pricing` | `Vec<PriceTierV1>` | قیمت کے tiers کے coefficients اور مدت کے قواعد۔ |
| `min_term_years` | `u8` | خریدی گئی مدت کے لیے کم از کم حد۔ |
| `grace_period_days` | `u16` | Default 30. |
| `redemption_period_days` | `u16` | Default 60. |
| `max_term_years` | `u8` | پیشگی تجدید کی زیادہ سے زیادہ مدت۔ |
| `referral_cap_bps` | `u16` | <=1000 (10%) charter کے مطابق۔ |
| `reserved_labels` | `Vec<ReservedNameV1>` | governance کی فراہم کردہ فہرست مع assign ہدایات۔ |
| `fee_split` | `SuffixFeeSplitV1` | treasury / steward / referral حصص (basis points)۔ |
| `fund_splitter_account` | `AccountId` | escrow رکھنے اور فنڈز تقسیم کرنے والا اکاؤنٹ۔ |
| `policy_version` | `u16` | ہر تبدیلی پر بڑھتا ہے۔ |
| `metadata` | `Metadata` | توسیعی نوٹس (KPI covenant, compliance doc hashes)۔ |

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

### 2.3 ریونیو اور settlement ریکارڈز

| Struct | فیلڈز | مقصد |
|--------|-------|------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | settlement epoch (ہفتہ وار) کے حساب سے routed ادائیگیوں کا deterministic ریکارڈ۔ |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | ہر ادائیگی پوسٹ ہونے پر emit (registration, renewal, auction)۔ |

تمام `TokenValue` فیلڈز Norito کی canonical fixed-point encoding استعمال کرتی ہیں اور کرنسی کوڈ متعلقہ `SuffixPolicyV1` میں declare ہوتا ہے۔

### 2.4 رجسٹری ایونٹس

Canonical events DNS/gateway automation اور analytics کے لیے replay log فراہم کرتے ہیں۔

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

ایونٹس کو replayable log (مثلا `RegistryEvents` domain) میں append کرنا ضروری ہے اور gateway feeds میں mirror کرنا ضروری ہے تاکہ DNS caches SLA کے اندر invalidate ہوں۔

## 3. Storage layout اور indexes

| Key | وضاحت |
|-----|-------------|
| `Names::<name_hash>` | `name_hash` سے `NameRecordV1` تک بنیادی map۔ |
| `NamesByOwner::<AccountId, suffix_id>` | wallet UI کے لیے ثانوی index (pagination friendly)۔ |
| `NamesByLabel::<suffix_id, normalized_label>` | conflicts detect کرتا ہے اور deterministic search فعال بناتا ہے۔ |
| `SuffixPolicies::<suffix_id>` | تازہ ترین `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` ہسٹری۔ |
| `RegistryEvents::<u64>` | append-only log جس کی key sequence monotonic ہے۔ |

تمام keys Norito tuples کے ساتھ serialize ہوتی ہیں تاکہ hosts کے درمیان hashing deterministic رہے۔ index updates بنیادی ریکارڈ کے ساتھ atomically ہوتی ہیں۔

## 4. Lifecycle state machine

| State | Entry Conditions | Allowed Transitions | Notes |
|-------|------------------|--------------------|-------|
| Available | جب `NameRecord` موجود نہ ہو۔ | `PendingAuction` (premium), `Active` (standard registration). | availability search صرف indexes پڑھتی ہے۔ |
| PendingAuction | جب `PriceTierV1.auction_kind` != none ہو۔ | `Active` (auction settles), `Tombstoned` (no bids). | auctions `AuctionOpened` اور `AuctionSettled` emit کرتی ہیں۔ |
| Active | رجسٹریشن یا تجدید کامیاب ہو۔ | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` transition چلاتا ہے۔ |
| GracePeriod | جب `now > expires_at` ہو۔ | `Active` (on-time renewal), `Redemption`, `Tombstoned`. | Default +30 دن; resolve ہوتا ہے مگر flag ہوتا ہے۔ |
| Redemption | `now > grace_expires_at` لیکن `< redemption_expires_at`. | `Active` (late renewal), `Tombstoned`. | کمانڈز پر penalty fee درکار ہے۔ |
| Frozen | governance یا guardian freeze۔ | `Active` (remediation کے بعد), `Tombstoned`. | transfer یا controllers update نہیں کر سکتے۔ |
| Tombstoned | رضاکارانہ surrender، مستقل dispute نتیجہ، یا redemption ختم۔ | `PendingAuction` (Dutch reopen) یا tombstoned رہتا ہے۔ | `NameTombstoned` ایونٹ میں وجہ شامل ہونی چاہیے۔ |

State transitions لازمی طور پر متعلقہ `RegistryEventKind` emit کریں تاکہ downstream caches ہم آہنگ رہیں۔ Tombstoned نام جو Dutch reopen auctions میں داخل ہوں وہ `AuctionKind::DutchReopen` payload شامل کرتے ہیں۔

## 5. Canonical events اور gateway sync

Gateways `RegistryEventV1` کو سبسکرائب کرتے ہیں اور DNS/SoraFS کو یوں sync کرتے ہیں:

1. ایونٹ sequence میں حوالہ کردہ تازہ ترین `NameRecordV1` حاصل کریں۔
2. resolver templates دوبارہ بنائیں (IH58 ترجیحی + compressed (`sora`) second‑best addresses, text records)۔
3. [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) میں بیان کردہ SoraDNS workflow کے ذریعے zone data pin کریں۔

Event delivery guarantees:

- ہر ٹرانزیکشن جو `NameRecordV1` پر اثر ڈالے *لازمی* طور پر `version` کے ساتھ صرف ایک ایونٹ شامل کرے جو سختی سے بڑھتی ہو۔
- `RevenueSharePosted` ایونٹس `RevenueShareRecordV1` سے emitted settlements کو reference کرتے ہیں۔
- freeze/unfreeze/tombstone ایونٹس audit replay کے لیے `metadata` میں governance artefact hashes شامل کرتے ہیں۔

## 6. Norito payloads کی مثالیں

### 6.1 NameRecord مثال

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
            account_address: Some(AccountAddress("0x02000001...")),
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

### 6.2 SuffixPolicy مثال

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

## 7. اگلے اقدامات

- **SN-2b (Registrar API & governance hooks):** ان structs کو Torii کے ذریعے expose کریں (Norito اور JSON bindings) اور admission checks کو governance artefacts سے جوڑیں۔
- **SN-3 (Auction & registration engine):** commit/reveal اور Dutch reopen logic کے لیے `NameAuctionStateV1` دوبارہ استعمال کریں۔
- **SN-5 (Payment & settlement):** مالی reconciliation اور رپورٹنگ automation کے لیے `RevenueShareRecordV1` استعمال کریں۔

سوالات یا تبدیلی کی درخواستیں `roadmap.md` میں SNS اپ ڈیٹس کے ساتھ درج کریں اور merge کے وقت `status.md` میں شامل کریں۔
