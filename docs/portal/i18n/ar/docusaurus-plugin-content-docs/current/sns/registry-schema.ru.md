---
lang: ar
direction: rtl
source: docs/portal/docs/sns/registry-schema.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
يعرض هذا الجزء `docs/source/sns/registry_schema.md` ويدعم بوابة النسخ القانونية. يتم تخزين الملف الأصلي للتحويلات الجديدة.
:::

# خدمة اسم سورا (SN-2a)

**الحالة:** تشيرنوفيك 24-03-2026 -- تم تعديله على أحدث برامج SNS  
**نسخة من خريطة الطريق:** SN-2a "مخطط التسجيل وتخطيط التخزين"  
**التقديم:** اقتراح الهياكل الأساسية Norito ومستلزمات دورة الحياة والمشاركة لخدمة Sora Name Service (SNS) لتحقيق التسجيل والمسجل يوقف تحديد العقود وSDK والبوابات.

يوضح هذا المستند مخططات البريد لـ SN-2a، على النحو التالي:

1. المعرفات والاختيار الصحيح (`SuffixId`، `NameHash`، محدد الاشتقاق).
2. بنيات/تعدادات Norito لكتابتها، واللاحقة السياسية، والمستويات السبعة، والإيرادات المتفاوتة، والمسجل المشترك.
3. فهرس التخطيط والبادئات لتحديد الإعادة.
4. الماكينات, التسجيل المميز, التعزيز, النعمة/الفداء, التجميد وشاهدة القبر.
5. الاشتراك القانوني، أتمتة DNS/البوابة المحتملة.

