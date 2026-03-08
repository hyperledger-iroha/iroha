---
lang: ur
direction: rtl
source: docs/portal/docs/sns/registry-schema.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sns/registry_schema.md` کا آئینہ دار ہے اور اب پورٹل کی کیننیکل کاپی کے طور پر کام کرتا ہے۔ ماخذ فائل ترجمہ کی تازہ کاریوں کے لئے باقی ہے۔
:::

# سورہ نام سروس رجسٹریشن اسکیم (SN-2A)

** حیثیت: ** لکھا ہوا 2026-03-24-ایس این ایس پروگرام کے ذریعہ جائزہ لینے کے لئے پیش کیا گیا  
** روڈ میپ لنک: ** SN-2A "رجسٹری اسکیما اور اسٹوریج لے آؤٹ"  
** دائرہ کار: ** سی او آر اے نام سروس (ایس این ایس) کے لئے خارج ہونے والے کیننیکل Norito ڈھانچے ، لائف سائیکل اسٹیٹس ، اور واقعات کی وضاحت کریں تاکہ رجسٹری اور رجسٹری کے نفاذ معاہدوں ، ایس ڈی کے اور گیٹ ویز میں عین مطابق ہوں۔

یہ دستاویز یہ وضاحت کرکے SN-2A کے لئے فراہم کرنے والے اسکیما کو مکمل کرتی ہے:

1. شناخت کنندگان اور ہیشنگ کے قواعد (`SuffixId` ، `NameHash` ، سلیکٹرز کا اخذ)۔
2. نام کی رجسٹریشن ، لاحقہ پالیسیاں ، قیمت کے درجات ، محصولات میں تقسیم ، اور رجسٹری کے واقعات کے لئے 2. ڈھانچے/enums Norito۔
3. اسٹوریج لے آؤٹ اور انڈیکس پریفکس ڈٹرمینسٹک ری پلے کے لئے۔
4. ایک ریاستی مشین جس میں رجسٹریشن ، تجدید ، فضل/چھٹکارا ، منجمد اور قبر کے پتھر شامل ہیں۔
5. DNS/گیٹ وے آٹومیشن کے ذریعہ استعمال ہونے والے کیننیکل واقعات۔

## 1۔ شناخت کنندہ اور ہیشنگ

| شناخت کنندہ | تفصیل | مشتق |
| ------------ | ------------- | -------------- |
| `SuffixId` (`u16`) | اعلی سطحی لاحقہ (`.sora` ، `.nexus` ، `.dao`) کے لئے ریکارڈ شناخت کنندہ۔ [`sns_suffix_governance_charter.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) میں لاحقہ کیٹلاگ کے ساتھ منسلک۔ | گورننس ووٹ کے ذریعہ تفویض ؛ `SuffixPolicyV1` میں محفوظ ہے۔ |
| `SuffixSelector` | لاحقہ کی کیننیکل اسٹرنگ فارم (ASCII ، لوئر کیس) | مثال: `.sora` -> `sora`۔ |
| `NameSelectorV1` | رجسٹرڈ لیبل کے لئے بائنری سلیکٹر۔ | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`۔ نورم V1 کے مطابق لیبل NFC + لوئر کیس ہے۔ |
| `NameHash` (`[u8;32]`) | معاہدوں ، واقعات اور کیچوں کے ذریعہ استعمال شدہ بنیادی تلاش کی کلید۔ | `blake3(NameSelectorV1_bytes)`۔ |

تعی .ن کی ضروریات:

- لیبلز کو نورم V1 (UTS-46 سخت ، STD3 ASCII ، NFC) کے ذریعے معمول بنایا جاتا ہے۔ ہیشنگ سے پہلے صارف کے ڈور کو معمول پر لانا ضروری ہے۔
- محفوظ لیبل (`SuffixPolicyV1.reserved_labels` سے) کبھی رجسٹری میں داخل نہ ہوں۔ گورننس صرف صرف ایورٹ ایونٹس کو اوور رائڈس `ReservedNameAssigned`۔

## 2. Norito ڈھانچے

### 2.1 namerecordv1| فیلڈ | قسم | نوٹ |
| ------- | ------ | ------- |
| `suffix_id` | `u16` | حوالہ `SuffixPolicyV1`۔ |
| `selector` | `NameSelectorV1` | آڈٹ/ڈیبگ کے لئے را سلیکٹر بائٹس۔ |
| `name_hash` | `[u8; 32]` | نقشہ جات/واقعات کی کلید۔ |
| `normalized_label` | `AsciiString` | انسانی پڑھنے کے قابل لیبل (پوسٹ نورم V1)۔ |
| `display_label` | `AsciiString` | اسٹیورڈ کے ذریعہ فراہم کردہ کیسنگ ؛ اختیاری کاسمیٹکس۔ |
| `owner` | `AccountId` | تجدیدات/منتقلی کو کنٹرول کرتا ہے۔ |
| `controllers` | `Vec<NameControllerV1>` | ہدف اکاؤنٹ کے پتے ، حل کرنے والے ، یا درخواست میٹا ڈیٹا کے حوالہ جات۔ |
| `status` | `NameStatus` | لائف سائیکل اشارے (سیکشن 4 دیکھیں)۔ |
| `pricing_class` | `u8` | لاحقہ قیمت کے درجے (معیاری ، پریمیم ، محفوظ) میں انڈیکس۔ |
| `registered_at` | `Timestamp` | ابتدائی ایکٹیویشن بلاک ٹائم اسٹیمپ۔ |
| `expires_at` | `Timestamp` | ادا شدہ مدت کا اختتام۔ |
| `grace_expires_at` | `Timestamp` | آٹو تجدید شدہ فضل کا اختتام (پہلے سے طے شدہ +30 دن) |
| `redemption_expires_at` | `Timestamp` | چھٹکارا ونڈو کا اختتام (پہلے سے طے شدہ +60 دن)۔ |
| `auction` | `Option<NameAuctionStateV1>` | جب ڈچ دوبارہ کھولنے یا فعال پریمیم نیلامی ہو تو موجود ہوں۔ |
| `last_tx_hash` | `Hash` | اس ورژن کو پیدا کرنے والے لین دین کی طرف اشارہ کرنے والا پوائنٹر۔ |
| `metadata` | `Metadata` | رجسٹرار کا صوابدیدی میٹا ڈیٹا (ٹیکسٹ ریکارڈز ، ثبوت)۔ |

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
| ------- | ------ | ------- |
| `suffix_id` | `u16` | بنیادی کلید ؛ پالیسی ورژن کے مابین مستحکم۔ |
| `suffix` | `AsciiString` | مثال کے طور پر ، `sora`۔ |
| `steward` | `AccountId` | گورننس چارٹر میں اسٹیورڈ کی تعریف کی گئی۔ |
| `status` | `SuffixStatus` | `Active` ، `Paused` ، `Revoked`۔ |
| `payment_asset_id` | `AsciiString` | طے شدہ اثاثہ شناخت کنندہ بطور ڈیفالٹ (مثال کے طور پر `xor#sora`)۔ |
| `pricing` | `Vec<PriceTierV1>` | درجے اور مدت کے قواعد کے ذریعہ قیمت کے گتانک۔ |
| `min_term_years` | `u8` | درجے کے اوور رائڈس سے قطع نظر خریدی گئی اصطلاح کے لئے فرش۔ |
| `grace_period_days` | `u16` | پہلے سے طے شدہ 30. |
| `redemption_period_days` | `u16` | پہلے سے طے شدہ 60. |
| `max_term_years` | `u8` | زیادہ سے زیادہ ابتدائی تجدید۔ |
| `referral_cap_bps` | `u16` | چارٹر کے مطابق <= 1000 (10 ٪)۔ |
| `reserved_labels` | `Vec<ReservedNameV1>` | اسائنمنٹ ہدایات کے ساتھ گورننس کے ذریعہ فراہم کردہ فہرست۔ |
| `fee_split` | `SuffixFeeSplitV1` | ٹریژری / اسٹیورڈ / ریفرل پارٹس (بنیاد پوائنٹس) |
| `fund_splitter_account` | `AccountId` | اکاؤنٹ جو یسکرو + کو برقرار رکھتا ہے فنڈز تقسیم کرتا ہے۔ |
| `policy_version` | `u16` | ہر تبدیلی کے ساتھ اضافہ۔ |
| `metadata` | `Metadata` | توسیعی نوٹ (KPI عہد ، تعمیل ڈاکٹر ہیشس)۔ |

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
| -------- | -------- | ----------- |
| `RevenueShareRecordV1` | `suffix_id` ، `epoch_id` ، `treasury_amount` ، `steward_amount` ، Norito ، `escrow_amount` ، `settled_at` ، Norito۔ | تصفیہ کی مدت (ہفتہ وار) کے ذریعہ ادائیگیوں کا تعی .ن ریکارڈ۔ |
| `RevenueAccrualEventV1` | `name_hash` ، `suffix_id` ، `event` ، `gross_amount` ، `net_amount` ، `referral_account`۔ | ہر بار ادائیگی پوسٹ کی جاتی ہے (رجسٹریشن ، تجدید ، نیلامی) |

