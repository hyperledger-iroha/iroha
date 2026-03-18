---
lang: ar
direction: rtl
source: docs/portal/docs/sns/registry-schema.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة تحمل عنوان `docs/source/sns/registry_schema.md` وتعمل الآن كنسخة من بوابة Canonica. ينبوع الملف الدائم لتحديث الترجمة.
:::

# تسجيل الدخول لخدمة اسم Sora (SN-2a)

**الحالة:** Redigido 2026-03-24 -- الإجراء الفرعي لمراجعة برنامج SNS  
**رابط خريطة الطريق:** SN-2a "مخطط التسجيل وتخطيط التخزين"  
**الخبر:** تعريف كأنظمة Norito الأساسية وحالات دورة الحياة والأحداث الصادرة لخدمة اسم Sora (SNS) حتى تتمكن تطبيقات التسجيل والتسجيل من تحديد المحددات في العقود ومجموعات SDK والبوابات.

هذا المستند كامل أو ممتد لـ SN-2a محدد:

1. معرفات وأنظمة التجزئة (`SuffixId`، `NameHash`، مشتقات التحديد).
2. الهياكل/التعدادات Norito لسجلات الأسماء، وسياسات اللاحقات، ومستويات الأولوية، وإعادة أجزاء الاستلام، وأحداث السجل.
3. تخطيط التخزين وبادئات المؤشرات لإعادة التشغيل الحتمي.
4. آلة حالة سجل كوبريندو، تجديد، نعمة/فداء، تجميد وشواهد القبور.
5. الأحداث الكنسي الاستهلاكية التي يتم إجراؤها بواسطة DNS/البوابة التلقائية.

