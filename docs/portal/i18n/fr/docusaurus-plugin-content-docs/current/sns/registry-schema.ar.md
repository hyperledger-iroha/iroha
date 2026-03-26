---
lang: fr
direction: ltr
source: docs/portal/docs/sns/registry-schema.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
عكس `docs/source/sns/registry_schema.md` وتعمل الان كنسخة البوابة القياسية. يبقى ملف المصدر لتحديثات الترجمة.
:::

# Utiliser le service de noms Sora (SN-2a)

**الحالة:** مسودة 2026-03-24 -- مقدمة لمراجعة برنامج SNS  
**رابط خارطة الطريق :** SN-2a "Schéma de registre et disposition du stockage"  
**النطاق:** Télécharger le code Norito pour le service de noms Sora (SNS) pour Sora Name Service (SNS) Les registraires et les registraires ont également des SDK et des passerelles.

يكمل هذا المستند تسليم المخطط لـ SN-2a عبر تحديد:

1. Fonctions de hachage (`SuffixId`, `NameHash`, sélecteur de fonction).
2. Norito structs/enums pour les suffixes de niveaux et les niveaux.
3. تخطيط التخزين وبوادئ الفهارس لضمان replay حتمي.
4. Il s'agit de la grâce/rédemption et de la pierre tombale.
5. Sélectionnez le DNS/passerelle.

