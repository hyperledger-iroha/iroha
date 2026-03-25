---
lang: ur
direction: rtl
source: docs/portal/docs/sns/registry-schema.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sns/registry_schema.md` کا آئینہ دار ہے اور پورٹل کی کیننیکل کاپی کے طور پر کام کرتا ہے۔ ماخذ فائل کو ترجمہ کی تازہ کاریوں کے لئے برقرار رکھا گیا ہے۔
:::

# سورہ نام سروس رجسٹری اسکیما (SN-2A)

** حیثیت: ** مسودہ 2026-03-24-ایس این ایس پروگرام کا جائزہ لینے کے لئے بھیجا گیا  
** روڈ میپ سے لنک: ** SN-2A "رجسٹری اسکیما اور اسٹوریج لے آؤٹ"  
** دائرہ کار: ** کیننیکل ڈھانچے Norito ، لائف سائیکل اسٹیٹس اور سورس نام کی خدمت (ایس این ایس) کے واقعات کی وضاحت کریں تاکہ رجسٹری اور رجسٹرار کے نفاذ معاہدوں ، ایس ڈی کے اور گیٹ ویز میں عین مطابق رہیں۔

یہ دستاویز SN-2A کے لئے اسکیماتی ترسیل کو مکمل کرتی ہے ، جس کی وضاحت:

1. شناخت کنندگان اور ہیشنگ کے قواعد (`SuffixId` ، `NameHash` ، سلیکٹرز کا اخذ)۔
2. Norito نام کے اندراجات ، لاحقہ پالیسیاں ، قیمت کے درجات ، محصولات کی تقسیم ، اور رجسٹری کے واقعات کے لئے ڈھانچے/enums۔
3. اسٹوریج لے آؤٹ اور انڈیکس پریفکس ڈٹرمینسٹک ری پلے کے لئے۔
4. رجسٹریشن ، تجدید ، فضل/چھٹکارا ، منجمد اور مقبرہ پتھر کا احاطہ کرنے والی اسٹیٹ مشین۔
5. DNS/گیٹ وے آٹومیشن کے ذریعہ استعمال ہونے والے کیننیکل واقعات۔

## 1۔ شناخت کنندہ اور ہیشنگ

| ID | تفصیل | مشتق |
| ------------ | ------------ | -------------- |
| `SuffixId` (`u16`) | اعلی سطح کے لاحقہ کے لئے رجسٹری ID (`.sora` ، `.nexus` ، `.dao`)۔ [`sns_suffix_governance_charter.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) میں لاحقہ کیٹلاگ کے ساتھ منسلک۔ | حکمرانی پر ووٹ کے ذریعہ مقرر ؛ `SuffixPolicyV1` میں محفوظ ہے۔ |
| `SuffixSelector` | لاحقہ کی کیننیکل اسٹرنگ فارم (ASCII ، لوئر کیس) | مثال: `.sora` -> `sora`۔ |
| `NameSelectorV1` | رجسٹرڈ لیبل کا بائنری سلیکٹر۔ | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`۔ نورم V1 کے مطابق NFC + لوئر کیس میں لیبل لگائیں۔ |
| `NameHash` (`[u8;32]`) | معاہدوں ، واقعات اور کیچوں کے ذریعہ استعمال شدہ بنیادی تلاش کی کلید۔ | `blake3(NameSelectorV1_bytes)`۔ |

عزم کے تقاضے:

- لیبلز کو نورم V1 (UTS-46 سخت ، STD3 ASCII ، NFC) کے ذریعے معمول بنایا جاتا ہے۔ ہیشنگ سے پہلے صارف کے ڈور کو معمول پر لانا ضروری ہے۔
- محفوظ لیبل (`SuffixPolicyV1.reserved_labels` سے) رجسٹری میں کبھی شامل نہیں ہوتے ہیں۔ گورننس صرف صرف اوور رائڈس ریلیز واقعات `ReservedNameAssigned`۔

## 2. ڈھانچے Norito

### 2.1 namerecordv1| فیلڈ | قسم | نوٹ |
| ------- | ------ | ----------- |
| `suffix_id` | `u16` | `SuffixPolicyV1` سے لنک کریں۔ |
| `selector` | `NameSelectorV1` | آڈٹ/ڈیبگ کے لئے را سلیکٹر بائٹس۔ |
| `name_hash` | `[u8; 32]` | نقشہ جات/واقعات کی کلید۔ |
| `normalized_label` | `AsciiString` | پڑھنے کے قابل لیبل (نورم V1 کے بعد)۔ |
| `display_label` | `AsciiString` | اسٹیورڈ کے ذریعہ کیسنگ ؛ کاسمیٹکس۔ |
| `owner` | `AccountId` | تجدید/منتقلی کا انتظام کرتا ہے۔ |
| `controllers` | `Vec<NameControllerV1>` | اکاؤنٹ کے پتے ، حل کرنے والے یا ایپلیکیشن میٹا ڈیٹا سے لنک۔ |
| `status` | `NameStatus` | لائف سائیکل پرچم (سیکشن 4 دیکھیں) |
| `pricing_class` | `u8` | قیمت کے درجے میں انڈیکس لاحقہ (معیاری ، پریمیم ، محفوظ) |
| `registered_at` | `Timestamp` | پرائمری ایکٹیویشن بلاک ٹائم۔ |
| `expires_at` | `Timestamp` | ادا شدہ مدت کا اختتام۔ |
| `grace_expires_at` | `Timestamp` | فضل کے اختتام پر آٹو رینو (پہلے سے طے شدہ +30 دن)۔ |
| `redemption_expires_at` | `Timestamp` | چھٹکارا ونڈو کا اختتام (پہلے سے طے شدہ +60 دن)۔ |
| `auction` | `Option<NameAuctionStateV1>` | ڈچ دوبارہ کھولنے یا پریمیم نیلامی میں موجود ہے۔ |
| `last_tx_hash` | `Hash` | ورژن کے لین دین کا تعی .ن پوائنٹر۔ |
| `metadata` | `Metadata` | مفت میٹا ڈیٹا رجسٹرار (متن کے ریکارڈ ، ثبوت) |

سپورٹ ڈھانچے:

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

### 2.2 suffixpolicyv1

| فیلڈ | قسم | نوٹ |
| ------- | ------ | ----------- |
| `suffix_id` | `u16` | بنیادی کلید ؛ پالیسی ورژن کے مابین مستحکم۔ |
| `suffix` | `AsciiString` | مثال کے طور پر ، `sora`۔ |
| `steward` | `AccountId` | اسٹیورڈ ، گورننس چارٹر میں تعریف کی گئی ہے۔ |
| `status` | `SuffixStatus` | `Active` ، `Paused` ، `Revoked`۔ |
| `payment_asset_id` | `AsciiString` | پہلے سے طے شدہ تصفیہ اثاثہ (مثال کے طور پر `61CtjvNd9T3THAR65GsMVHr82Bjc`)۔ |
| `pricing` | `Vec<PriceTierV1>` | درجے اور مدت کے قواعد کے ذریعہ قیمت کے گتانک۔ |
| `min_term_years` | `u8` | کم سے کم خریداری کی مدت قطع نظر کہ درجے کے اوور رائڈز سے قطع نظر۔ |
| `grace_period_days` | `u16` | پہلے سے طے شدہ 30. |
| `redemption_period_days` | `u16` | پہلے سے طے شدہ 60. |
| `max_term_years` | `u8` | ادائیگی کی زیادہ سے زیادہ مدت۔ |
| `referral_cap_bps` | `u16` | چارٹر کے مطابق <= 1000 (10 ٪)۔ |
| `reserved_labels` | `Vec<ReservedNameV1>` | منزل مقصود کی ہدایات کے ساتھ گورننس سے فہرست۔ |
| `fee_split` | `SuffixFeeSplitV1` | ٹریژری / اسٹیورڈ / ریفرل (بیس پوائنٹس) کے حصص۔ |
| `fund_splitter_account` | `AccountId` | ایسکرو اکاؤنٹ + فنڈز کی تقسیم۔ |
| `policy_version` | `u16` | ہر تبدیلی کے ساتھ بڑھتا ہے. |
| `metadata` | `Metadata` | توسیعی نوٹ (کے پی آئی عہد ، تعمیل ہیش دستاویزات)۔ |

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

### 2.3 انکم ریکارڈ اور تصفیہ| ڈھانچہ | فیلڈز | منزل |
| -------- | ------ | ----------- |
| `RevenueShareRecordV1` | `suffix_id` ، `epoch_id` ، `treasury_amount` ، `steward_amount` ، Norito ، `escrow_amount` ، `settled_at` ، Norito۔ | تصفیہ کے عہد (ہفتہ) کے ذریعہ تقسیم شدہ ادائیگیوں کا تعی .ن ریکارڈ۔ |
| `RevenueAccrualEventV1` | `name_hash` ، `suffix_id` ، `event` ، `gross_amount` ، `net_amount` ، `referral_account`۔ | ہر ادائیگی (رجسٹریشن ، تجدید ، نیلامی) کے ساتھ جاری کیا گیا۔ |

تمام `TokenValue` فیلڈز متعلقہ `SuffixPolicyV1` سے کرنسی کوڈ کے ساتھ کیننیکل فکسڈ انکوڈنگ Norito کا استعمال کرتے ہیں۔

### 2.4 رجسٹری کے واقعات

کیننیکل واقعات DNS/گیٹ وے آٹومیشن اور تجزیات کے لئے ری پلے لاگ فراہم کرتے ہیں۔

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

واقعات کو ری پلے ایبل لاگ (مثال کے طور پر ، ڈومین `RegistryEvents`) میں شامل کرنا ضروری ہے اور گیٹ وے فیڈز میں آئینہ دار ہے تاکہ ایس ایل اے کے اندر ڈی این ایس کیچز کو باطل کردیا جائے۔

## 3. اسٹوریج لے آؤٹ اور انڈیکس

| کلید | تفصیل |
| ----- | ------------ |
| `Names::<name_hash>` | مین کارڈ `name_hash` -> `NameRecordV1`۔ |
| `NamesByOwner::<AccountId, suffix_id>` | پرس UI (صفحہ بندی دوستانہ) کے لئے ثانوی انڈیکس۔ |
| `NamesByLabel::<suffix_id, normalized_label>` | تنازعہ کا پتہ لگانا ، عزم کی تلاش۔ |
| `SuffixPolicies::<suffix_id>` | موجودہ `SuffixPolicyV1`۔ |
| `RevenueShare::<suffix_id, epoch_id>` | تاریخ `RevenueShareRecordV1`۔ |
| `RegistryEvents::<u64>` | صرف ایک ہی اجارہ داری کی ترتیب کے ساتھ لاگ ان کریں۔ |

تمام چابیاں میزبانوں کے مابین ڈٹرمینسٹک ہیشنگ کے لئے Norito ٹوپلس کے ذریعہ سیریلائز کی جاتی ہیں۔ مرکزی ریکارڈ کے ساتھ انڈیکس اپڈیٹس ایٹمکی طور پر انجام دیئے جاتے ہیں۔

## 4۔ لائف سائیکل اسٹیٹ مشین

| ریاست | اندراج کے حالات | درست منتقلی | نوٹ |
| ------- | ------- | --------------------- | -------------- |
| دستیاب | جب `NameRecord` غائب ہو تو اخذ کیا گیا ہے۔ | `PendingAuction` (پریمیم) ، `Active` (معیاری رجسٹریشن)۔ | دستیابی کی تلاش صرف انڈیکس پڑھتی ہے۔ |
| زیر التواء | پھینک دیا گیا جب `PriceTierV1.auction_kind`! = کوئی نہیں۔ | `Active` (نیلامی طے شدہ) ، `Tombstoned` (کوئی بولی نہیں)۔ | نیلامی کا مسئلہ `AuctionOpened` اور `AuctionSettled`۔ |
| فعال | رجسٹریشن یا تجدید کامیاب۔ | `GracePeriod` ، `Frozen` ، `Tombstoned`۔ | `expires_at` منتقلی کو منتقل کرتا ہے۔ |
| فضل پیریڈ | خود بخود `now > expires_at` پر۔ | `Active` (وقت کی تجدید) ، `Redemption` ، `Tombstoned`۔ | پہلے سے طے شدہ +30 دن ؛ حل کرتا ہے ، لیکن نشان زد ہے۔ |
| چھٹکارا | `now > grace_expires_at` لیکن `< redemption_expires_at`۔ | `Active` (دیر سے تجدید) ، `Tombstoned`۔ | ٹیمیں جرمانے کی ادائیگی کا مطالبہ کررہی ہیں۔ |
| منجمد | گورننس یا سرپرست سے منجمد کریں۔ | `Active` (علاج کے بعد) ، `Tombstoned`۔ | کنٹرولرز کو منتقل یا اپ ڈیٹ نہیں کیا جاسکتا۔ |
| مقبرہ | رضاکارانہ طور پر ہتھیار ڈالنے ، تنازعہ یا میعاد ختم ہونے والے چھٹکارے کا نتیجہ۔ | `PendingAuction` (ڈچ دوبارہ کھلنے) یا ٹامبسٹونڈ رہتا ہے۔ | واقعہ `NameTombstoned` میں ایک وجہ شامل ہونی چاہئے۔ |ریاست کے منتقلی کو بہاو کیچوں کو مستقل رہنے کے ل the اسی طرح کے `RegistryEventKind` کا اخراج کرنا ہوگا۔ ڈچ دوبارہ کھولنے والی نیلامی میں شامل ٹومبسٹنڈ نام پے لوڈ `AuctionKind::DutchReopen` منسلک کرتے ہیں۔

## 5. کیننیکل واقعات اور گیٹ وے کی ہم آہنگی

گیٹ ویز `RegistryEventV1` کو سبسکرائب کریں اور DNS/SoraFS کو ہم آہنگ کریں: کر کے:

1. آخری `NameRecordV1` کو لوڈ کرنا واقعات کی ترتیب کی طرف اشارہ کیا۔
2. حل کرنے والے ٹیمپلیٹس کی تخلیق نو (I105 ترجیحی طور پر + کمپریسڈ (`sora`) دوسرے انتخاب کے طور پر ، متن کے ریکارڈ)۔
3. [`soradns_registry_rfc.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) سے Soradns ورک فلو کے توسط سے PIN اپ ڈیٹ شدہ زون کا ڈیٹا۔

