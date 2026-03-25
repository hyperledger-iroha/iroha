---
lang: ur
direction: rtl
source: docs/portal/docs/sns/registry-schema.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ معیاری ماخذ
یہ `docs/source/sns/registry_schema.md` کی آئینہ دار ہے اور اب معیاری گیٹ وے ورژن کے طور پر کام کرتا ہے۔ ماخذ فائل ترجمہ کی تازہ کاریوں کے لئے باقی ہے۔
:::

# سورہ نام سروس لاگ اسکیما (SN-2A)

** حیثیت: ** مسودہ 2026-03-24-ایس این ایس پروگرام کے جائزے کے لئے پیش کیا گیا  
** روڈ میپ لنک: ** SN-2A "رجسٹری اسکیما اور اسٹوریج لے آؤٹ"  
** دائرہ کار: ** معیاری Norito ڈھانچے ، لائف سائیکل اسٹیٹس ، اور SORA نام سروس (SNS) کے لئے خارج ہونے والے واقعات کی وضاحت کریں تاکہ یہ یقینی بنایا جاسکے کہ معاہدوں ، SDKs اور گیٹ ویز میں رجسٹری اور رجسٹرار کے نفاذ کا تعی .ن ہے۔

یہ دستاویز SN-2A کے لئے چارٹ کی ترسیل کو یہ بتاتے ہوئے مکمل کرتی ہے:

1. شناخت کنندگان اور ہیشنگ کے قواعد (`SuffixId` ، `NameHash` ، سلیکٹر مشتق)۔
2. Norito نام کے ریکارڈ ، لاحقہ پالیسیاں ، قیمتوں کا تعین کرنے والے درجے ، محصولات مختص کرنے ، اور رجسٹری کے واقعات کے لئے ڈھانچے/enums۔
3. ذخیرہ کرنے کی منصوبہ بندی اور انڈیکس کے سابقہ ​​عین مطابق ری پلے کو یقینی بنانے کے لئے۔
4. مقدمات رجسٹریشن ، تجدید ، فضل/چھٹکارے ، منجمد اور مقبرہ پتھر کا احاطہ کرتے ہیں۔
5. DNS/گیٹ وے آٹومیشن کے ذریعہ استعمال ہونے والے معیاری واقعات۔

## 1۔ شناخت کنندہ اور ہیشنگ

