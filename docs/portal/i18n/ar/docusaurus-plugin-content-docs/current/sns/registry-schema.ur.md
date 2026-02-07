---
lang: ar
direction: rtl
source: docs/portal/docs/sns/registry-schema.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة مستند ماخذ
هذه الصفحة `docs/source/sns/registry_schema.md` هي عبارة عن بطاقة بريدية ولوحة مفاتيح إلكترونية تعمل على أكمل وجه. لقد تم ترجمة سورس فويل إلى هذا القرار.
:::

# خدمة اسم سورا رجسٹری اسکیمہ (SN-2a)

**الحدث:** مسود 24-03-2026 -- برنامج SNS ريويو ليوم جمعة  
**نظام التشغيل:** SN-2a "مخطط التسجيل وتخطيط التخزين"  
**الصفحة:** خدمة اسم Sora (SNS) التي تدعم Norito، لمواقع الويب وإخراج الأيونات الممتعة هي ميزة التسجيل والمسجل لتطبيقات الشبكة وSDKs والبوابات الحتمية رہیں۔

يحتوي هذا البرنامج على SN-2a الذي يحتوي على مجموعة متنوعة من البطاقات المكملة، وهو عبارة عن غلاف يتضمن ما يلي:

1. قواعد التجزئة والتجزئة (`SuffixId`، `NameHash`، اشتقاق المحدد).
2. اسم التسجيل، اللاحقة باليسيس، الطبقات السعرية، ريونيو سبلت وتسجيل الأيونات إلى Norito الهياكل/التعدادات.
3. إعادة التشغيل الحتمية لتخطيط التخزين وبادئات الفهرس.
4. رجسٹریشن، تجدید، النعمة/الفداء، التجميد وشاهدة القبر پر تتضمن آلة الدولة۔
5. أتمتة DNS/البوابة للأحداث الأساسية.

## 1.شناختیں والتجزئة| شناخت | وضاحت | أخذ |
|------------|-------------|------------|
| `SuffixId` (`u16`) | ٹاپ لیول اللواحق (`.sora`, `.nexus`, `.dao`) کے لیے رجسٹری شناخت. [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) كتالوج لاحقة يتوافق. | تصويت الحكم سے متخصص; `SuffixPolicyV1` محفوظ. |
| `SuffixSelector` | لاحقة کی شكل سلسلة متعارف عليها (ASCII، أحرف صغيرة)۔ | مثال: `.sora` -> `sora`. |
| `NameSelectorV1` | تم تسجيل اسم محدد لبنك الإنترنت. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. لبل Norm v1 يتوافق مع NFC + الأحرف الصغيرة. |
| `NameHash` (`[u8;32]`) | تحتوي الشبكة والأيونات وذاكرة التخزين المؤقت على مفتاح البحث. | `blake3(NameSelectorV1_bytes)`. |

الحتمية ضرورة:

- لبلز Norm v1 (UTS-46 الصارم، STD3 ASCII، NFC) تم تطبيعه. استخدم سلاسل تجزئة لتطبيع السلاسل المطلوبة.
- التسميات المحجوزة (`SuffixPolicyV1.reserved_labels`) لا توجد في السجل؛ تتجاوز الحوكمة فقط `ReservedNameAssigned` خارج البطاقة.

## 2.Norito اختيں

### 2.1 NameRecordV1| فيلڈ | قسم | أخبار |
|-------|------|-------|
| `suffix_id` | `u16` | `SuffixPolicyV1` ريفرنس۔ |
| `selector` | `NameSelectorV1` | التدقيق/التصحيح هو بايت المحدد الأولي. |
| `name_hash` | `[u8; 32]` | الخرائط/الأحداث کے لیے مفتاح. |
| `normalized_label` | `AsciiString` | إنسانية قابلة للتحمل (القاعدة الإصدار 1 بعد ذلك). |
| `display_label` | `AsciiString` | ستيوارد کی الغلاف؛ اختیاری مستحضرات التجميل۔ |
| `owner` | `AccountId` | التجديدات/النقل کرتا ہے. |
| `controllers` | `Vec<NameControllerV1>` | إحصاءات أو محللات أو بيانات وصفية لشبكة الإنترنت. |
| `status` | `NameStatus` | لطائرات السايكل (الجزء 4 دقايق). |
| `pricing_class` | `u8` | لاحقة کے مستويات الأسعار کا مؤشر (قياسي، مميز، محجوز)۔ |
| `registered_at` | `Timestamp` | ابتدائی التنشيط كا بلاک ٹدائما. |
| `expires_at` | `Timestamp` | لقد حدث مدت کا اختتام۔ |
| `grace_expires_at` | `Timestamp` | نعمة التجديد التلقائي کا اختتام (افتراضي +30 دن)۔ |
| `redemption_expires_at` | `Timestamp` | نافذة الاسترداد کا اختتام (الافتراضي +60 دن)۔ |
| `auction` | `Option<NameAuctionStateV1>` | إعادة فتح الهولندية أو المزادات المتميزة التي من الممكن أن تكون متاحة. |
| `last_tx_hash` | `Hash` | لقد أصبحت الثورة الفرنسية بمثابة مؤشر حتمي. |
| `metadata` | `Metadata` | المسجل کی البيانات الوصفية بشكل تعسفي (السجلات النصية، البراهين)۔ |

