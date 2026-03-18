---
lang: ar
direction: rtl
source: docs/portal/docs/sns/registry-schema.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة المصدر الكنسي
تعكس هذه الصفحة `docs/source/sns/registry_schema.md` وستؤدي إلى تغيير شكل النسخة الأساسية من اللوحة. يبقى الملف المصدر لأيام الترجمة.
:::

# مخطط تسجيل خدمة اسم Sora (SN-2a)

**الحالة:** Redige 24-03-2026 -- مشارك في مراجعة برنامج SNS  
**خريطة طريق الامتياز:** SN-2a "مخطط التسجيل وتخطيط التخزين"  
**المنفذ:** تحديد الهياكل الأساسية Norito وحالات دورة الحياة والأحداث الصادرة لخدمة أسماء Sora (SNS) حتى يتم تحديد تطبيقات التسجيل والمسجل في العقود ومجموعات SDK والبوابات.

يكمل هذا المستند المخطط القابل للتشغيل لـ SN-2a بشكل محدد:

1. المعرفات وقواعد التجزئة (`SuffixId`، `NameHash`، اشتقاق المحددات).
2. الهياكل/التعدادات Norito لتسجيلات الأسماء وسياسات اللواحق ومستويات الأسعار وإعادة تقسيم الإيرادات وأحداث التسجيل.
3. تخطيط المخزون وبادئات الفهارس لتحديد إعادة التشغيل.
4. تغطي آلة الحالة التسجيل والتجديد والنعمة/الاسترداد والتجميد وشواهد القبور.
5. الأحداث الأساسية المستهلكة من خلال أتمتة DNS/البوابة.

