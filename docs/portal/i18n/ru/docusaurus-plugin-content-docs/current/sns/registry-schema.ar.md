---
lang: ru
direction: ltr
source: docs/portal/docs/sns/registry-schema.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note المصدر القياسي
تعكس `docs/source/sns/registry_schema.md` وتعمل الان كنسخة البوابة القياسية. يبقى ملف المصدر لتحديثات الترجمة.
:::

# مخطط سجل Sora Name Service (SN-2a)

**الحالة:** مسودة 2026-03-24 -- مقدمة لمراجعة برنامج SNS  
**رابط خارطة الطريق:** SN-2a "Registry schema & storage layout"  
**النطاق:** تعريف هياكل Norito القياسية وحالات دورة الحياة والاحداث المنبعثة لخدمة Sora Name Service (SNS) لضمان ان تنفيذات السجل والـ registrar تبقى حتمية عبر العقود و SDKs و gateways.

يكمل هذا المستند تسليم المخطط لـ SN-2a عبر تحديد:

1. المعرفات وقواعد hashing (`SuffixId`, `NameHash`, اشتقاق selector).
2. Norito structs/enums لسجلات الاسماء، سياسات suffix، tiers التسعير، توزيعات الايراد، واحداث السجل.
3. تخطيط التخزين وبوادئ الفهارس لضمان replay حتمي.
4. الة حالات تغطي التسجيل، التجديد، grace/redemption، التجميد و tombstone.
5. الاحداث القياسية التي تستهلكها اتمتة DNS/gateway.

## 1. المعرفات و hashing