بنيات معاون:

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

### 2.2 SuffixPolicyV1| فيلڈ | قسم | أخبار |
|-------|------|-------|
| `suffix_id` | `u16` | مفتاح بنياديّ؛ باليس ويرنز مطمئنة. |
| `suffix` | `AsciiString` | مثال على ذلك `sora`. |
| `steward` | `AccountId` | ميثاق الحوكمة مفيد . |
| `status` | `SuffixStatus` | `Active`، `Paused`، `Revoked`. |
| `payment_asset_id` | `AsciiString` | معرف أصول التسوية الافتراضية (مثلا `xor#sora`)۔ |
| `pricing` | `Vec<PriceTierV1>` | أسعار ومستويات ومعاملات وقواعد متعددة. |
| `min_term_years` | `u8` | الشراء يستمر حتى النهاية. |
| `grace_period_days` | `u16` | الافتراضي 30. |
| `redemption_period_days` | `u16` | الافتراضي 60. |
| `max_term_years` | `u8` | تجد المزيد من المعلومات في وقت لاحق. |
| `referral_cap_bps` | `u16` | <=1000 (10%) ميثاق حسب. |
| `reserved_labels` | `Vec<ReservedNameV1>` | الحكم هو نظام قائم على التعيينات. |
| `fee_split` | `SuffixFeeSplitV1` | الخزينة / الوكيل / الإحالة حصص (نقاط الأساس)۔ |
| `fund_splitter_account` | `AccountId` | رهن الضمان والقسائم المالية والمدفوعات. |
| `policy_version` | `u16` | لقد تم تغييره بالفعل. |
| `metadata` | `Metadata` | توسیعی نوتٹس (ميثاق مؤشرات الأداء الرئيسية، تجزئات مستند الامتثال)۔ |

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

### 2.3 ريونيو وريكارز التسوية| هيكل | فيليز | قصدي |
|--------|-------|-----|
| `RevenueShareRecordV1` | `suffix_id`، `epoch_id`، `treasury_amount`، `steward_amount`، `referral_amount`، `escrow_amount`، `settled_at`، `tx_hash`. | عصر الاستيطان (الحرب) تم توجيهه إلى ریکارڈ. |
| `RevenueAccrualEventV1` | `name_hash`، `suffix_id`، `event`، `gross_amount`، `net_amount`، `referral_account`. | ہر ادائیگی پوسٹ ہونے پر انبعاث (تسجيل، تجديد، مزاد)۔ |

يتم الإعلان عن تمام `TokenValue` فيلنج Norito وهو استخدام ترميز النقاط الثابتة الأساسي والعلاقة الأساسية `SuffixPolicyV1`.

### 2.4 رجسٹری ايونٹس

الأحداث الأساسية أتمتة وتحليلات DNS/البوابة لإعادة تشغيل السجل.

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

يمكن للسجل القابل لإعادة التشغيل (مثل مجال `RegistryEvents`) إلحاق العناصر الضرورية وموجزات البوابة التي تعكس العناصر الضرورية وذاكرة التخزين المؤقت لنظام أسماء النطاقات (SLA) التي يبطلها اندر.

## 3. تخطيط التخزين والفهارس

| مفتاح | وضاحت |
|-----|------------|
| `Names::<name_hash>` | `name_hash` سے `NameRecordV1` خريطة بنياد. |
| `NamesByOwner::<AccountId, suffix_id>` | واجهة مستخدم المحفظة کے لیے ثانوي فهرس (صفحات ودية)۔ |
| `NamesByLabel::<suffix_id, normalized_label>` | كشف الصراعات کرتا ہے والبحث الحتمي فعال بناتنا ہے۔ |
| `SuffixPolicies::<suffix_id>` | أحدث `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` ہستري. |
| `RegistryEvents::<u64>` | سجل الإلحاق فقط هو تسلسل المفاتيح الرتيب. |تمام المفاتيح Norito الصفوف التي تقوم بالتسلسل وتضيف المضيفين إلى تجزئة حتمية. يتم تحديث فهرس التحديثات بشكل تلقائي.

