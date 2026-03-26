---
lang: ar
direction: rtl
source: docs/portal/docs/sns/registry-schema.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة المصدر القياسي
احترام `docs/source/sns/registry_schema.md` المعتمد الان كنسخة البوابة القياسية. يبقى ملف المصدر لتحديثات الترجمة.
:::

# مخطط سجل Sora Name Service (SN-2a)

**الحالة:** مسودة 2026-03-24 -- مقدمة لمراجعة برنامج SNS  
**رابط خريطة الطريق:** SN-2a "مخطط التسجيل وتخطيط التخزين"  
**النطاق:** تعريف هياكل Norito القياسية وحالات دورة الحياة والأحداث المنبعثة لخدمة Sora Name Service (SNS) دائمًا ان تنفيذات السجل والـ المسجل تستمر حتمية عبر العقود و SDKs و gates.

يكمل هذا الإيمان بالثقة لـ SN-2a عبر التحديد:

1. المعرفات وقواعد التجزئة (`SuffixId`, `NameHash`, اشتقاق محدد).
2. Norito الهياكل/التعدادات لسجلات الاسماء، السياسات اللاحقة، مستويات التسعير، توزيعات الايراد، وأحداث السجل.
3. تخطيط التخزين وبوائئ الفهارس واعادتها حتمي.
4. حالات تغطية التسجيل، النعمة/الفداء، التجميد و شاهد القبر.
5. الاحداث القياسية التي تستهلكها DNS/gateway.