| ID | تفصیل | مشتق |
| ------------ | ------------- | -------------- |
| `SuffixId` (`u16`) | اوپری SFX (`.sora` ، `.nexus` ، `.dao`) کے لئے ریکارڈ ID۔ [`sns_suffix_governance_charter.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) پر SOFX کیٹلاگ کے ساتھ ہم آہنگ۔ | گورننس ووٹ کے ذریعہ مقرر ؛ `SuffixPolicyV1` پر اسٹورز۔ |
| `SuffixSelector` | SOFX (ASCII ، لوئر کیس) کے لئے معیاری متن کی شکل۔ | مثال: `.sora` -> `sora`۔ |
| `NameSelectorV1` | رجسٹرڈ ٹیگ کے لئے بائنری سلیکٹر۔ | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`۔ نورم V1 کے مطابق ٹیگ این ایف سی + لوئر کیس۔ |
| `NameHash` (`[u8;32]`) | معاہدوں ، واقعات اور کیچوں میں استعمال ہونے والی بنیادی تلاش کی کلید۔ | `blake3(NameSelectorV1_bytes)`۔ |

تعی .ن کی ضروریات:

- ٹیگز کو معمول کے مطابق V1 (UTS-46 سخت ، STD3 ASCII ، NFC) کے ذریعے معمول بنایا جاتا ہے۔ ہیشنگ سے پہلے صارف کے ڈور کو معمول پر لانا ضروری ہے۔
- محفوظ ٹیگز (`SuffixPolicyV1.reserved_labels` سے) رجسٹری میں کبھی داخل نہیں ہوتے ہیں۔ گورننس کی خلاف ورزی صرف واقعات `ReservedNameAssigned` جاری کرتی ہے۔

## 2. Norito ڈھانچے

### 2.1 namerecordv1| فیلڈ | قسم | نوٹ |
| ------- | ------ | ----------- |
| `suffix_id` | `u16` | حوالہ `SuffixPolicyV1`۔ |
| `selector` | `NameSelectorV1` | آڈٹ/ڈیبگ کے لئے را سلیکٹر بائٹس۔ |
| `name_hash` | `[u8; 32]` | نقشہ جات/واقعات کی کلید۔ |
| `normalized_label` | `AsciiString` | پڑھنے کے قابل ٹیگ (نورم V1 کے بعد)۔ |
| `display_label` | `AsciiString` | اسٹیورڈ کے ذریعہ فراہم کردہ کردار کی شکل ؛ کاسمیٹک۔ |
| `owner` | `AccountId` | تجدیدات/منتقلی کو کنٹرول کرتا ہے۔ |
| `controllers` | `Vec<NameControllerV1>` | درخواست کے لئے اکاؤنٹ کے عنوانات ، حل کرنے والے ، یا میٹا ڈیٹا کا حوالہ۔ |
| `status` | `NameStatus` | زندگی سائیکل کی حیثیت (سیکشن 4 دیکھیں)۔ |
| `pricing_class` | `u8` | سوفوریکس (معیاری ، پریمیم ، محفوظ) کے لئے قیمتوں کا تعین کرنے کا اشاریہ۔ |
| `registered_at` | `Timestamp` | پہلی رجسٹریشن کو چالو کرنے کا وقت۔ |
| `expires_at` | `Timestamp` | ادا شدہ مدت کا اختتام۔ |
| `grace_expires_at` | `Timestamp` | گریس اینڈ آٹو تجدید (پہلے سے طے شدہ +30 دن)۔ |
| `redemption_expires_at` | `Timestamp` | چھٹکارا ونڈو کا اختتام (پہلے سے طے شدہ +60 دن)۔ |
| `auction` | `Option<NameAuctionStateV1>` | اس وقت ظاہر ہوتا ہے جب ڈچ دوبارہ کھولیں یا پریمیم نیلامی فعال ہوں۔ |
| `last_tx_hash` | `Hash` | اس کاپی تیار کرنے والے اس لین دین کا ایک عین مطابق اشارے۔ |
| `metadata` | `Metadata` | میٹا ڈیٹا رجسٹرار (ٹیکسٹ ریکارڈز ، ثبوت) کے لئے اختیاری ہے۔ |

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
| `suffix_id` | `u16` | پالیسی کی نقل میں مستقل بنیادی کلید۔ |
| `suffix` | `AsciiString` | مثال کے طور پر `sora`۔ |
| `steward` | `AccountId` | گورننس چارٹر میں اسٹیورڈ آئی ڈی۔ |
| `status` | `SuffixStatus` | `Active` ، `Paused` ، `Revoked`۔ |
| `payment_asset_id` | `AsciiString` | پہلے سے طے شدہ تصفیہ ID (جیسے `61CtjvNd9T3THAR65GsMVHr82Bjc`)۔ |
| `pricing` | `Vec<PriceTierV1>` | درجات اور مدت کے قواعد کے ذریعہ قیمتوں کا تعین کرنا۔ |
| `min_term_years` | `u8` | کم سے کم خریداری کی مدت قطع نظر اس سے قطع نظر کہ اوور رائڈز۔ |
| `grace_period_days` | `u16` | پہلے سے طے شدہ 30. |
| `redemption_period_days` | `u16` | پہلے سے طے شدہ 60. |
| `max_term_years` | `u8` | زیادہ سے زیادہ پیشگی تجدید۔ |
| `referral_cap_bps` | `u16` | چارٹر کے مطابق <= 1000 (10 ٪)۔ |
| `reserved_labels` | `Vec<ReservedNameV1>` | حسب ضرورت ہدایات کے ساتھ گورننس کی فہرست۔ |
| `fee_split` | `SuffixFeeSplitV1` | ٹریژری/اسٹیورڈ/ریفرل شیئرز (بنیاد پوائنٹس) |
| `fund_splitter_account` | `AccountId` | اکاؤنٹ میں یسکرو ہوتا ہے اور فنڈز کو تقسیم کرتا ہے۔ |
| `policy_version` | `u16` | یہ ہر تبدیلی کے ساتھ بڑھتا ہے۔ |
| `metadata` | `Metadata` | توسیعی نوٹ (کے پی آئی عہد ، تعمیل دستاویزات کے لئے ہیش)۔ |

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

### 2.3 محصول اور تصفیے کے ریکارڈ| ڈھانچہ | فیلڈز | مقصد |
| -------- | -------- | ------- |
| `RevenueShareRecordV1` | `suffix_id` ، `epoch_id` ، `treasury_amount` ، `steward_amount` ، `referral_amount` ، `escrow_amount` ، `settled_at` ، `tx_hash`۔ | آبادکاری کے دور (ہفتہ وار) کے ذریعہ ٹوٹ جانے والی ادائیگیوں کا تعی .ن ریکارڈ۔ |
| `RevenueAccrualEventV1` | `name_hash` ، `suffix_id` ، `event` ، `gross_amount` ، `net_amount` ، `referral_account`۔ | یہ ہر ادائیگی (رجسٹریشن ، تجدید ، نیلامی) کے اندراج کے بعد جاری کیا جاتا ہے۔ |

تمام `TokenValue` فیلڈز کرنسی کوڈ کے ساتھ نوریٹو کی معیاری ہارڈ کوڈنگ کا استعمال کرتے ہیں جس سے متعلقہ `SuffixPolicyV1` میں اعلان کیا گیا ہے۔

### 2.4 لاگ واقعات

معیاری واقعات DNS/گیٹ وے آپریشنز اور اسکیلنگ کا ری پلے لاگ فراہم کرتے ہیں۔

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

واقعات کو ایک تولیدی لاگ (جیسے `RegistryEvents` رینج) میں شامل کرنا چاہئے اور گیٹ وے کی فیڈز میں جھلکتی ہے جب تک کہ ایس ایل اے کے اندر ڈی این ایس کیچز ختم نہ ہوں۔

## 3. اسٹوریج لے آؤٹ اور انڈیکس

| کلید | تفصیل |
| ----- | ------------- |
| `Names::<name_hash>` | `name_hash` سے `NameRecordV1` سے بیس نقشہ۔ |
| `NamesByOwner::<AccountId, suffix_id>` | بٹوے (صفحہ بندی دوستانہ) انٹرفیس کے لئے ثانوی انڈیکس۔ |
| `NamesByLabel::<suffix_id, normalized_label>` | تنازعات کا پتہ لگائیں اور عصبی تلاش کو قابل بنائیں۔ |
| `SuffixPolicies::<suffix_id>` | ایک اور `SuffixPolicyV1`۔ |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` کو رجسٹر کریں۔ |
| `RegistryEvents::<u64>` | صرف ایک ہی سیریل کلید کے ساتھ ضمیمہ لاگ ان کریں۔ |

تمام چابیاں Norito Tuples کے ذریعے سیریلائز کی جاتی ہیں تاکہ میزبانوں میں عین مطابق ہیشنگ کو برقرار رکھا جاسکے۔ انڈیکس اپڈیٹس بیس ریکارڈ کے ساتھ جوہری طور پر کی جاتی ہیں۔

## 4۔ لائف سائیکل اسٹیٹس مشین

| حیثیت | اندراج کے حالات | منتقلی کے نوٹوں کی اجازت |
| ---------------------- | ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| دستیاب | جب `NameRecord` غیر حاضر ہے تو اخذ کیا گیا ہے۔ | `PendingAuction` (پریمیم) ، `Active` (معیاری رجسٹریشن)۔ | دستیابی کی تلاش صرف انڈیکس پڑھتی ہے۔ |
| زیر التواء | جب `PriceTierV1.auction_kind`! = کوئی نہیں۔ | `Active` (نیلامی تصفیہ) ، `Tombstoned` (کوئی بولی نہیں)۔ | نیلامی `AuctionOpened` اور `AuctionSettled` جاری کررہی ہے۔ |
| فعال | کامیاب رجسٹریشن یا تجدید۔ | `GracePeriod` ، `Frozen` ، `Tombstoned`۔ | `expires_at` منتقلی کو آگے بڑھاتا ہے۔ |
| فضل پیریڈ | `now > expires_at` پر آٹو۔ | `Active` (وقت میں تجدید) ، `Redemption` ، `Tombstoned`۔ | پہلے سے طے شدہ +30 دن ؛ وہ اب بھی حل کر رہا ہے ، لیکن وہ ایک استاد ہے۔ |
| چھٹکارا | `now > grace_expires_at` لیکن `< redemption_expires_at`۔ | `Active` (دیر سے تجدید) ، `Tombstoned`۔ | احکامات میں جرمانے کی فیس کی ضرورت ہوتی ہے۔ |
| منجمد | گورننس یا سرپرست منجمد کریں۔ | `Active` (پروسیسنگ کے بعد) ، `Tombstoned`۔ | نام یا اپ ڈیٹ کنٹرولرز کو منتقل نہیں کرسکتے ہیں۔ |
| مقبرہ | رضاکارانہ مراعات ، مستقل تنازعہ ، یا چھٹکارے کے نتیجے میں۔ | `PendingAuction` (ڈچ دوبارہ کھلنے) یا ٹامبسٹونڈ رہتا ہے۔ | واقعہ `NameTombstoned` اس کی وجہ کو شامل کرنا چاہئے۔ |

`RegistryEventKind` کے مطابق ریاستی منتقلی کو خارج کرنا ضروری ہے تاکہ بہاو کیچوں کو مستقل رہنے کے ل .۔ ڈچ جون کی نیلامی میں مقبرہ ناموں میں داخل ہونے والے ناموں میں پے لوڈ `AuctionKind::DutchReopen` شامل ہیں۔

## 5. معیاری واقعات اور گیٹ ویز کے لئے مطابقت پذیری

گیٹ ویز شیئر `RegistryEventV1` اور DNS/SoraFS ہم وقت سازی کے ذریعے:

1. آخری `NameRecordV1` کو واقعات کی ترتیب میں حوالہ دیا گیا۔
2. دوبارہ حل کرنے والے ریزولور ٹیمپلیٹس (ترجیحی I105 ایڈریس + کمپریسڈ (`sora`) دوسرے آپشن کے طور پر ، ٹیکسٹ ریکارڈز)۔
3. [`soradns_registry_rfc.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) میں بیان کردہ Soradns فلو کے ذریعے زون کا ڈیٹا انسٹال کریں۔واقعہ کی فراہمی کی ضمانتیں:

- `NameRecordV1` کو متاثر کرنے والے ہر ٹرانزیکشن کو * `version` کے ساتھ صرف ایک ہی پروگرام شامل کرنا ہوگا۔
- واقعات `RevenueSharePosted` `RevenueShareRecordV1` کے ذریعہ جاری کردہ بستیوں کا حوالہ دیتے ہیں۔
- منجمد/غیر منقولہ/مقبرے کے واقعات میں `metadata` کے اندر حکمرانی کو توڑنے کے لئے ہیشس شامل ہیں تاکہ واقعات کو دوبارہ آڈٹ کریں۔

## 6. Norito پے لوڈ کی مثالیں

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
- ** SN-5 (ادائیگی اور تصفیہ): ** مالی تصفیے اور خودکار رپورٹوں کے لئے `RevenueShareRecordV1` کا استعمال۔

SNS روڈ میپ اپ ڈیٹس کے ساتھ سوالات یا تبدیلی کی درخواستوں کو `roadmap.md` میں پیش کیا جانا چاہئے اور انضمام کے بعد `status.md` میں ظاہر ہوتا ہے۔