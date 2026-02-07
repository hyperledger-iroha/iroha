---
lang: ar
direction: rtl
source: docs/portal/docs/sns/registry-schema.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة فوينتي كانونيكا
تعكس هذه الصفحة `docs/source/sns/registry_schema.md` والآن يتم إرسالها كنسخة Canonica من البوابة. يعمل الأرشيف على الحفاظ على تحديثات التجارة.
:::

# علامة تسجيل خدمة اسم Sora (SN-2a)

**الحالة:** تم التعديل بتاريخ 24-03-2026 -- إرسال مراجعة لبرنامج SNS  
**إرفاق خريطة الطريق:** SN-2a "مخطط التسجيل وتخطيط التخزين"  
**Alcance:** تحديد الهياكل الأساسية Norito وحالة دورة الحياة والأحداث الصادرة لخدمة اسم Sora (SNS) بطريقة يتم تحديد عمليات تنفيذ التسجيل والمسجل بها في العقود وSDKs والبوابات.

يكتمل هذا المستند بالمفتاح القابل للدمج لـ SN-2a على النحو المحدد:

1. المعرفات وقواعد التجزئة (`SuffixId`، `NameHash`، اشتقاق المحددات).
2. الهياكل/التعدادات Norito لسجلات الأسماء وسياسات الصوفيين ومستويات الأسعار وأجزاء الإدخال وأحداث السجل.
3. تخطيط التخزين وبادئات المؤشرات لإعادة التحديد.
4. آلة حالة تشمل السجل والتجديد والنعمة/التنازل والتجميد وشواهد القبور.
5. الأحداث الأساسية المستهلكة من خلال أتمتة بوابة/DNS.