## 1. المعرفات والاختيار| المعرف | الوصف | منتج |
|------------|-------------|------------|
| `SuffixId` (`u16`) | معرف القائمة لللاحقة العليا (`.sora`، `.nexus`، `.dao`). سغلاسوفان بالكتالوج لاحقة في [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | بادئ ذي بدء ، الحكم; ранится в `SuffixPolicyV1`. |
| `SuffixSelector` | Каноническая строковая пома suffixa (ASCII، أحرف صغيرة). | المثال: `.sora` -> `sora`. |
| `NameSelectorV1` | تم تحديد المختار الثنائي حسب الرغبة. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. LEEYBL في NFC + أحرف صغيرة في Norm v1. |
| `NameHash` (`[u8;32]`) | المفتاح الرئيسي هو العقود المستخدمة والعقود والعقود. | `blake3(NameSelectorV1_bytes)`. |

تحديد الرغبة:

- تتم تسوية المستوى عبر Norm v1 (UTS-46 الصارم، STD3 ASCII، NFC). يتم تطبيع السكتة الدماغية الفائضة قبل الاختيار.
- لا يتم إدخال خزان الوقود (من `SuffixPolicyV1.reserved_labels`) إلى المسجل؛ تتجاوز الإدارة فقط تسجيل الاشتراك `ReservedNameAssigned`.

## 2. الهياكل Norito

### 2.1 NameRecordV1| بول | النوع | مساعدة |
|-------|------|-----------|
| `suffix_id` | `u16` | خيار `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | مجموعة مختارة من البيانات للتدقيق/التصحيح. |
| `name_hash` | `[u8; 32]` | مفتاح للبطاقة/الاشتراك. |
| `normalized_label` | `AsciiString` | شيتايمي ليبل (بعد نورم الإصدار 1). |
| `display_label` | `AsciiString` | غلاف من ستيوارد؛ مستحضرات التجميل. |
| `owner` | `AccountId` | إدارة التوزيعات/التحويلات. |
| `controllers` | `Vec<NameControllerV1>` | إعدادات حسابات العنوان أو أدوات الحل أو تطبيقات البيانات الوصفية. |
| `status` | `NameStatus` | علم الحياة البرية (سم. رازديل 4). |
| `pricing_class` | `u8` | مؤشر بمستويات لاحقة (قياسي، مميز، محجوز). |
| `registered_at` | `Timestamp` | وقت التنشيط الأول. |
| `expires_at` | `Timestamp` | كونيتس مرحبًا به. |
| `grace_expires_at` | `Timestamp` | Конец Grace التجديد التلقائي (الافتراضي +30 يومًا). |
| `redemption_expires_at` | `Timestamp` | يتم الاسترداد بالكامل (الافتراضي +60 يومًا). |
| `auction` | `Option<NameAuctionStateV1>` | يرجى تقديم المساعدة عند إعادة فتح الهولندية أو المزادات المتميزة. |
| `last_tx_hash` | `Hash` | تحديد إصدار المعاملة. |
| `metadata` | `Metadata` | Произвольная مسجل البيانات الوصفية (السجلات النصية، البراهين). |

هياكل الدعم:

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

### 2.2 SuffixPolicyV1| بول | النوع | مساعدة |
|-------|------|-----------|
| `suffix_id` | `u16` | الكلمة الأولى; استقرار بين الإصدارات السياسية. |
| `suffix` | `AsciiString` | على سبيل المثال، `sora`. |
| `steward` | `AccountId` | ستيوارد، رائد في ميثاق الحوكمة. |
| `status` | `SuffixStatus` | `Active`، `Paused`، `Revoked`. |
| `payment_asset_id` | `AsciiString` | التسوية النشطة للتسوية (على سبيل المثال `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | التكلفة الوظيفية حسب المستويات والرفاهية المناسبة. |
| `min_term_years` | `u8` | الحد الأدنى من مشتريات المشتريات ليس إدراكًا لتجاوزات الطبقة. |
| `grace_period_days` | `u16` | الافتراضي 30. |
| `redemption_period_days` | `u16` | الافتراضي 60. |
| `max_term_years` | `u8` | الحد الأقصى من العروض المالية. |
| `referral_cap_bps` | `u16` | <=1000 (10%) على الميثاق. |
| `reserved_labels` | `Vec<ReservedNameV1>` | قائمة الحوكمة مع تعليمات خاصة بها. |
| `fee_split` | `SuffixFeeSplitV1` | Доли الخزانة / المضيف / الإحالة (نقاط الأساس). |
| `fund_splitter_account` | `AccountId` | حساب الضمان + خدمة التوزيع. |
| `policy_version` | `u16` | استمتع بالتغييرات في كل مكان. |
| `metadata` | `Metadata` | Расshireнные заметки (ميثاق مؤشرات الأداء الرئيسية، مستندات التجزئة للامتثال). |

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

### 2.3 تسجيل الدخول والتسوية| هيكل | بوليا | الاسم |
|--------|------|------------|
| `RevenueShareRecordV1` | `suffix_id`، `epoch_id`، `treasury_amount`، `steward_amount`، `referral_amount`، `escrow_amount`، `settled_at`، `tx_hash`. | يتم تحديد قرار القرض المتفق عليه في مستوطنة إبوهام (أسبوع). |
| `RevenueAccrualEventV1` | `name_hash`، `suffix_id`، `event`، `gross_amount`، `net_amount`، `referral_account`. | Эмитиruется pri кадом платезе (التسجيل، التجديد، المزاد). |

جميع `TokenValue` تستخدم الترميز القانوني الثابت Norito مع قيم الكود البرمجي `SuffixPolicyV1`.

### 2.4 سجل الاشتراك

السجل الأساسي هو سجل إعادة التشغيل لأتمتة وتحليلات DNS/البوابة.

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

يتم إضافة البيانات إلى سجل قابل لإعادة التشغيل (على سبيل المثال، المجال `RegistryEvents`) ويتم التحقق منها في خلاصات البوابة، مما يؤدي إلى إلغاء صلاحية ذاكرات DNS المؤقتة في المقدمة جيش تحرير السودان.

## 3. تخطيط الفهارس والفهارس

| كليتش | الوصف |
|-----|---------|
| `Names::<name_hash>` | البطاقة الأساسية `name_hash` -> `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | الفهرس القياسي لواجهة مستخدم المحفظة (سهل الاستخدام وترقيم الصفحات). |
| `NamesByLabel::<suffix_id, normalized_label>` | استكشاف الصراعات وتحديد النقاط. |
| `SuffixPolicies::<suffix_id>` | الفعلي `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | التاريخ `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | سجل الإلحاق فقط مع monotonной последовательностью. |جميع المفاتيح التسلسلية هي Norito مجموعات لتحديد الاختيار بين المضيفين. يتم استخدام مؤشر الإرجاع تلقائيًا في السجل الأساسي.

## 4. آلة الحياة الأساسية| الحالة | خدمة المياه | المستلزمات المسبقة | مساعدة |
|-------|----------------|---------------------|-----------|
| متاح | منتج عندما يتم الرد على `NameRecord`. | `PendingAuction` (ممتاز)، `Active` (تسجيل قياسي). | اقرأ المزيد من المؤشرات فقط. |
| في انتظار المزاد | تم الاتصال به عندما `PriceTierV1.auction_kind` != لا شيء. | `Active` (تسوية المزاد)، `Tombstoned` (بدون عطاءات). | المزادات تحرر `AuctionOpened` و`AuctionSettled`. |
| نشط | التسجيل أو التوزيع ناجح. | `GracePeriod`، `Frozen`، `Tombstoned`. | `expires_at` تحرك للأمام. |
| فترة النعمة | أوتوماتيكية عند `now > expires_at`. | `Active` (التجديد في الوقت المحدد)، `Redemption`، `Tombstoned`. | الافتراضي +30 يوم؛ الحل ولكن بسيط. |
| فداء | `now > grace_expires_at` إلى `< redemption_expires_at`. | `Active` (التجديد المتأخر)، `Tombstoned`. | تحتاج الأوامر إلى لوحة قوية. |
| مجمدة | تجميد الحكم أو الوصي. | `Active` (المعالجة اللاحقة)، `Tombstoned`. | قم بالتحويل أو تغيير وحدات التحكم. |
| شاهد القبر | حالة جيدة، هذا هو الخلاص أو الفداء الحقيقي. | `PendingAuction` (إعادة الفتح الهولندي) أو ترك علامة مميزة. | يجب أن تتضمن الرسالة `NameTombstoned` السعر. |تُزيل أولويتنا الأساسية `RegistryEventKind` من أجل تخزين ذاكرات التخزين المؤقتة في اتجاه مجرى النهر. Tombstoned имена, вододящие в الهولندية فتح المزادات, pricrepleaute الحمولة `AuctionKind::DutchReopen`.

## 5. بوابة الاشتراك والمزامنة الكنسية

تتوافق البوابات مع `RegistryEventV1` وتقوم بمزامنة DNS/SoraFS، وهي متاحة:

1. قم بشحن `NameRecordV1` التالي، مما يؤدي إلى شفاء أفضل.
2. قوالب محلل الترقية (I105 افتراضي + مضغوط (`sora`) كاختيار خارجي، وسجلات نصية).
3. قم بتثبيت مناطق البيانات الأساسية عبر سير عمل SoraDNS من [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

ضمانات التسليم:

- كل معاملة تتعلق بـ `NameRecordV1`، *مطلوب* إضافة اشتراك واحد جديد بقوة `version`.
- `RevenueSharePosted` يتم التواصل مع المستوطنات من `RevenueShareRecordV1`.
- يتضمن تجميد/إلغاء التجميد/علامة مميزة هذه العناصر الإدارية الموجودة في `metadata` لإعادة تشغيل التدقيق.

## 6. أمثلة الحمولات Norito

### 6.1 المثال NameRecord

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

### 6.2 سياسة اللاحقة التمهيدية

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

## 7. الخطوات التالية- **SN-2b (واجهة برمجة تطبيقات المسجل وخطافات الإدارة):** فتح هذه الهياكل من خلال Torii (ارتباطات Norito وJSON) والسماح بفحص القبول في عنصر الإدارة.
- **SN-3 (محرك المزاد والتسجيل):** يستخدم `NameAuctionStateV1` لمنطق الالتزام/الكشف وإعادة الفتح الهولندي.
- **SN-5 (الدفع والتسوية):** يستخدم `RevenueShareRecordV1` للتحويلات المالية والأتمتة.

يجب حل مشكلات وحلول التحسين فورًا من خلال خريطة طريق SNS المحددة في `roadmap.md` والخروج في `status.md` خطط.