## 1. Paramètres et hachage| المعرف | الوصف | الاشتقاق |
|------------|-------------|------------|
| `SuffixId` (`u16`) | معرف السجل لللسوفكسات العليا (`.sora`, `.nexus`, `.dao`). متوافق مع كتالوج السوفكس في [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | يعين بتصويت الحوكمة; Il s'agit de `SuffixPolicyV1`. |
| `SuffixSelector` | الشكل النصي القياسي للسوفكس (ASCII, minuscule). | Utiliser : `.sora` -> `sora`. |
| `NameSelectorV1` | sélecteur ثنائي للوسم المسجل. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. الوسم NFC + minuscules par Norm v1. |
| `NameHash` (`[u8;32]`) | مفتاح البحث الاساسي المستخدم في العقود والاحداث والكاشات. | `blake3(NameSelectorV1_bytes)`. |

متطلبات الحتمية:

- Il s'agit d'une norme v1 (UTS-46 stricte, STD3 ASCII, NFC). Il s'agit d'un outil de hachage.
- الوسوم المحجوزة (من `SuffixPolicyV1.reserved_labels`) لا تدخل السجل ابدا ; Les informations relatives à l'installation sont conformes à `ReservedNameAssigned`.

## 2. Norito

### 2.1 NomEnregistrementV1| الحقل | النوع | الملاحظات |
|-------|------|---------------|
| `suffix_id` | `u16` | Voir `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Utilisez le sélecteur pour/debug. |
| `name_hash` | `[u8; 32]` | مفتاح للخرائط/الاحداث. |
| `normalized_label` | `AsciiString` | Il s'agit de la norme (selon la norme v1). |
| `display_label` | `AsciiString` | تنسيق احرف يقدمه intendant; تجميلي. |
| `owner` | `AccountId` | يتحكم في التجديدات/التحويلات. |
| `controllers` | `Vec<NameControllerV1>` | Il existe des résolveurs et des métadonnées. |
| `status` | `NameStatus` | حالة دورة الحياة (انظر القسم 4). |
| `pricing_class` | `u8` | فهرس niveaux التسعير للسوفكس (standard, premium, réservé). |
| `registered_at` | `Timestamp` | وقت تفعيل التسجيل الاول. |
| `expires_at` | `Timestamp` | نهاية المدة المدفوعة. |
| `grace_expires_at` | `Timestamp` | نهاية grace للتجديد التلقائي (par défaut +30 يوما). |
| `redemption_expires_at` | `Timestamp` | Rédemption de نهاية نافذة (par défaut +60 يوما). |
| `auction` | `Option<NameAuctionStateV1>` | Il y a la réouverture des Pays-Bas et la prime premium. |
| `last_tx_hash` | `Hash` | مؤشر حتمي على المعاملة التي انتجت هذه النسخة. |
| `metadata` | `Metadata` | métadonnées اختيارية للـ registraire (enregistrements de texte, preuves). |

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

### 2.2 SuffixPolicyV1| الحقل | النوع | الملاحظات |
|-------|------|---------------|
| `suffix_id` | `u16` | مفتاح اساسي ثابت عبر نسخ السياسة. |
| `suffix` | `AsciiString` | Il s'agit de `sora`. |
| `steward` | `AccountId` | intendant معرف في charte الحوكمة. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | معرف اصل règlement الافتراضي (مثلا `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | معاملات التسعير حسب niveaux وقواعد المدة. |
| `min_term_years` | `u8` | Il s'agit de remplacements. |
| `grace_period_days` | `u16` | Par défaut 30. |
| `redemption_period_days` | `u16` | Par défaut 60. |
| `max_term_years` | `u8` | الحد الاقصى للتجديد المسبق. |
| `referral_cap_bps` | `u16` | <=1000 (10%) charter. |
| `reserved_labels` | `Vec<ReservedNameV1>` | قائمة من الحوكمة مع تعليمات التخصيص. |
| `fee_split` | `SuffixFeeSplitV1` | حصص trésorerie / intendant / référence (points de base). |
| `fund_splitter_account` | `AccountId` | حساب يمسك escrow ويقسم الاموال. |
| `policy_version` | `u16` | يزيد مع كل تغيير. |
| `metadata` | `Metadata` | ملاحظات موسعة (accord KPI, hachage et conformité). |

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

### 2.3 سجلات الايرادات et règlement| Structure | الحقول | الغرض |
|--------|--------|-------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | سجل حتمي لمدفوعات موزعة حسب حقبة règlement (اسبوعيا). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | يصدر عند تسجيل كل دفعة (تسجيل، تجديد، مزاد). |

جميع حقول `TokenValue` تستخدم الترميز الثابت القياسي لنوريتو مع كود العملة المعلن في `SuffixPolicyV1` المرتبط.

### 2.4 Mise à jour

Les journaux de relecture sont également disponibles pour DNS/passerelle.

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

يجب اضافة الاحداث الى log قابل للاعادة (مثل نطاق `RegistryEvents`) et الى feeds الخاصة بالبوابة حتى تنتهي Il met en cache DNS et SLA.

## 3. تخطيط التخزين والفهارس

| المفتاح | الوصف |
|-----|-------------|
| `Names::<name_hash>` | Utilisez la référence `name_hash` pour `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | فهرس ثانوي لواجهة wallet (pagination conviviale). |
| `NamesByLabel::<suffix_id, normalized_label>` | كشف التعارضات وتمكين البحث الحتمي. |
| `SuffixPolicies::<suffix_id>` | Voir `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Voir `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | log append-only بمفتاح تسلسل احادي. |

Les tuples Norito sont utilisés pour le hachage. تحديثات الفهرس تتم بشكل ذري مع السجل الاساسي.

## 4. الة حالات دورة الحياة| الحالة | شروط الدخول | الانتقالات المسموحة | الملاحظات |
|-------|----------------|-----------|------------|
| Disponible | مشتقة عند غياب `NameRecord`. | `PendingAuction` (premium), `Active` (enregistrement standard). | البحث عن التوفر يقرأ الفهارس فقط. |
| En attente d'enchères | Utilisez `PriceTierV1.auction_kind` != none. | `Active` (تسوية المزاد), `Tombstoned` (بدون عروض). | Les modèles sont `AuctionOpened` et `AuctionSettled`. |
| Actif | تسجيل او تجديد ناجح. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` est disponible. |
| Période de grâce | Il s'agit de `now > expires_at`. | `Active` (تجديد في الوقت), `Redemption`, `Tombstoned`. | Par défaut +30 يوما ; ما زال يحل لكنه معلّم. |
| Rédemption | `now > grace_expires_at` à `< redemption_expires_at`. | `Active` (تجديد متاخر), `Tombstoned`. | الاوامر تتطلب رسوم عقوبة. |
| Congelé | تجميد حوكمة او tuteur. | `Active` (pour معالجة), `Tombstoned`. | Il y a des contrôleurs et des contrôleurs. |
| Tombé | Il s'agit d'une question de rédemption. | `PendingAuction` (réouverture aux Pays-Bas) et est tombé. | حدث `NameTombstoned` يجب ان يتضمن السبب. |

Il s'agit d'un exemple de mise en cache `RegistryEventKind` qui met en cache les caches en aval. Les Pays-Bas ont rouvert la charge utile `AuctionKind::DutchReopen`.

## 5. Fonctions de synchronisation et de synchronisation

Paramètres de `RegistryEventV1` et DNS/SoraFS:1. Utilisez le `NameRecordV1` المشار اليه في تسلسل الاحداث.
2. Utilisez des modèles de résolveur (modèles I105 + compressés (`sora`) pour les enregistrements de texte).
3. Vous pouvez utiliser SoraDNS pour [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

ضمانات تسليم الاحداث:

- كل معاملة تؤثر على `NameRecordV1` *يجب* ان تضيف حدثا واحدا فقط مع `version` متزايد بصرامة.
- احداث `RevenueSharePosted` تشير الى التسويات الصادرة من `RevenueShareRecordV1`.
- Permet de geler/dégeler/tombstone pour les hachages et pour `metadata`.

## 6. Charges utiles Norito

### 6.1 par NameRecord

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

### 6.2 par SuffixPolicy

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

## 7. خطوات التالية

- **SN-2b (API du registraire et hooks de gouvernance) :** Les structures sont basées sur Torii (Norito et liaisons JSON) et les contrôles d'admission sont également disponibles.
- **SN-3 (moteur d'enchères et d'enregistrement) :** اعادة استخدام `NameAuctionStateV1` لتنفيذ منطق commit/reveal et réouverture néerlandaise.
- **SN-5 (Paiement et règlement) :** استخدام `RevenueShareRecordV1` للتسوية المالية واتوتمة التقارير.

يجب تقديم الاسئلة او طلبات التغيير مع تحديثات خارطة طريق SNS pour `roadmap.md` et `status.md` عند الدمج.