## 1. المعرفات والتجزئة| المعرف | الوصف | اشتقاق |
|------------|-------------|------------|
| `SuffixId` (`u16`) | معرف السجل للمستوى الأعلى (`.sora`، `.nexus`، `.dao`). تم إنشاؤه باستخدام كتالوج الصوفيات في [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Assignado por voto de gobernanza; الماكينادو في `SuffixPolicyV1`. |
| `SuffixSelector` | Forma canonica en string del sufijo (ASCII، أحرف صغيرة). | المثال: `.sora` -> `sora`. |
| `NameSelectorV1` | محدد ثنائي لبيانات التسجيل. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. الميزة هي NFC + الحالة الصغيرة في الإصدار Norm v1. |
| `NameHash` (`[u8;32]`) | مفتاح البحث الأساسي يستخدم للعقود والأحداث والمخابئ. | `blake3(NameSelectorV1_bytes)`. |

متطلبات الحتمية:

- تم تطبيع الإعدادات عبر Norm v1 (UTS-46 الصارم، STD3 ASCII، NFC). تتم تسوية سلاسل المستخدم DEBEN قبل التجزئة.
- لا يتم إدخال العلامات المحجوزة (من `SuffixPolicyV1.reserved_labels`) في السجل؛ يتم إصدار التجاوزات فقط بواسطة الأحداث `ReservedNameAssigned`.

## 2. الهياكل الأساسية Norito

### 2.1 NameRecordV1| كامبو | تيبو | نوتاس |
|-------|------|-------|
| `suffix_id` | `u16` | المرجع `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | لا توجد وحدات بايت من المحدد لمعالجة الصوت/التصحيح. |
| `name_hash` | `[u8; 32]` | مفتاح للخرائط/الأحداث. |
| `normalized_label` | `AsciiString` | آداب مقروءة من أجل البشر (نشر القاعدة v1). |
| `display_label` | `AsciiString` | غلاف مقدم من قبل المضيف؛ مستحضرات التجميل اختيارية. |
| `owner` | `AccountId` | التحكم في التجديدات/التحويلات. |
| `controllers` | `Vec<NameControllerV1>` | مراجع إلى عناوين الأهداف أو محللي البيانات الوصفية للتطبيق. |
| `status` | `NameStatus` | Bandera de ciclo de vida (الإصدار القسم 4). |
| `pricing_class` | `u8` | Indice en tiers de precios del sufijo (قياسي، مميز، محجوز). |
| `registered_at` | `Timestamp` | الطابع الزمني لحظر التنشيط الأولي. |
| `expires_at` | `Timestamp` | نهاية النهاية. |
| `grace_expires_at` | `Timestamp` | Fin de gracia de auto-renovation (الافتراضي +30 يومًا). |
| `redemption_expires_at` | `Timestamp` | زعانف انخفاض النافذة (افتراضي +60 دياس). |
| `auction` | `Option<NameAuctionStateV1>` | Presente cuando se reabre Dutch o subastas premium estan activas. |
| `last_tx_hash` | `Hash` | يحدد اللاعب المعاملة التي ينتج عنها هذا الإصدار. || `metadata` | `Metadata` | البيانات الوصفية التحكيمية للمسجل (السجلات النصية والبراهين). |

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

### 2.2 SuffixPolicyV1| كامبو | تيبو | نوتاس |
|-------|------|-------|
| `suffix_id` | `u16` | كلاف بريماريا؛ مستقرة بين الإصدارات السياسية. |
| `suffix` | `AsciiString` | على سبيل المثال، `sora`. |
| `steward` | `AccountId` | تم تحديد المضيف في ميثاق الإدارة. |
| `status` | `SuffixStatus` | `Active`، `Paused`، `Revoked`. |
| `payment_asset_id` | `AsciiString` | معرف نشاط التسوية بسبب العيب (على سبيل المثال `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | معاملات الأسعار حسب المستويات وأنظمة التحمل. |
| `min_term_years` | `u8` | Piso para el termino comprado دون تجاوز تجاوزات المستوى. |
| `grace_period_days` | `u16` | الافتراضي 30. |
| `redemption_period_days` | `u16` | الافتراضي 60. |
| `max_term_years` | `u8` | الحد الأقصى للتجديد من أجل التكيف. |
| `referral_cap_bps` | `u16` | <=1000 (10%) من الميثاق. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Lista sumministrada por gobernanza with تعليمات التعيين. |
| `fee_split` | `SuffixFeeSplitV1` | Porciones de tesoreria / ستيوارد / إحالة (نقاط الأساس). |
| `fund_splitter_account` | `AccountId` | Cuenta que mantiene escrow + distribuye fondos. |
| `policy_version` | `u16` | زيادة في كل تغيير. |
| `metadata` | `Metadata` | الملاحظات الموسعة (ميثاق مؤشرات الأداء الرئيسية، تجزئات مستندات الولاء). |```text
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

### 2.3 سجلات الإدخال والتسوية

| هيكل | كامبوس | اقتراح |
|--------|--------|---------|
| `RevenueShareRecordV1` | `suffix_id`، `epoch_id`، `treasury_amount`، `steward_amount`، `referral_amount`، `escrow_amount`، `settled_at`، `tx_hash`. | سجل تحديد المدفوعات الموجه إلى عصر التسوية (الشهري). |
| `RevenueAccrualEventV1` | `name_hash`، `suffix_id`، `event`، `gross_amount`، `net_amount`، `referral_account`. | Emitido cada vez que un pago seregistra (تسجيل، تجديد، فرعي). |

تستخدم جميع المجالات `TokenValue` الكود القانوني Norito مع كود العملة المعلن في `SuffixPolicyV1` المرتبط.

### 2.4 أحداث التسجيل

أثبتت الأحداث الأساسية وجود سجل إعادة تشغيل لأتمتة بوابة DNS/التحليلات.

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

يجب أن تجمع الأحداث سجلاً قابلاً للتكرار (على سبيل المثال، النطاق `RegistryEvents`) وتعيد نشره في خلاصات البوابة حتى تبطل ذاكرات DNS المؤقتة في اتفاقية مستوى الخدمة (SLA).

## 3. تخطيط التخزين والمؤشرات| كلاف | الوصف |
|-----|------------|
| `Names::<name_hash>` | الخريطة الأولية من `name_hash` إلى `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | مؤشر ثانوي لواجهة مستخدم المحفظة (صفحة ودية). |
| `NamesByLabel::<suffix_id, normalized_label>` | كشف الصراعات، القدرة على البحث عن الحتمية. |
| `SuffixPolicies::<suffix_id>` | ألتيمو `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | تاريخي `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | سجل الإلحاق فقط مع مفتاح الأمان الرتيب. |

يتم تسلسل جميع المفاتيح باستخدام Norito للحفاظ على تحديد التجزئة بين المضيفين. تتم تحديثات المؤشرات بشكل ذري جنبًا إلى جنب مع السجل الأساسي.

## 4. آلة حالة دورة الحياة| حالة | شروط الدخول | ترانزيسيونيس مسموحات | نوتاس |
|-------|----------------------------------------|---------|-------|
| متاح | مشتق من `NameRecord` هذا صحيح. | `PendingAuction` (ممتاز)، `Active` (السجل القياسي). | البحث عن التوفر للمؤشرات الفردية. |
| في انتظار المزاد | إنشاء cuando `PriceTierV1.auction_kind` != لا شيء. | `Active` (la subasta se Liquida)، `Tombstoned` (الخطيئة). | تصدر الكائنات الفرعية `AuctionOpened` و`AuctionSettled`. |
| نشط | التسجيل أو التجديد الخروج. | `GracePeriod`، `Frozen`، `Tombstoned`. | `expires_at` يدفع عملية النقل. |
| فترة النعمة | اوتوماتيكي عند `now > expires_at`. | `Active` (التجديد في الوقت المناسب)، `Redemption`، `Tombstoned`. | الافتراضي +30 دياس. aun resuelve pero marcado. |
| فداء | `now > grace_expires_at` بيرو `< redemption_expires_at`. | `Active` (تجديد التأخر)، `Tombstoned`. | يتطلب القادة رسومًا جزائية. |
| مجمدة | تجميد الحكومة أو الوصي. | `Active` (المعالجة)، `Tombstoned`. | لا يمكن نقل أو تحديث وحدات التحكم. |
| شاهد القبر | التسليم الطوعي، نتيجة نزاع دائم، أو انتهاء صلاحية التنازل. | `PendingAuction` (إعادة الفتح الهولندي) أو شاهد القبر الدائم. | يجب أن يشتمل الحدث `NameTombstoned` على سبب وجيه. |تُصدر تحويلات حالة DEBEN المراسلات `RegistryEventKind` حتى تظل ذاكرات التخزين المؤقت في اتجاه مجرى النهر متماسكة. يتم إعادة فتح الأسماء المميزة التي تدخل إلى الأقسام الفرعية الهولندية مع حمولة `AuctionKind::DutchReopen`.

## 5. الأحداث الأساسية ومزامنة البوابات

تشترك البوابات في `RegistryEventV1` وتتزامن مع DNS/SoraFS في المتوسط:

1. احصل على `NameRecordV1` المرجع الأخير لتأمين الأحداث.
2. إعادة إنشاء قوالب المحلل (الاتجاهات المفضلة IH58 + المضغوطة (`sora`) كخيار ثانٍ، والسجلات النصية).
3. قم بتثبيت بيانات المنطقة التي تم تحديثها عبر التدفق الموصوف في SoraDNS في [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

ضمانات متابعة الأحداث:

- كل معاملة تؤثر على `NameRecordV1` *يجب* أن تجمع حدثًا بالضبط مع `version` بشكل متزايد.
- الأحداث الصادرة `RevenueSharePosted` هي السوائل الصادرة بواسطة `RevenueShareRecordV1`.
- تشتمل أحداث التجميد/إلغاء التجميد/علامة مميزة على تجزئات من المصنوعات اليدوية داخل `metadata` لإعادة تشغيل الجلسة.

## 6. نماذج الحمولات Norito

### 6.1 مثال NameRecord

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

### 6.2 مثال سياسة اللاحقة

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

## 7. بروكسيموس باسوس- **SN-2b (واجهة برمجة تطبيقات المسجل وخطافات الإدارة):** شرح هذه الهياكل عبر Torii (الارتباطات Norito وJSON) وتوصيل عمليات التحقق من القبول إلى المصنوعات اليدوية.
- **SN-3 (محرك المزاد والتسجيل):** إعادة استخدام `NameAuctionStateV1` لتنفيذ منطق الالتزام/الكشف وإعادة الفتح الهولندي.
- **SN-5 (الدفع والتسوية):** يوافق على `RevenueShareRecordV1` للتسوية المالية وأتمتة التقارير.

يجب تسجيل الأسئلة أو طلبات التغيير جنبًا إلى جنب مع تحديثات خريطة طريق SNS في `roadmap.md` وإعادتها إلى `status.md` عندما تصبح متكاملة.