---
lang: he
direction: rtl
source: docs/portal/docs/sns/registry-schema.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f8df534d156b6a3e516cdd71b4c2f8ea2c6473e45afc643000e450d6d331190
source_last_modified: "2025-11-15T16:28:04.299911+00:00"
translation_last_reviewed: 2026-01-01
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sns/registry_schema.md` ומשמש כעותק הקנוני של הפורטל. קובץ המקור נשמר לעדכוני תרגום.
:::

# סכמת רישום Sora Name Service (SN-2a)

**סטטוס:** טיוטה 2026-03-24 -- הוגש לביקורת תוכנית SNS  
**קישור Roadmap:** SN-2a "Registry schema & storage layout"  
**תחום:** להגדיר את מבני Norito, מצבי מחזור חיים ואירועים עבור Sora Name Service (SNS), כדי שהמימושים של registry ו-registrar יהיו דטרמיניסטיים בין חוזים, SDKs ו-gateways.

מסמך זה משלים את מסירת הסכמה עבור SN-2a באמצעות הגדרה של:

1. מזהים וכללי hashing (`SuffixId`, `NameHash`, נגזרת selector).
2. Norito structs/enums לרשומות שם, מדיניות suffix, tiers תמחור, חלוקות הכנסה ואירועי registry.
3. layout אחסון ו-prefixes של אינדקסים ל-replay דטרמיניסטי.
4. מכונת מצבים לרישום, חידוש, grace/redemption, freeze ו-tombstone.
5. אירועים קנוניים שנצרכים על ידי אוטומציית DNS/gateway.

## 1. מזהים ו-hashing

| מזהה | תיאור | נגזרת |
|------------|-------------|------------|
| `SuffixId` (`u16`) | מזהה registry לסיומות עליונות (`.sora`, `.nexus`, `.dao`). מיושר לקטלוג הסיומות ב-[`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | מוקצה בהצבעת governance; מאוחסן ב-`SuffixPolicyV1`. |
| `SuffixSelector` | הצורה הקנונית של suffix כמחרוזת (ASCII, lower-case). | דוגמה: `.sora` -> `sora`. |
| `NameSelectorV1` | selector בינארי לתגית הרשומה. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. התגית היא NFC + lower-case לפי Norm v1. |
| `NameHash` (`[u8;32]`) | מפתח חיפוש ראשי לחוזים, אירועים ו-caches. | `blake3(NameSelectorV1_bytes)`. |

דרישות דטרמיניזם:

- תגיות מנורמלות באמצעות Norm v1 (UTS-46 strict, STD3 ASCII, NFC). מחרוזות משתמש חייבות נרמול לפני hashing.
- תגיות שמורות (`SuffixPolicyV1.reserved_labels`) אינן נכנסות לרישום; overrides של governance בלבד מנפיקים `ReservedNameAssigned`.

## 2. מבני Norito

### 2.1 NameRecordV1

| שדה | סוג | הערות |
|-------|------|-------|
| `suffix_id` | `u16` | הפניה ל-`SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | bytes גולמיים לצרכי audit/debug. |
| `name_hash` | `[u8; 32]` | מפתח maps/events. |
| `normalized_label` | `AsciiString` | תווית קריאה (לאחר Norm v1). |
| `display_label` | `AsciiString` | casing שסופק על ידי steward; קוסמטי. |
| `owner` | `AccountId` | שולט בחידושים/העברות. |
| `controllers` | `Vec<NameControllerV1>` | הפניות לכתובות חשבון, resolvers או metadata יישומית. |
| `status` | `NameStatus` | דגל מחזור חיים (ראו סעיף 4). |
| `pricing_class` | `u8` | אינדקס tiers תמחור של הסיומת (standard, premium, reserved). |
| `registered_at` | `Timestamp` | זמן בלוק של הפעלה ראשונית. |
| `expires_at` | `Timestamp` | סוף התקופה ששולמה. |
| `grace_expires_at` | `Timestamp` | סוף grace לחידוש אוטומטי (default +30 ימים). |
| `redemption_expires_at` | `Timestamp` | סוף חלון redemption (default +60 ימים). |
| `auction` | `Option<NameAuctionStateV1>` | קיים כאשר Dutch reopen או מכרזי premium פעילים. |
| `last_tx_hash` | `Hash` | מצביע דטרמיניסטי לטרנזקציה שיצרה את הגרסה. |
| `metadata` | `Metadata` | metadata שרירותי של registrar (text records, proofs). |

מבנים תומכים:

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

| שדה | סוג | הערות |
|-------|------|-------|
| `suffix_id` | `u16` | מפתח ראשי; יציב בין גרסאות מדיניות. |
| `suffix` | `AsciiString` | לדוגמה `sora`. |
| `steward` | `AccountId` | steward שמוגדר ב-charter של governance. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | מזהה נכס settlement ברירת מחדל (למשל `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | מקדמי תמחור לפי tiers וכללי משך. |
| `min_term_years` | `u8` | רצפה למשך רכישה ללא קשר ל-overrides. |
| `grace_period_days` | `u16` | Default 30. |
| `redemption_period_days` | `u16` | Default 60. |
| `max_term_years` | `u8` | מקסימום חידוש מראש. |
| `referral_cap_bps` | `u16` | <=1000 (10%) לפי charter. |
| `reserved_labels` | `Vec<ReservedNameV1>` | רשימה מה-governance עם הוראות הקצאה. |
| `fee_split` | `SuffixFeeSplitV1` | חלוקה בין treasury / steward / referral (basis points). |
| `fund_splitter_account` | `AccountId` | חשבון שמחזיק escrow ומחלק כספים. |
| `policy_version` | `u16` | גדל בכל שינוי. |
| `metadata` | `Metadata` | הערות מורחבות (KPI covenant, hashes של מסמכי compliance). |

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