| المعرف | الوصف | الاشتقاق |
|------------|-------------|------------|
| `SuffixId` (`u16`) | معرف السجل للسوفكسات العليا (`.sora`, `.nexus`, `.dao`). متوافق مع كتالوج السوفكس في [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | يعين بتصويت الحوكمة; يخزن في `SuffixPolicyV1`. |
| `SuffixSelector` | الشكل النصي القياسي للسوفكس (ASCII, lower-case). | مثال: `.sora` -> `sora`. |
| `NameSelectorV1` | selector ثنائي للوسم المسجل. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. الوسم NFC + lower-case حسب Norm v1. |
| `NameHash` (`[u8;32]`) | مفتاح البحث الاساسي المستخدم في العقود والاحداث والكاشات. | `blake3(NameSelectorV1_bytes)`. |

متطلبات الحتمية:

- يتم تطبيع الوسوم عبر Norm v1 (UTS-46 strict, STD3 ASCII, NFC). يجب تطبيع سلاسل المستخدم قبل hashing.
- الوسوم المحجوزة (من `SuffixPolicyV1.reserved_labels`) لا تدخل السجل ابدا; التجاوزات الخاصة بالحوكمة فقط تصدر احداث `ReservedNameAssigned`.

## 2. هياكل Norito

### 2.1 NameRecordV1

| الحقل | النوع | الملاحظات |
|-------|------|-----------|
| `suffix_id` | `u16` | مرجع `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | بايتات selector الخام للتدقيق/debug. |
| `name_hash` | `[u8; 32]` | مفتاح للخرائط/الاحداث. |
| `normalized_label` | `AsciiString` | وسم قابل للقراءة (بعد Norm v1). |
| `display_label` | `AsciiString` | تنسيق احرف يقدمه steward; تجميلي. |
| `owner` | `AccountId` | يتحكم في التجديدات/التحويلات. |
| `controllers` | `Vec<NameControllerV1>` | مراجع لعناوين الحساب او resolvers او metadata للتطبيق. |
| `status` | `NameStatus` | حالة دورة الحياة (انظر القسم 4). |
| `pricing_class` | `u8` | فهرس tiers التسعير للسوفكس (standard, premium, reserved). |
| `registered_at` | `Timestamp` | وقت تفعيل التسجيل الاول. |
| `expires_at` | `Timestamp` | نهاية المدة المدفوعة. |
| `grace_expires_at` | `Timestamp` | نهاية grace للتجديد التلقائي (default +30 يوما). |
| `redemption_expires_at` | `Timestamp` | نهاية نافذة redemption (default +60 يوما). |
| `auction` | `Option<NameAuctionStateV1>` | يظهر عند Dutch reopen او مزادات premium النشطة. |
| `last_tx_hash` | `Hash` | مؤشر حتمي على المعاملة التي انتجت هذه النسخة. |
| `metadata` | `Metadata` | metadata اختيارية للـ registrar (text records, proofs). |

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

### 2.2 SuffixPolicyV1

| الحقل | النوع | الملاحظات |
|-------|------|-----------|
| `suffix_id` | `u16` | مفتاح اساسي ثابت عبر نسخ السياسة. |
| `suffix` | `AsciiString` | مثلا `sora`. |
| `steward` | `AccountId` | steward معرف في charter الحوكمة. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | معرف اصل settlement الافتراضي (مثلا `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | معاملات التسعير حسب tiers وقواعد المدة. |
| `min_term_years` | `u8` | حد ادنى لمدة الشراء بغض النظر عن overrides. |
| `grace_period_days` | `u16` | Default 30. |
| `redemption_period_days` | `u16` | Default 60. |
| `max_term_years` | `u8` | الحد الاقصى للتجديد المسبق. |
| `referral_cap_bps` | `u16` | <=1000 (10%) حسب charter. |
| `reserved_labels` | `Vec<ReservedNameV1>` | قائمة من الحوكمة مع تعليمات التخصيص. |
| `fee_split` | `SuffixFeeSplitV1` | حصص treasury / steward / referral (basis points). |
| `fund_splitter_account` | `AccountId` | حساب يمسك escrow ويقسم الاموال. |
| `policy_version` | `u16` | يزيد مع كل تغيير. |
| `metadata` | `Metadata` | ملاحظات موسعة (KPI covenant, hashes لوثائق compliance). |

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

### 2.3 سجلات الايرادات و settlement

| Struct | الحقول | الغرض |
|--------|--------|-------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | سجل حتمي لمدفوعات موزعة حسب حقبة settlement (اسبوعيا). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | يصدر عند تسجيل كل دفعة (تسجيل، تجديد، مزاد). |

جميع حقول `TokenValue` تستخدم الترميز الثابت القياسي لنوريتو مع كود العملة المعلن في `SuffixPolicyV1` المرتبط.

### 2.4 احداث السجل

الاحداث القياسية توفر replay log لعمليات DNS/gateway والقياس.

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

يجب اضافة الاحداث الى log قابل للاعادة (مثل نطاق `RegistryEvents`) وعكسها الى feeds الخاصة بالبوابة حتى تنتهي صلاحية caches DNS ضمن SLA.

## 3. تخطيط التخزين والفهارس

| المفتاح | الوصف |
|-----|-------------|
| `Names::<name_hash>` | خريطة اساسية من `name_hash` الى `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | فهرس ثانوي لواجهة wallet (pagination friendly). |
| `NamesByLabel::<suffix_id, normalized_label>` | كشف التعارضات وتمكين البحث الحتمي. |
| `SuffixPolicies::<suffix_id>` | اخر `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | سجل `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | log append-only بمفتاح تسلسل احادي. |

كل المفاتيح تتسلسل عبر Norito tuples للحفاظ على hashing الحتمي عبر المضيفات. تحديثات الفهرس تتم بشكل ذري مع السجل الاساسي.

## 4. الة حالات دورة الحياة

| الحالة | شروط الدخول | الانتقالات المسموحة | الملاحظات |
|-------|----------------|---------------------|------------|
| Available | مشتقة عند غياب `NameRecord`. | `PendingAuction` (premium), `Active` (standard registration). | البحث عن التوفر يقرأ الفهارس فقط. |
| PendingAuction | تنشأ عندما `PriceTierV1.auction_kind` != none. | `Active` (تسوية المزاد), `Tombstoned` (بدون عروض). | المزادات تصدر `AuctionOpened` و `AuctionSettled`. |
| Active | تسجيل او تجديد ناجح. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` يقود الانتقال. |
| GracePeriod | تلقائي عند `now > expires_at`. | `Active` (تجديد في الوقت), `Redemption`, `Tombstoned`. | Default +30 يوما; ما زال يحل لكنه معلّم. |
| Redemption | `now > grace_expires_at` لكن `< redemption_expires_at`. | `Active` (تجديد متاخر), `Tombstoned`. | الاوامر تتطلب رسوم عقوبة. |
| Frozen | تجميد حوكمة او guardian. | `Active` (بعد المعالجة), `Tombstoned`. | لا يمكن نقل الاسم او تحديث controllers. |
| Tombstoned | تنازل طوعي، نتيجة نزاع دائم، او انتهاء redemption. | `PendingAuction` (Dutch reopen) او يبقى tombstoned. | حدث `NameTombstoned` يجب ان يتضمن السبب. |

يجب ان تصدر انتقالات الحالة `RegistryEventKind` الموافق حتى تبقى caches downstream متسقة. الاسماء tombstoned التي تدخل مزادات Dutch reopen ترفق payload `AuctionKind::DutchReopen`.

## 5. الاحداث القياسية و sync للبوابات

تشترك البوابات في `RegistryEventV1` وتزامن DNS/SoraFS عبر:

1. جلب اخر `NameRecordV1` المشار اليه في تسلسل الاحداث.
2. اعادة توليد resolver templates (عناوين IH58 المفضلة + compressed (`sora`) كخيار ثانٍ، text records).
3. تثبيت بيانات المنطقة عبر تدفق SoraDNS الموضح في [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

ضمانات تسليم الاحداث:

- كل معاملة تؤثر على `NameRecordV1` *يجب* ان تضيف حدثا واحدا فقط مع `version` متزايد بصرامة.
- احداث `RevenueSharePosted` تشير الى التسويات الصادرة من `RevenueShareRecordV1`.
- احداث freeze/unfreeze/tombstone تتضمن hashes لقطع الحوكمة داخل `metadata` لاعادة تدقيق الاحداث.

## 6. امثلة على Norito payloads

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

### 6.2 مثال SuffixPolicy

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

## 7. الخطوات التالية

- **SN-2b (Registrar API & governance hooks):** عرض هذه structs عبر Torii (Norito و JSON bindings) وربط admission checks بقطع الحوكمة.
- **SN-3 (Auction & registration engine):** اعادة استخدام `NameAuctionStateV1` لتنفيذ منطق commit/reveal و Dutch reopen.
- **SN-5 (Payment & settlement):** استخدام `RevenueShareRecordV1` للتسوية المالية واتوتمة التقارير.

يجب تقديم الاسئلة او طلبات التغيير مع تحديثات خارطة طريق SNS في `roadmap.md` وعكسها في `status.md` عند الدمج.