واقعہ کی فراہمی کی ضمانتیں:

- `NameRecordV1` کو متاثر کرنے والے ہر ٹرانزیکشن * کو لازمی طور پر ایک واقعہ شامل کرنا ہوگا جس میں `version` میں سختی سے اضافہ ہوتا ہے۔
- `RevenueSharePosted` واقعات `RevenueShareRecordV1` سے بستیوں کا حوالہ دیتے ہیں۔
- آڈٹ ری پلے کے لئے `metadata` میں گورننس آرٹیکٹیکٹ ہیشوں کو منجمد/غیر منقولہ/مقبرہ واقعات میں شامل کریں۔

## 6. پے لوڈ کی مثالیں Norito

### 6.1 نمریکورڈ مثال

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "i105...",
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

### 6.2 لاحقہ مثال

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "i105...",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("i105..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "i105...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. اگلے اقدامات

۔
۔
- ** SN-5 (ادائیگی اور تصفیہ): ** مالی مفاہمت اور رپورٹ آٹومیشن کے لئے `RevenueShareRecordV1` استعمال کریں۔

`roadmap.md` میں SNS روڈ میپ اپڈیٹس کے ساتھ سوالات اور تبدیلی کی درخواستوں کو بھی ریکارڈ کیا جانا چاہئے اور جب ضم ہوجاتے ہیں تو `status.md` میں ظاہر ہوتا ہے۔