## 4. آلة حالة دورة الحياة

| الدولة | شروط الدخول | التحولات المسموح بها | ملاحظات |
|-------|------------------|--------------------|-------|
| متاح | ج `NameRecord` غير موجود. | `PendingAuction` (ممتاز)، `Active` (تسجيل قياسي). | البحث عن توافر الفهارس فقط |
| في انتظار المزاد | ج `PriceTierV1.auction_kind` != لا يوجد ہو۔ | `Active` (تسوية المزاد)، `Tombstoned` (بدون عطاءات). | المزادات `AuctionOpened` و`AuctionSettled` تنبعث منها كرات طينية. |
| نشط | رجسٹریشن أو تجدید كامیاب ہو۔ | `GracePeriod`، `Frozen`، `Tombstoned`. | `expires_at` الانتقال چلاتا ہے۔ |
| فترة النعمة | ج `now > expires_at` ہو۔ | `Active` (التجديد في الوقت المحدد)، `Redemption`، `Tombstoned`. | الافتراضي +30 دن؛ حل ہوتا ہے مگر علم ہوتا ہے۔ |
| فداء | `now > grace_expires_at` ليكن `< redemption_expires_at`. | `Active` (التجديد المتأخر)، `Tombstoned`. | يتم فرض رسوم على عقوبة الجزاء. |
| مجمدة | الحكم أو الوصي تجميد ۔ | `Active` (المعالجة التالية)، `Tombstoned`. | لا يوجد أي تحديث للنقل أو وحدات التحكم. |
| شاهد القبر | رضاكارانہ استسلام، نزاع متكامل نتیجہ، أو ختم الفداء. | `PendingAuction` (إعادة فتح الهولندية) أو علامة مميزة رہتا ہے۔ | `NameTombstoned` يشمل المزيد. |يجب أن تكون انتقالات الحالة `RegistryEventKind` تنبعث من ذاكرة التخزين المؤقت النهائية. تم العثور على علامة Tombstoned التي تحمل اسم الهولندية لإعادة فتح المزادات داخل الحمولة وحمولة `AuctionKind::DutchReopen`.

## 5. الأحداث الأساسية ومزامنة البوابة

تقوم البوابات `RegistryEventV1` بمزامنة البيانات الرئيسية وDNS/SoraFS:

1. تم تحويل التسلسل الإلكتروني بشكل أفضل إلى `NameRecordV1`.
2. قوالب المحلل المزدوجة (IH58 ترجيح + مضغوط (`sora`) ثاني أفضل العناوين والسجلات النصية).
3. [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) يدعم سير عمل SoraDNS وهو دبوس بيانات المنطقة.

ضمانات تسليم الحدث:

- لقد تم إنشاء نسخة احتياطية من `NameRecordV1` *لازم* لـ `version` والتي ظلت تحتوي على بطاقة قوية للغاية.
- `RevenueSharePosted` `RevenueShareRecordV1` المستوطنات المنبعثة كمرجع.
- تجميد/إلغاء التجميد/علامة مميزة لإعادة تشغيل التدقيق في `metadata` تتضمن تجزئات الحوكمة الفنية التي تتضمن البطاقة.

## 6. مثال على الحمولات Norito

### 6.1 مثال على سجل الاسم

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

### 6.2 مثال على سياسة اللاحقة

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

## 7.اگلے اجراءات- **SN-2b (واجهة برمجة تطبيقات المسجل وخطافات الإدارة):** تعمل بنيات Torii على كشف الملفات (ارتباطات Norito وJSON) وفحوصات القبول لعناصر الإدارة.
- **SN-3 (محرك المزاد والتسجيل):** الالتزام/الكشف ومنطق إعادة الفتح الهولندي لـ `NameAuctionStateV1` يستخدم مرة أخرى.
- **SN-5 (الدفع والتسوية):** استخدام التسوية المالية وأتمتة التقارير لـ `RevenueShareRecordV1`.

تتضمن المكالمات والتحويلات `roadmap.md` SNS أكثر من مجرد درجة درج ودمج منذ `status.md`.