تمام `TokenValue` فیلڈز Norito کے کیننیکل فکسڈ انکوڈنگ کا استعمال کرنسی کوڈ کے ساتھ استعمال کرتے ہیں جس سے متعلقہ `SuffixPolicyV1` میں اعلان کیا گیا ہے۔

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

واقعات کو ایک تولیدی لاگ (مثال کے طور پر ، ڈومین `RegistryEvents`) میں شامل کیا جانا چاہئے اور ڈی این ایس کیچوں کو ایس ایل اے کے اندر باطل کرنے کے لئے گیٹ وے فیڈ میں جھلکتا ہے۔

## 3. اسٹوریج لے آؤٹ اور انڈیکس

| کلید | تفصیل |
| ----- | ------------- |
| `Names::<name_hash>` | `name_hash` سے `NameRecordV1` سے بنیادی نقشہ۔ |
| `NamesByOwner::<AccountId, suffix_id>` | والیٹ UI (صارف دوست صفحہ بندی) کے لئے ثانوی انڈیکس۔ |
| `NamesByLabel::<suffix_id, normalized_label>` | تنازعات کا پتہ لگاتا ہے ، عصبی تلاش کو قابل بناتا ہے۔ |
| `SuffixPolicies::<suffix_id>` | آخری `SuffixPolicyV1`۔ |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` کی تاریخ۔ |
| `RegistryEvents::<u64>` | مونوٹون تسلسل کی کلید کے ساتھ صرف لاگ ان کریں۔ |

میزبانوں کے مابین اختیاری ہیشنگ کو برقرار رکھنے کے لئے Norito Tuples کا استعمال کرتے ہوئے تمام کلیدیں سیریلائز کرتی ہیں۔ انڈیکس کی تازہ کارییں بنیادی ریکارڈ کے ساتھ ساتھ ایٹمکی طور پر پائی جاتی ہیں۔

## 4۔ لائف سائیکل اسٹیٹ مشین

| حیثیت | اندراج کے حالات | منتقلی کی اجازت | نوٹ |
| ------- | ------------------------ | -------------------------- | ------- |
| دستیاب | جب `NameRecord` غائب ہو تو اخذ کیا گیا ہے۔ | `PendingAuction` (پریمیم) ، `Active` (معیاری رجسٹریشن)۔ | دستیابی کی تلاش صرف انڈیکس پڑھتی ہے۔ |
| زیر التواء | جب `PriceTierV1.auction_kind`! = کوئی نہیں۔ | `Active` (نیٹ نیلامی) ، `Tombstoned` (کوئی بولی نہیں)۔ | نیلامی کا مسئلہ `AuctionOpened` اور `AuctionSettled`۔ |
| فعال | کامیاب رجسٹریشن یا تجدید۔ | `GracePeriod` ، `Frozen` ، `Tombstoned`۔ | `expires_at` منتقلی کی رہنمائی کرتا ہے۔ |
| فضل پیریڈ | خودکار جب `now > expires_at`۔ | `Active` (تازہ ترین تجدید) ، `Redemption` ، `Tombstoned`۔ | پہلے سے طے شدہ +30 دن ؛ پھر بھی حل ہوتا ہے لیکن پرچم لگا ہوا ہے۔ |
| چھٹکارا | `now > grace_expires_at` لیکن `< redemption_expires_at`۔ | `Active` (دیر سے تجدید) ، `Tombstoned`۔ | احکامات میں جرمانے کی شرح کی ضرورت ہوتی ہے۔ |
| منجمد | گورننس یا سرپرست منجمد۔ | `Active` (علاج کے بعد) ، `Tombstoned`۔ | آپ کنٹرولرز کی منتقلی یا اپ ڈیٹ نہیں کرسکتے ہیں۔ |
| مقبرہ | رضاکارانہ استعفیٰ ، مستقل تنازعہ یا میعاد ختم ہونے والے چھٹکارے کا نتیجہ۔ | `PendingAuction` (ڈچ دوبارہ کھلنے) یا ٹامبسٹونڈ رہتا ہے۔ | واقعہ `NameTombstoned` میں وجہ شامل ہونا ضروری ہے۔ |مربوط بہاو کیچوں کو برقرار رکھنے کے لئے ریاستی منتقلی کو متعلقہ `RegistryEventKind` جاری کرنا ہوگا۔ ڈچ دوبارہ کھولنے والی نیلامی میں داخل ہونے والے مقبرہ نام `AuctionKind::DutchReopen` پے لوڈ سے منسلک ہوتے ہیں۔

## 5. کیننیکل واقعات اور گیٹ وے کی مطابقت پذیری

گیٹ ویز `RegistryEventV1` کو سبسکرائب کریں اور DNS/SoraFS کو ہم آہنگ کریں:

1. آخری `NameRecordV1` کے لئے تلاش کریں جو واقعات کی ترتیب سے حوالہ دیتے ہیں۔
2. دوبارہ حل کرنے والے ریزولور ٹیمپلیٹس (ترجیحی IH58 ایڈریس + کمپریسڈ (`sora`) دوسرے آپشن کے طور پر ، ٹیکسٹ ریکارڈز)۔
3. [`soradns_registry_rfc.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) میں بیان کردہ Soradns فلو کے ذریعے زون کا ڈیٹا PIN کریں۔

واقعہ کی فراہمی کی ضمانتیں:

- ہر ٹرانزیکشن جو `NameRecordV1` * کو متاثر کرتا ہے * کو * بالکل ایک واقعہ سے منسلک کرنا چاہئے جس میں `version` میں سختی سے اضافہ ہوتا ہے۔
- واقعات `RevenueSharePosted` حوالہ بستیوں `RevenueShareRecordV1` کے ذریعہ جاری کردہ۔
- منجمد/غیر منقولہ/مقبرہ واقعات میں آڈٹ ری پلے کے لئے `metadata` کے اندر گورننس نمونے کی ہیش شامل ہیں۔

## 6. پے لوڈ کی مثالیں Norito

### 6.1 نمریکورڈ مثال

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

### 6.2 لاحقہ مثال

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

۔
۔
- ** SN-5 (ادائیگی اور تصفیہ): ** مالی مفاہمت اور رپورٹنگ آٹومیشن کے لئے `RevenueShareRecordV1`۔

سوالات یا تبدیلی کی درخواستوں کو `roadmap.md` میں SNS روڈ میپ اپڈیٹس کے ساتھ لاگ ان کیا جانا چاہئے اور جب مربوط ہونے پر `status.md` میں ظاہر ہوتا ہے۔