## 1. المعرفات و التجزئة| المعرف | الوصف | الاشتقاق |
|------------|-------------|------------|
| `SuffixId` (`u16`) | معرف السجل للسوفكسات العليا (`.sora`, `.nexus`, `.dao`). متوافق مع كتالوج السوفكس في [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | يعين بتصويت الغزو; يخزن في `SuffixPolicyV1`. |
| `SuffixSelector` | الشكل النصي القياسي للسوفكس (ASCII، أحرف صغيرة). | مثال: `.sora` -> `sora`. |
| `NameSelectorV1` | محدد ثنائي الوسم الموزع. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. الوسم NFC + أحرف صغيرة حسب Norm v1. |
| `NameHash` (`[u8;32]`) | مفتاح البحث الأساسي المستخدم في والعقود حداث والكاشات. | `blake3(NameSelectorV1_bytes)`. |

المتطلبات الحتمية:

- يتم تطبيع الوسوم عبر Norm v1 (UTS-46 الصارم، STD3 ASCII، NFC). يجب تطبيع سلاسل المستخدم قبل التجزئة.
- الوسوم المحجوزة (من `SuffixPolicyV1.reserved_labels`) لا تسجل أبدا; التجاوزات الخاصة بالتحكم فقط في احداث `ReservedNameAssigned`.

## 2. الهياكل Norito

### 2.1 NameRecordV1| الحقل | النوع | مذكرة |
|-------|------|-----------|
| `suffix_id` | `u16` | مرجع `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | بايتات محدد برايد للتدقيق/debug. |
| `name_hash` | `[u8; 32]` | مفتاح للخرائط/الاحداث. |
| `normalized_label` | `AsciiString` | إشارة قابلة للقراءة (بعد Norm v1). |
| `display_label` | `AsciiString` | لاحرف ارسله ستيوارد; مستحضرات التجميل. |
| `owner` | `AccountId` | يتحكم في التعديلات/التحويلات. |
| `controllers` | `Vec<NameControllerV1>` | المراجع لاناوين الحساب او المحللين او البيانات الوصفية. |
| `status` | `NameStatus` | حالة دورة الحياة (انظر القسم 4). |
| `pricing_class` | `u8` | فهرس مستويات التسعير للسوفكس (قياسي، مميز، محجوز). |
| `registered_at` | `Timestamp` | وقت تفعيل التسجيل الاول. |
| `expires_at` | `Timestamp` | نهاية المدة ل. |
| `grace_expires_at` | `Timestamp` | نهاية Grace للتجديد التلقائي (افتراضي +30 يومًا). |
| `redemption_expires_at` | `Timestamp` | نهاية فداء النافذة (افتراضي +60 يومًا). |
| `auction` | `Option<NameAuctionStateV1>` | يظهر عند فتح الهولندية او المزادات المميزة العضوية. |
| `last_tx_hash` | `Hash` | حتمي على القياس الذي انتجت هذه النسخة. |
| `metadata` | `Metadata` | البيانات الوصفية اختيارية للمسجل (السجلات النصية، البراهين). |

هياكل مساندة:

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

### 2.2 SuffixPolicyV1| الحقل | النوع | مذكرة |
|-------|------|-----------|
| `suffix_id` | `u16` | المفتاح الأساسي الثابت عبر نسخة السياسة. |
| `suffix` | `AsciiString` | مثلا `sora`. |
| `steward` | `AccountId` | ستيوارد معرف في ميثاق ال تور. |
| `status` | `SuffixStatus` | `Active`، `Paused`، `Revoked`. |
| `payment_asset_id` | `AsciiString` | معرف اصل التسوية الافتراضية (مثلا `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | اتفاقيات التسعير حسب المستويات والقواعد المدة. |
| `min_term_years` | `u8` | حد ادنى للشراء بغض النظر عن التجاوزات. |
| `grace_period_days` | `u16` | الافتراضي 30. |
| `redemption_period_days` | `u16` | الافتراضي 60. |
| `max_term_years` | `u8` | الحد الاقصى للتجديد المسبق. |
| `referral_cap_bps` | `u16` | <=1000 (10%) حسب الميثاق. |
| `reserved_labels` | `Vec<ReservedNameV1>` | قائمة من التورم مع تعليمات التخصيص. |
| `fee_split` | `SuffixFeeSplitV1` | حصص الخزينة / الوكيل / الإحالة (نقاط الأساس). |
| `fund_splitter_account` | `AccountId` | حساب يمسك الضمان ويقسم الاموال. |
| `policy_version` | `u16` | يزيد مع كل تغيير. |
| `metadata` | `Metadata` | ملاحظات موسعة (ميثاق مؤشرات الأداء الرئيسية، التجزئات للامتثال للوثائق). |

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

### 2.3 سجلات الطيران والتسوية| هيكل | بمعنى | اللحوم |
|--------|--------|-------|
| `RevenueShareRecordV1` | `suffix_id`، `epoch_id`، `treasury_amount`، `steward_amount`، `referral_amount`، `escrow_amount`، `settled_at`، `tx_hash`. | سجل حتمي لمدفوعات موزعة حسب حقبة التسوية (اسبوعيا). |
| `RevenueAccrualEventV1` | `name_hash`، `suffix_id`، `event`، `gross_amount`، `net_amount`، `referral_account`. | يصدر عند تسجيل كل دفعة (تسجيل، تجديد، مزاد). |

شامل `TokenValue` يستخدم الترميز الثابت القياسي لنوريتو مع رمز العملة البائع في `SuffixPolicyV1` غير.

### 2.4 احدث السجل

الاحداث القياسية متوفرة replay log DNS/gateway والقياس.

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

يجب إضافة الاحداث الى سجل قابل للاعادة (مثل نطاق `RegistryEvents`) وعكسها الى الخلاصات الخاصة بالبوابة حتى انتهاء صلاحية ذاكرة التخزين المؤقت DNS ضمن SLA.

## 3. تخطيط التخطيط و الفهارس

| المفتاح | الوصف |
|-----|------------|
| `Names::<name_hash>` | الخريطة الأساسية من `name_hash` إلى `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | فهرس ثانية لمحفظة الواجهة (صديقة للصفحات). |
| `NamesByLabel::<suffix_id, normalized_label>` | كشف العارضات وتمكين البحث الهتمي. |
| `SuffixPolicies::<suffix_id>` | اخر `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | سجل `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | سجل إلحاق فقط بمفتاح سلسلة الاحادي. |

كل لوحة المفاتيح تتسلسل عبر Norito tuples حطمي على تجزئة حطمي عبر المسرحيات. تحديثات الفهرس تتم بشكل ذري مع السجل الأساسي.

## 4. حالات دورة الحياة| الحالة | شروط الدخول | الانتقالات الإضافية | مذكرة |
|-------|----------------|---------------------|-----------|
| متاح | مشغلة عند غياب `NameRecord`. | `PendingAuction` (ممتاز)، `Active` (تسجيل قياسي). | ابحث عن المتوفرة لفيلم الفهارس فقط. |
| في انتظار المزاد | لسبب عندما `PriceTierV1.auction_kind` != none. | `Active` (تسوية المزاد), `Tombstoned` (بدون عروض). | المزادات `AuctionOpened` و `AuctionSettled`. |
| نشط | تسجيل او تجديد ناجح. | `GracePeriod`، `Frozen`، `Tombstoned`. | `expires_at` قادم. |
| فترة النعمة | تلقائي عند `now > expires_at`. | `Active` (تجديد في الوقت)، `Redemption`، `Tombstoned`. | الافتراضي +30 يومًا؛ ما يحل يحل معلّم. |
| فداء | `now > grace_expires_at` لكن `< redemption_expires_at`. | `Active` (تجديد متاخر), `Tombstoned`. | اوامر تتطلب جرح. |
| مجمدة | تجميد او الوصي. | `Active` (بعد المعالجة المركزية)، `Tombstoned`. | لا يمكن نقل الاسم او تحديث وحدات التحكم. |
| شاهد القبر | تنازل طوعي، نتيجة لنتيجة دائمة، او انتهاء الفداء. | `PendingAuction` (إعادة فتح الهولندية) أو يظل محفورًا. | الحدث `NameTombstoned` يجب ان يشمل المبدع. |

يجب ان يتم فحص انتقالات الحالة `RegistryEventKind` بشكل جيد حتى تبقى ذاكرات التخزين المؤقت النهائية متسقة. الاسماء Tombstoned التي أثرت على مزادات هولندية أعيد فتحها ترفق الحمولة النافعة `AuctionKind::DutchReopen`.

##5.الاحداث القياسية و المزامنة للبوابات

اشترك البوابات في `RegistryEventV1` وتزامن DNS/SoraFS عبر:1. اخر جلب `NameRecordV1` ما اليه في سلسلة الاحداث.
2. إعادة توليد قوالب المحلل (عناوين i105 المفضلة + المضغوطة (`sora`) كخيار ثانٍ، السجلات النصية).
3. تثبيت بيانات المنطقة عبر تدفق SoraDNS الموضح في [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

ضمانات تسليم الاحداث:

- كل ما تريده على `NameRecordV1` *يجب* ان تحميل حدث واحدا فقط مع `version` المزيد بصرامة.
- احداث `RevenueSharePosted` شاهد الى التسويقيات الصادرة من `RevenueShareRecordV1`.
- احداثتجميد/unfreeze/علامة مميزة تتضمن تجزئات لقطع التصفح الداخلي `metadata` لاعادة تدقيق الاحداث.

## 6.مثلة على الحمولات Norito

### 6.1 مثال NameRecord

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "<katakana-i105-account-id>",
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

### 6.2 مثال SuffixPolicy

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "<katakana-i105-account-id>",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("<katakana-i105-account-id>"), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "<katakana-i105-account-id>",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. الخطوات التالية

- **SN-2b (واجهة برمجة تطبيقات المسجل وخطافات الإدارة):** عرض هذه البنيات عبر Torii (Norito وروابط JSON) وربط فحوصات القبول التي يمكنك التحكم بها.
- **SN-3 (محرك المزاد والتسجيل):** إعادة استخدام `NameAuctionStateV1` منطقة الالتزام/الكشف وإعادة فتح الهولندية.
- **SN-5 (الدفع والتسوية):** استخدام `RevenueShareRecordV1` للتسويق المالي واتمة التقدير.

يجب تقديم الاسئلة او طلبات الغد مع تحديثات الكهرباء طريق SNS في `roadmap.md` وعكسها في `status.md` عند المدمج.