### 2.3 רישומי הכנסות ו-settlement

| Struct | שדות | מטרה |
|--------|--------|-------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | רישום דטרמיניסטי של תשלומים מחולקים לפי epoch של settlement (שבועי). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | נפלט בכל פעם שתשלום נרשם (registration, renewal, auction). |

כל שדות `TokenValue` משתמשים בקידוד fixed-point הקנוני של Norito עם קוד מטבע שמוצהר ב-`SuffixPolicyV1` המתאים.

### 2.4 אירועי registry

אירועים קנוניים מספקים replay log לאוטומציה של DNS/gateway ולניתוח.

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

אירועים חייבים להירשם ב-log הניתן לשחזור (למשל domain `RegistryEvents`) ולהשתקף ל-gateway feeds כדי ש-DNS caches יתבטלו בתוך SLA.

## 3. layout אחסון ואינדקסים

| מפתח | תיאור |
|-----|-------------|
| `Names::<name_hash>` | מפה ראשית מ-`name_hash` ל-`NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | אינדקס משני ל-UI של wallet (pagination friendly). |
| `NamesByLabel::<suffix_id, normalized_label>` | זיהוי התנגשויות וחיפוש דטרמיניסטי. |
| `SuffixPolicies::<suffix_id>` | `SuffixPolicyV1` העדכני. |
| `RevenueShare::<suffix_id, epoch_id>` | היסטוריית `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | log append-only עם מפתח רצף מונוטוני. |

כל המפתחות מסודרים עם Norito tuples כדי לשמור hashing דטרמיניסטי בין hosts. עדכוני אינדקס מתבצעים אטומית לצד הרישום הראשי.

## 4. מכונת מצבים של מחזור חיים

| מצב | תנאי כניסה | מעברים מותרים | הערות |
|-------|----------------|---------------------|------------|
| Available | נגזר כאשר `NameRecord` חסר. | `PendingAuction` (premium), `Active` (standard registration). | חיפוש זמינות קורא רק אינדקסים. |
| PendingAuction | נוצר כאשר `PriceTierV1.auction_kind` != none. | `Active` (auction settles), `Tombstoned` (no bids). | מכרזים פולטים `AuctionOpened` ו-`AuctionSettled`. |
| Active | רישום או חידוש הצליחו. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` מניע מעבר. |
| GracePeriod | אוטומטי כאשר `now > expires_at`. | `Active` (on-time renewal), `Redemption`, `Tombstoned`. | Default +30 ימים; עדיין נפתר אך מסומן. |
| Redemption | `now > grace_expires_at` אך `< redemption_expires_at`. | `Active` (late renewal), `Tombstoned`. | פעולות דורשות קנס. |
| Frozen | Freeze של governance או guardian. | `Active` (לאחר remediation), `Tombstoned`. | אין אפשרות להעביר או לעדכן controllers. |
| Tombstoned | ויתור מרצון, תוצאה של מחלוקת קבועה או redemption שפג. | `PendingAuction` (Dutch reopen) או נשאר tombstoned. | אירוע `NameTombstoned` חייב לכלול סיבה. |

מעברי מצב חייבים לפלוט את `RegistryEventKind` המתאים כדי לשמור על עקביות caches downstream. שמות tombstoned שנכנסים ל-Dutch reopen מצרפים payload `AuctionKind::DutchReopen`.

## 5. אירועים קנוניים וסנכרון gateways

Gateways נרשמים ל-`RegistryEventV1` ומסנכרנים DNS/SoraFS דרך:

1. שליפת `NameRecordV1` האחרון שמוזכר ברצף האירועים.
2. יצירה מחדש של resolver templates (כתובות i105 מועדפות + i105 כאופציה שנייה, text records).
3. Pin לנתוני zone מעודכנים באמצעות זרימת SoraDNS המתוארת ב-[`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

ערבויות מסירת אירועים:

- כל טרנזקציה שמשפיעה על `NameRecordV1` *חייבת* להוסיף אירוע אחד בלבד עם `version` עולה בקפדנות.
- `RevenueSharePosted` מפנה ל-settlements שנפלטו על ידי `RevenueShareRecordV1`.
- אירועי freeze/unfreeze/tombstone כוללים hashes של artefacts governance בתוך `metadata` לצורך audit replay.

## 6. דוגמאות Norito payloads

### 6.1 דוגמת NameRecord

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

### 6.2 דוגמת SuffixPolicy

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

## 7. צעדים הבאים

- **SN-2b (Registrar API & governance hooks):** לחשוף את ה-structs הללו דרך Torii (Norito/JSON bindings) ולחבר admission checks לארטיפקטי governance.
- **SN-3 (Auction & registration engine):** להשתמש מחדש ב-`NameAuctionStateV1` ללוגיקת commit/reveal ו-Dutch reopen.
- **SN-5 (Payment & settlement):** להשתמש ב-`RevenueShareRecordV1` לצרכי reconciliation פיננסי ואוטומציה של דוחות.

שאלות או בקשות שינוי יש להגיש לצד עדכוני SNS ב-`roadmap.md` ולשקף ב-`status.md` בעת מיזוג.
