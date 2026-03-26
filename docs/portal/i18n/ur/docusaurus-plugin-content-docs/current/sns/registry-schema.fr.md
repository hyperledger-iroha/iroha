---
lang: ur
direction: rtl
source: docs/portal/docs/sns/registry-schema.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sns/registry_schema.md` کی عکاسی کرتا ہے اور اب پورٹل کی کیننیکل کاپی کے طور پر کام کرتا ہے۔ ماخذ فائل ترجمہ کی تازہ کاریوں کے لئے باقی ہے۔
:::

# سورہ نام سروس رجسٹری اسکیما (SN-2A)

** حیثیت: ** لکھا ہوا 2026-03-24-ایس این ایس پروگرام کے جائزے میں پیش کیا گیا  
** روڈ میپ لنک: ** SN-2A "رجسٹری اسکیما اور اسٹوریج لے آؤٹ"  
** دائرہ کار: ** کیننیکل Norito ڈھانچے ، لائف سائیکل اسٹیٹس اور سورس نام سروس (ایس این ایس) کے لئے خارج ہونے والے واقعات کی وضاحت کریں تاکہ رجسٹری اور رجسٹرار کے نفاذ معاہدوں ، ایس ڈی کے اور گیٹ ویز میں عین مطابق رہیں۔

یہ دستاویز یہ وضاحت کرکے SN-2A کے لئے اسکیمیٹک ڈلیوری قابل مکمل کرتی ہے:

1. شناخت کنندگان اور ہیشنگ کے قواعد (`SuffixId` ، `NameHash` ، سلیکٹرز کا اخذ)۔
2. نام ریکارڈز ، لاحقہ پالیسیاں ، قیمتوں کا تعین کرنے والے درجے ، محصولات کی تقسیم اور رجسٹری کے واقعات کے لئے 2. ڈھانچے/enums Norito۔
3. اسٹوریج لے آؤٹ اور انڈیکس پریفکس ڈٹرمینسٹک ری پلے کے لئے۔
4. ایک ریاستی مشین جس میں رجسٹریشن ، تجدید ، فضل/چھٹکارا ، منجمد اور قبر کے پتھر شامل ہیں۔
5. DNS/گیٹ وے آٹومیشن کے ذریعہ استعمال ہونے والے کیننیکل واقعات۔

## 1۔ شناخت کنندہ اور ہیشنگ

| شناخت کنندہ | تفصیل | مشتق |
| ------------ | -------- | ------------ |
| `SuffixId` (`u16`) | اعلی سطحی لاحقہ (`.sora` ، `.nexus` ، `.dao`) کے لئے شناخت کنندہ کو رجسٹر کریں۔ [`sns_suffix_governance_charter.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) میں لاحقہ کیٹلاگ کے ساتھ سیدھ میں ہے۔ | گورننس ووٹ کے ذریعہ تفویض ؛ `SuffixPolicyV1` میں اسٹورز۔ |
| `SuffixSelector` | لاحقہ کی کیننیکل اسٹرنگ فارم (ASCII ، لوئر کیس) | مثال: `.sora` -> `sora`۔ |
| `NameSelectorV1` | ریکارڈ شدہ لیبل کے لئے بائنری سلیکٹر۔ | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`۔ نورم V1 کے مطابق لیبل NFC + لوئر کیس ہے۔ |
| `NameHash` (`[u8;32]`) | معاہدوں ، واقعات اور کیچوں کے ذریعہ استعمال شدہ بنیادی تلاش کی کلید۔ | `blake3(NameSelectorV1_bytes)`۔ |

تعی .ن کی ضروریات:

- لیبلز کو نورم V1 (UTS-46 سخت ، STD3 ASCII ، NFC) کے ذریعے معیاری بنایا جاتا ہے۔ ہیشنگ سے پہلے صارف کے ڈور کو معمول پر لانا ضروری ہے۔
- محفوظ لیبل (`SuffixPolicyV1.reserved_labels` سے) کبھی رجسٹری میں داخل نہ ہوں۔ گورننس صرف اوور رائڈس آئٹ `ReservedNameAssigned` واقعات۔

## 2. ڈھانچے Norito

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
| `pricing_class` | `u8` | لاحقہ (معیاری ، پریمیم ، محفوظ) کے قیمت کے درجے میں انڈیکس۔ |
| `registered_at` | `Timestamp` | ابتدائی ایکٹیویشن کے ٹائم اسٹیمپ کو بلاک کریں۔ |
| `expires_at` | `Timestamp` | تنخواہ کی مدت کا اختتام۔ |
| `grace_expires_at` | `Timestamp` | آٹو تجدید شدہ فضل کا اختتام (پہلے سے طے شدہ +30 دن) |
| `redemption_expires_at` | `Timestamp` | چھٹکارا ونڈو کا اختتام (پہلے سے طے شدہ +60 دن)۔ |
| `auction` | `Option<NameAuctionStateV1>` | جب ڈچ دوبارہ کھولیں یا پریمیم نیلامی فعال ہوں۔ |
| `last_tx_hash` | `Hash` | اس ورژن کو تیار کرنے والے لین دین کی طرف اشارہ کرنے والا پوائنٹر۔ |
| `metadata` | `Metadata` | صوابدیدی رجسٹرار میٹا ڈیٹا (متن کے ریکارڈ ، ثبوت) |

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
| `payment_asset_id` | `AsciiString` | پہلے سے طے شدہ تصفیہ اثاثہ شناخت کنندہ (جیسے `61CtjvNd9T3THAR65GsMVHr82Bjc`)۔ |
| `pricing` | `Vec<PriceTierV1>` | تیسری اور مدت کے قواعد کے ذریعہ قیمت کے اعدادوشمار۔ |
| `min_term_years` | `u8` | درجے کے اوور رائڈ سے قطع نظر خریدی گئی اصطلاح کے لئے فرش۔ |
| `grace_period_days` | `u16` | پہلے سے طے شدہ 30. |
| `redemption_period_days` | `u16` | پہلے سے طے شدہ 60. |
| `max_term_years` | `u8` | زیادہ سے زیادہ تجدید متوقع۔ |
| `referral_cap_bps` | `u16` | <= 1000 (10 ٪) چارٹر پر منحصر ہے۔ |
| `reserved_labels` | `Vec<ReservedNameV1>` | اسائنمنٹ ہدایات کے ساتھ گورننس کے ذریعہ فراہم کردہ فہرست۔ |
| `fee_split` | `SuffixFeeSplitV1` | ٹریژری / اسٹیورڈ / ریفرل (بیس پوائنٹس) کے حصص۔ |
| `fund_splitter_account` | `AccountId` | اکاؤنٹ جو ایسکرو + رکھتا ہے + فنڈز تقسیم کرتا ہے۔ |
| `policy_version` | `u16` | ہر تبدیلی کے ساتھ اضافہ۔ |
| `metadata` | `Metadata` | توسیعی نوٹ (عہد کے پی آئی ، تعمیل ڈاکٹر ہیشس)۔ |

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
| -------- | -------- | --------- |
| `RevenueShareRecordV1` | `suffix_id` ، `epoch_id` ، `treasury_amount` ، `steward_amount` ، Norito ، `escrow_amount` ، `settled_at` ، Norito۔ | تصفیہ کی مدت (ہفتہ وار) کے ذریعہ روٹ کی ادائیگیوں کا تعی .ن ریکارڈنگ۔ |
| `RevenueAccrualEventV1` | `name_hash` ، `suffix_id` ، `event` ، `gross_amount` ، `net_amount` ، `referral_account`۔ | ہر پوسٹ ادائیگی (رجسٹریشن ، تجدید ، نیلامی) پر جاری کیا گیا۔ |

تمام `TokenValue` فیلڈز Norito کے کیننیکل فکسڈ انکوڈنگ کا استعمال کرنسی کوڈ کے ساتھ استعمال کرتے ہیں جس سے متعلقہ `SuffixPolicyV1` میں اعلان کیا گیا ہے۔

### 2.4 واقعات رجسٹر کریں

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

واقعات کو دوبارہ چلانے کے قابل لاگ (مثال کے طور پر ، ڈومین `RegistryEvents`) میں شامل کیا جانا چاہئے اور DNS کیچوں کے لئے گیٹ وے فیڈز کی عکاسی کریں تاکہ SLAs میں باطل ہو۔

## 3. اسٹوریج لے آؤٹ اور انڈیکس

| کلید | تفصیل |
| ----- | ------------- |
| `Names::<name_hash>` | `name_hash` سے `NameRecordV1` سے بنیادی نقشہ۔ |
| `NamesByOwner::<AccountId, suffix_id>` | UI پرس (صفحہ بندی دوستانہ) کے لئے ثانوی انڈیکس۔ |
| `NamesByLabel::<suffix_id, normalized_label>` | تنازعات کا پتہ لگاتا ہے ، عصبی تحقیق کو کھانا کھلاتا ہے۔ |
| `SuffixPolicies::<suffix_id>` | تازہ ترین `SuffixPolicyV1`۔ |
| `RevenueShare::<suffix_id, epoch_id>` | تاریخ `RevenueShareRecordV1`۔ |
| `RegistryEvents::<u64>` | لاگ ان ضمیمہ صرف کلیدی فی ایک نیرٹونک تسلسل۔ |

میزبانوں کے مابین اختیاری ہیشنگ کو برقرار رکھنے کے لئے تمام چابیاں Norito Tuples کے ذریعے سیریلائز کی جاتی ہیں۔ انڈیکس کی تازہ کارییں مرکزی ریکارڈ کے ساتھ جوہری طور پر کی جاتی ہیں۔

## 4۔ لائف سائیکل اسٹیٹ مشین

| ریاست | اندراج کی ضروریات | منتقلی کی اجازت | نوٹ |
| ------- | ---------- | ------------ | ------- |
| دستیاب | جب `NameRecord` غیر حاضر ہے تو بہتا ہے۔ | `PendingAuction` (پریمیم) ، `Active` (معیاری رجسٹریشن)۔ | دستیابی کی تلاش صرف انڈیکس پڑھتی ہے۔ |
| زیر التواء | جب `PriceTierV1.auction_kind`! = کوئی نہیں۔ | `Active` (طے شدہ بولی) ، `Tombstoned` (کوئی بولی نہیں)۔ | نیلامی کا مسئلہ `AuctionOpened` اور `AuctionSettled`۔ |
| فعال | رجسٹریشن یا تجدید کامیاب۔ | `GracePeriod` ، `Frozen` ، `Tombstoned`۔ | `expires_at` منتقلی کو کنٹرول کرتا ہے۔ |
| فضل پیریڈ | خودکار جب `now > expires_at`۔ | `Active` (وقت پر تجدید) ، `Redemption` ، `Tombstoned`۔ | پہلے سے طے شدہ +30 دن ؛ حل لیکن نشان۔ |
| چھٹکارا | `now > grace_expires_at` لیکن `< redemption_expires_at`۔ | `Active` (دیر سے تجدید) ، `Tombstoned`۔ | احکامات میں جرمانے کی فیس کی ضرورت ہوتی ہے۔ |
| منجمد | گورننس یا سرپرست منجمد کریں۔ | `Active` (علاج کے بعد) ، `Tombstoned`۔ | کنٹرولرز کی منتقلی یا اپ ڈیٹ نہیں کرسکتے ہیں۔ |
| مقبرہ | رضاکارانہ طور پر ترک کرنا ، مستقل قانونی چارہ جوئی کا نتیجہ ، یا میعاد ختم ہونے والی چھٹکارا۔ | `PendingAuction` (ڈچ دوبارہ کھلنے) یا ٹامبسٹونڈ رہتا ہے۔ | واقعہ `NameTombstoned` میں ایک وجہ شامل ہونی چاہئے۔ |ریاستی منتقلی کو لازمی طور پر برقرار رہنے کے لئے بہاو کیچوں کے لئے اسی طرح کے `RegistryEventKind` کا اخراج کرنا ہوگا۔ ڈچ دوبارہ کھولنے والی نیلامی میں داخل ہونے والے مقبرہ ناموں سے ایک پے لوڈ `AuctionKind::DutchReopen` منسلک ہوتا ہے۔

## 5. کیننیکل واقعات اور مطابقت پذیری گیٹ وے

گیٹ وے `RegistryEventV1` کو سبسکرائب کریں اور DNS/SoraFS کے ذریعے ہم آہنگی کریں:

1. واقعات کی ترتیب سے آخری `NameRecordV1` حوالہ بازیافت کریں۔
2. حل کرنے والے ٹیمپلیٹس کو دوبارہ تخلیق کریں (I105 ایڈریس ترجیحی + کمپریسڈ (`sora`) دوسری پسند ، متن کے ریکارڈ)۔
3. [`soradns_registry_rfc.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) میں بیان کردہ Soradns ورک فلو کے ذریعے تازہ ترین زون کے اعداد و شمار کو پن کریں۔

واقعہ کی فراہمی کی ضمانتیں:

- ہر ٹرانزیکشن جو `NameRecordV1` * کو متاثر کرتا ہے * لازمی طور پر `version` میں سختی سے اضافہ کے ساتھ بالکل ایک واقعہ شامل کرنا چاہئے۔
- واقعات `RevenueSharePosted` `RevenueShareRecordV1` کے ذریعہ جاری کردہ بستیوں کا حوالہ دیتے ہیں۔
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

### 6.2 لاحقہ مثال

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

## 7. اگلے اقدامات

۔
۔
- ** SN-5 (ادائیگی اور تصفیہ): ** مالی مفاہمت اور رپورٹنگ آٹومیشن کے لئے `RevenueShareRecordV1`۔

سوالات یا تبدیلی کی درخواستیں `roadmap.md` میں SNS روڈ میپ کی تازہ کاریوں کے ساتھ دائر کی جانی چاہئیں اور انضمام کے بعد `status.md` میں ظاہر ہوں۔