## 1. تجزئة المعرفات| المعرف | وصف | مشتقات |
|------------|-------------|------------|
| `SuffixId` (`u16`) | معرف السجل لللاحقات ذات المستوى الأعلى (`.sora`، `.nexus`، `.dao`). انضم إلى كتالوج الملحقات في [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Atribuido por voto de Goveranca؛ تم تخزينه في `SuffixPolicyV1`. |
| `SuffixSelector` | صيغة canonica em سلسلة لاحقة (ASCII، أحرف صغيرة). | مثال: `.sora` -> `sora`. |
| `NameSelectorV1` | اختر الخيار الثنائي لنطاق التسجيل. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. التدوير و NFC + الحالة الصغيرة الثانية Norm v1. |
| `NameHash` (`[u8;32]`) | يجب استخدام البحث الأساسي عن طريق العقود والأحداث والمخابئ. | `blake3(NameSelectorV1_bytes)`. |

متطلبات الحتمية:

- نظام التشغيل rotulos sao Normalizados عبر Norm v1 (UTS-46 الصارم، STD3 ASCII، NFC). نظرًا لأن سلاسل المستخدم DEVEM ستكون طبيعية قبل إجراء التجزئة.
- السجلات المحجوزة (de `SuffixPolicyV1.reserved_labels`) لا تدخل في السجل؛ يتجاوز قواعد الإدارة التي تصدر الأحداث `ReservedNameAssigned`.

## 2. استروتوراس Norito

### 2.1 NameRecordV1| كامبو | تيبو | نوتاس |
|-------|------|-------|
| `suffix_id` | `u16` | المرجع `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | تقوم وحدات البايت باختيار كامل للصوت/التصحيح. |
| `name_hash` | `[u8; 32]` | Chave para Mapas/eventos. |
| `normalized_label` | `AsciiString` | Rotulolegivel por humanos (ما بعد القاعدة v1). |
| `display_label` | `AsciiString` | غلاف fornecido pelo ستيوارد؛ مستحضرات التجميل اختيارية. |
| `owner` | `AccountId` | كونترولا رينوفاكويس/ترانسفرينسياس. |
| `controllers` | `Vec<NameControllerV1>` | المراجع المتعلقة بحسابات أخرى أو محللات أو بيانات وصفية للتطبيق. |
| `status` | `NameStatus` | مؤشر ciclo de vida (الإصدار Secao 4). |
| `pricing_class` | `u8` | مؤشر nos tiers de preco do sufixo (قياسي، مميز، محجوز). |
| `registered_at` | `Timestamp` | الطابع الزمني لحظر التنشيط الأولي. |
| `expires_at` | `Timestamp` | فيم دو ترمو باجو. |
| `grace_expires_at` | `Timestamp` | ميزة التجديد التلقائي (افتراضي +30 يومًا). |
| `redemption_expires_at` | `Timestamp` | Fim da janela de redemption (افتراضي +60 يومًا). |
| `auction` | `Option<NameAuctionStateV1>` | Presente quando ha Dutch open ou leiloes premium ativos. |
| `last_tx_hash` | `Hash` | يحدد القدر كيفية التعامل مع هذه المعاملة بالعكس. || `metadata` | `Metadata` | البيانات الوصفية التعسفية للمسجل (السجلات النصية والبراهين). |

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
| `suffix_id` | `u16` | تشاف بريماريا؛ estavel entre versoes de politica. |
| `suffix` | `AsciiString` | على سبيل المثال، `sora`. |
| `steward` | `AccountId` | لم يحدد ستيوارد ميثاق الحكم. |
| `status` | `SuffixStatus` | `Active`، `Paused`، `Revoked`. |
| `payment_asset_id` | `AsciiString` | معرف نشاط التسوية الخاص بالأرض (على سبيل المثال `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | معاملات السعر حسب المستويات والأنظمة المتينة. |
| `min_term_years` | `u8` | السعر المناسب للشراء بشكل مستقل عن تجاوزات الطبقة. |
| `grace_period_days` | `u16` | الافتراضي 30. |
| `redemption_period_days` | `u16` | الافتراضي 60. |
| `max_term_years` | `u8` | الحد الأقصى من التجديد المتوقع. |
| `referral_cap_bps` | `u16` | <=1000 (10%) ثانية أو ميثاق. |
| `reserved_labels` | `Vec<ReservedNameV1>` | قائمة التعليمات الخاصة بالإدارة مع تعليمات التوزيع. |
| `fee_split` | `SuffixFeeSplitV1` | Partes tesouraria / ستيوارد / إحالة (نقاط الأساس). |
| `fund_splitter_account` | `AccountId` | Conta que mantem escrow + Distribui Fundos. |
| `policy_version` | `u16` | زيادة في كل مرة. |
| `metadata` | `Metadata` | ملاحظات قياسية (ميثاق مؤشرات الأداء الرئيسية، تجزئات مستندات الامتثال). |

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
```### 2.3 سجلات الاستلام والتسوية

| هيكل | كامبوس | اقتراح |
|--------|--------|-----------|
| `RevenueShareRecordV1` | `suffix_id`، `epoch_id`، `treasury_amount`، `steward_amount`، `referral_amount`، `escrow_amount`، `settled_at`، `tx_hash`. | سجل الحتمية للمدفوعات الدوارة في عصر التسوية (الشهري). |
| `RevenueAccrualEventV1` | `name_hash`، `suffix_id`، `event`، `gross_amount`، `net_amount`، `referral_account`. | Emitido cada vez que um pagamento e postado (registro, renovacao, leilao). |

جميع المجالات `TokenValue` تستخدم كودًا ثابتًا لـ Norito مع كود الطريقة المُعلن عنه رقم `SuffixPolicyV1` المرتبط.

### 2.4 الأحداث المسجلة

توفر الأحداث الأساسية سجل إعادة تشغيل لـ DNS/البوابة والتحليلات التلقائية.

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

سيتم إرفاق الأحداث بسجل يتم إعادة إنتاجه (على سبيل المثال، النطاق `RegistryEvents`) وإعادة تغذيتنا بخلاصات البوابة حتى لا تكون ذاكرات DNS المؤقتة غير صالحة داخل SLA.

## 3. تخطيط التخزين والمؤشرات| تشاف | وصف |
|-----|------------|
| `Names::<name_hash>` | الخريطة الأولية لـ `name_hash` لـ `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | المؤشر الثاني لواجهة مستخدم المحفظة (صفحة الأصدقاء). |
| `NamesByLabel::<suffix_id, normalized_label>` | اكتشاف الصراعات والقدرة على البحث عن الحتمية. |
| `SuffixPolicies::<suffix_id>` | ألتيمو `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | تاريخ `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | سجل الإلحاق فقط مع سلسلة رتيبة. |

كل ما عليك فعله هو إجراء تسلسل باستخدام Norito للحفاظ على تحديد التجزئة بين المضيفين. يتم تحديث الفهرس بشكل ذري جنبًا إلى جنب مع السجل الأساسي.

## 4. آلة وضع دورة الحياة| حالة | شروط الدخول | Transicos تصاريح | نوتاس |
|-------|--------------------------------------|------|-------|
| متاح | مشتق عندما `NameRecord` هذا صحيح. | `PendingAuction` (ممتاز)، `Active` (معيار التسجيل). | ابحث عن توفر المؤشرات فقط. |
| في انتظار المزاد | Criado quando `PriceTierV1.auction_kind`!= لا شيء. | `Active` (leilao Liquida)، `Tombstoned` (رماح شبه). | منتج Leiloes `AuctionOpened` و`AuctionSettled`. |
| نشط | قم بالتسجيل أو التجديد. | `GracePeriod`، `Frozen`، `Tombstoned`. | `expires_at` من خلال النقل. |
| فترة النعمة | تلقائي عندما `now > expires_at`. | `Active` (تجديد القطر)، `Redemption`، `Tombstoned`. | الافتراضي +30 دياس. أيندا حل ماس سيناليزادو. |
| فداء | `now > grace_expires_at` ماس `< redemption_expires_at`. | `Active` (رينوفاكاو تارديا)، `Tombstoned`. | يطلب القادة تصنيف العقوبة. |
| مجمدة | تجميد الحكم أو الوصي. | `Active` (apos remediacao)، `Tombstoned`. | لا يمكن نقل وحدات التحكم إلى تحديثها. |
| شاهد القبر | التنازل الطوعي، نتيجة النزاع الدائم أو انتهاء صلاحية الاسترداد. | `PendingAuction` (إعادة الفتح الهولندي) أو شاهد القبر بشكل دائم. | يتضمن الحدث `NameTombstoned` التخفيض. |عندما تقوم عمليات نقل الحالة، تقوم DEVEM بإصدار مراسلات `RegistryEventKind` للحفاظ على ذاكرة التخزين المؤقت للأجزاء المركزية في اتجاه المصب. Nomes tombstoned que entram em leiloes الهولندية تعيد فتح anexam um payload `AuctionKind::DutchReopen`.

## 5. الأحداث الكنسي ومزامنة البوابات

البوابات تعمل `RegistryEventV1` ومزامنة DNS/SoraFS ao:

1. ابحث عن `NameRecordV1` الأخير المرجعي على تسلسل الأحداث.
2. إعادة إنشاء قوالب المحلل (المفضلات I105 المفضلة + المضغوطة (`sora`) مثل الخيار الثاني والسجلات النصية).
3. قم بتثبيت بيانات المنطقة التي تم تحديثها عبر التدفق الموصوف من SoraDNS في [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

ضمانات متابعة الأحداث:

- كل معاملة تقوم بها `NameRecordV1` *تطوير* تضيف حدثًا رائعًا مع `version` بشكل متصاعد.
- تشير الأحداث `RevenueSharePosted` إلى العملات السائلة الصادرة عن `RevenueShareRecordV1`.
- تشتمل أحداث التجميد/إلغاء التجميد/علامة مميزة على تجزئات التحكم الأثرية في `metadata` لإعادة تشغيل الاستماع.

## 6. أمثلة الحمولات Norito

### 6.1 مثال على NameRecord

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

### 6.2 نموذج لسياسة SuffixPolicy

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

## 7. بروكسيموس باسوس- **SN-2b (واجهة برمجة تطبيقات المسجل وخطافات الإدارة):** تصدير هذه الهياكل عبر Torii (الارتباطات Norito e JSON) وعمليات التحقق من القبول لأدوات الإدارة.
- **SN-3 (محرك المزاد والتسجيل):** إعادة استخدام `NameAuctionStateV1` لتنفيذ منطق الالتزام/الكشف وإعادة فتح اللغة الهولندية.
- **SN-5 (الدفع والتسوية):** تمت الموافقة على `RevenueShareRecordV1` للتسوية المالية والعلاقة الآلية.

يجب أن يتم تسجيل الأسئلة أو طلبات التعديل جنبًا إلى جنب مع تحديثات خريطة الطريق SNS في `roadmap.md` وإعادة تقديمها في `status.md` عندما تكون متكاملة.