## 1. المعرفات والتجزئة| معرف | الوصف | الاشتقاق |
|------------|-------------|------------|
| `SuffixId` (`u16`) | معرف التسجيل للاحقات المستوى الأول (`.sora`، `.nexus`، `.dao`). قم بمحاذاة كتالوج اللواحق في [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | ينسب إلى التصويت للحكم؛ المخزون في `SuffixPolicyV1`. |
| `SuffixSelector` | Forme canonique en chaine du suffixe (ASCII، أحرف صغيرة). | مثال: `.sora` -> `sora`. |
| `NameSelectorV1` | حدد ثنائيًا لتسجيل الملصق. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Le label est NFC + أحرف صغيرة selon Norm v1. |
| `NameHash` (`[u8;32]`) | يتم استخدام أول بحث أولي من خلال العقود والأحداث والمخابئ. | `blake3(NameSelectorV1_bytes)`. |

متطلبات الحتمية:

- يتم تطبيع التسميات عبر Norm v1 (UTS-46 الصارم، STD3 ASCII، NFC). تستخدم السلاسل DOIVENT ويتم تطبيعها قبل التجزئة.
- العلامات الاحتياطية (de `SuffixPolicyV1.reserved_labels`) لا تدخل في السجل؛ التجاوزات تحكم فقط الأحداث `ReservedNameAssigned`.

## 2. الهياكل Norito

### 2.1 NameRecordV1| بطل | اكتب | ملاحظات |
|-------|------|-------|
| `suffix_id` | `u16` | المرجع `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | ثمانيات التحديد الشامل للتدقيق/التصحيح. |
| `name_hash` | `[u8; 32]` | Cle صب الخرائط/الأحداث. |
| `normalized_label` | `AsciiString` | التسمية lisible pour l'humain (post Norm v1). |
| `display_label` | `AsciiString` | الغلاف متوفر على قدم المساواة مع المضيف؛ خيار مستحضرات التجميل. |
| `owner` | `AccountId` | التحكم في التجديدات/التحويلات. |
| `controllers` | `Vec<NameControllerV1>` | مراجع لعناوين الحسابات أو المحللين أو البيانات التعريفية للتطبيق. |
| `status` | `NameStatus` | مؤشر دورة الحياة (القسم 4). |
| `pricing_class` | `u8` | Index dans les tiers de prix du suffixe (قياسي، مميز، محجوز). |
| `registered_at` | `Timestamp` | الطابع الزمني لكتلة التنشيط الأولي. |
| `expires_at` | `Timestamp` | نهاية المدة المدفوعة. |
| `grace_expires_at` | `Timestamp` | Fin de Grace d'auto-renouvellement (افتراضي +30 يوم). |
| `redemption_expires_at` | `Timestamp` | نهاية نافذة الفداء (افتراضي +60 يوم). |
| `auction` | `Option<NameAuctionStateV1>` | الحاضر عند إعادة فتح اللغة الهولندية أو الاستمرار في الأنشطة المتميزة. |
| `last_tx_hash` | `Hash` | يحدد المؤشر المعاملة التي يصدرها هذا المنتج. || `metadata` | `Metadata` | البيانات الوصفية التحكيمية للمسجل (السجلات النصية والبراهين). |

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

### 2.2 SuffixPolicyV1| بطل | اكتب | ملاحظات |
|-------|------|-------|
| `suffix_id` | `u16` | كلي بريمير؛ مستقرة بين الإصدارات السياسية. |
| `suffix` | `AsciiString` | على سبيل المثال، `sora`. |
| `steward` | `AccountId` | يحدد المضيف في ميثاق الحكم. |
| `status` | `SuffixStatus` | `Active`، `Paused`، `Revoked`. |
| `payment_asset_id` | `AsciiString` | معرف نشاط التسوية الافتراضي (على سبيل المثال `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | معاملات الأسعار حسب المستويات وقواعد المدة. |
| `min_term_years` | `u8` | Plancher pour le terme achete quel queit l'override de tier. |
| `grace_period_days` | `u16` | الافتراضي 30. |
| `redemption_period_days` | `u16` | الافتراضي 60. |
| `max_term_years` | `u8` | الحد الأقصى لتوقعات التجديد. |
| `referral_cap_bps` | `u16` | <=1000 (10%) حسب الميثاق. |
| `reserved_labels` | `Vec<ReservedNameV1>` | القائمة الأربعة للحوكمة مع تعليمات التنفيذ. |
| `fee_split` | `SuffixFeeSplitV1` | خزينة الأجزاء / ستيوارد / إحالة (نقاط الأساس). |
| `fund_splitter_account` | `AccountId` | احسب ما تسحبه من الضمان + قم بتوزيع الأموال. |
| `policy_version` | `u16` | قم بزيادة التغيير. |
| `metadata` | `Metadata` | الملاحظات (ميثاق مؤشرات الأداء الرئيسية، تجزئات مستندات الامتثال). |

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
```### 2.3 سجلات الإيرادات والتسوية

| هيكل | الأبطال | لكن |
|--------|--------|-----|
| `RevenueShareRecordV1` | `suffix_id`، `epoch_id`، `treasury_amount`، `steward_amount`، `referral_amount`، `escrow_amount`، `settled_at`، `tx_hash`. | تسجيل تحديد طرق الدفع في عصر التسوية (hebdomadaire). |
| `RevenueAccrualEventV1` | `name_hash`، `suffix_id`، `event`، `gross_amount`، `net_amount`، `referral_account`. | Emis a chaque paiement poste (تسجيل، تجديد، تسجيل). |

يستخدم جميع الأبطال `TokenValue` الترميز الكنسي الثابت لـ Norito مع إنشاء الكود في `SuffixPolicyV1` المرتبط.

### 2.4 أحداث التسجيل

توفر الأحداث الأساسية سجل إعادة تشغيل لأتمتة DNS/البوابة والتحليل.

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

ستؤدي الأحداث إلى إضافة سجل ممتع (على سبيل المثال، المجال `RegistryEvents`) وإرجاعه إلى بوابة الخلاصات حتى تكون ذاكرات DNS المؤقتة غير صالحة في SLA.

## 3. تخطيط المخزون والفهارس| كلي | الوصف |
|-----|------------|
| `Names::<name_hash>` | الخريطة الأولية من `name_hash` إلى `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | مؤشر ثانوي لمحفظة واجهة المستخدم (سهلة ترقيم الصفحات). |
| `NamesByLabel::<suffix_id, normalized_label>` | اكتشف الصراعات، ثم قم بالبحث المحدد. |
| `SuffixPolicies::<suffix_id>` | ديرنير `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | تاريخي `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | سجل إلحاق فقط cle par تسلسل رتيب. |

يتم تسلسل كافة العناصر عبر مجموعات Norito من أجل حماية التجزئة المحددة بين الفنادق. يتم تشغيل بيانات الفهرس اليومية مع التسجيل الرئيسي.

## 4. آلة وضع دورة الحياة| إيتات | شروط الدخول | تصاريح التحولات | ملاحظات |
|-------|--------------------|--------------------||-------|
| متاح | اشتقاق عندما `NameRecord` غائب. | `PendingAuction` (ممتاز)، `Active` (معيار التسجيل). | البحث المتاح مضاء فقط بالفهارس. |
| في انتظار المزاد | كري عندما `PriceTierV1.auction_kind` != لا شيء. | `Active` (نظام enchere)، `Tombstoned` (aucune enchere). | تنبعث عمليات الإدخال `AuctionOpened` و`AuctionSettled`. |
| نشط | التسجيل أو التجديد reussi. | `GracePeriod`، `Frozen`، `Tombstoned`. | `expires_at` يقود عملية الانتقال. |
| فترة النعمة | آلي عندما `now > expires_at`. | `Active` (تجديد مؤقت)، `Redemption`، `Tombstoned`. | الافتراضي +30 يوم؛ القرار أكثر ماركي. |
| فداء | `now > grace_expires_at` ولكن `< redemption_expires_at`. | `Active` (تجديد متأخر)، `Tombstoned`. | الأوامر التي تتطلب عقوبة السجن. |
| مجمدة | تجميد الحكم أو الوصي. | `Active` (المعالجة المسبقة)، `Tombstoned`. | لا يمكن نقل وحدات التحكم إلى يوم واحد. |
| شاهد القبر | التخلي طوعًا، نتيجة للتقاضي الدائم، أو انتهاء الاسترداد. | `PendingAuction` (إعادة الفتح الهولندي) أو وضع علامة مميزة. | الحدث `NameTombstoned` يتضمن سببًا. |تنتقل انتقالات حالة DOIVENT إلى `RegistryEventKind` المتوافق حتى تظل ذاكرات التخزين المؤقت في اتجاه مجرى النهر متماسكة. Les noms tombstoned entrant en encheres الهولندية تفتح مرفقًا بحمولة `AuctionKind::DutchReopen`.

## 5. بوابة الأحداث والمزامنة

تتصل البوابات بـ `RegistryEventV1` وتتزامن مع DNS/SoraFS عبر:

1. قم باستعادة المرجع الأخير `NameRecordV1` حسب تسلسل الأحداث.
2. قم بإعادة إنشاء نماذج المحلل (العناوين المفضلة I105 + المضغوطة (`sora`) في الخيار الثاني، والسجلات النصية).
3. قم بتثبيت بيانات المنطقة خلال اليوم عبر سير العمل SoraDNS الموضح في [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

ضمانات تأمين الأحداث:

- كل معاملة تؤثر على `NameRecordV1` *افعل* إضافة دقيقة لحدث مع `version` بشكل صارم.
- تشير الأحداث `RevenueSharePosted` إلى تسويات الانبعاث على قدم المساواة `RevenueShareRecordV1`.
- تشتمل الأحداث التي يتم تجميدها/إلغاء تجميدها/علامة مميزة على تجزئات عناصر الإدارة في `metadata` لإعادة تشغيل التدقيق.

## 6. أمثلة الحمولات Norito

### 6.1 مثال NameRecord

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

### 6.2 نموذج لسياسة اللاحقة

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "i105...",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("i105..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "i105...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. الأشرطة Prochaines- **SN-2b (واجهة برمجة تطبيقات المسجل وخطافات الإدارة):** يعرض هذه الهياكل عبر Torii (الارتباطات Norito وJSON) ويربط فحوصات القبول بعناصر الإدارة.
- **SN-3 (محرك المزاد والتسجيل):** إعادة استخدام `NameAuctionStateV1` لتنفيذ منطق الالتزام/الكشف وإعادة فتح اللغة الهولندية.
- **SN-5 (الدفع والتسوية):** المستغل `RevenueShareRecordV1` للتسوية المالية وأتمتة التقارير.

يتم طرح الأسئلة أو طلبات التغيير مع تواريخ خريطة الطريق SNS في `roadmap.md` ويتم طرحها في `status.md` أثناء الاندماج.