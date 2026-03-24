---
lang: pt
direction: ltr
source: docs/portal/docs/sns/registry-schema.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
Verifique `docs/source/sns/registry_schema.md` e verifique o valor do arquivo. Não há problema em fazer isso.
:::

# مخطط سجل Sora Name Service (SN-2a)

**الحالة:** Maio 2026-03-24 -- مقدمة لمراجعة برنامج SNS  
**رابط خارطة الطريق:** SN-2a "Esquema de registro e layout de armazenamento"  
**النطاق:** Faça o download do Norito e verifique o Sora Name Service (SNS) O registrador e o registrador não precisam de nenhum recurso, SDKs e gateways.

يكمل هذا المستند تسليم المخطط لـ SN-2a عبر تحديد:

1. Hashing de código e hash (`SuffixId`, `NameHash`, seletor de número).
2. Norito structs/enums لسجلات الاسماء, سياسات suffix, tiers التسعير, توزيعات الايراد, واحداث السجل.
3. تخطيط التخزين وبوادئ الفهارس لضمان replay حتمي.
4. الة حالات تغطي التسجيل, التجديد, graça/redenção, التجميد e lápide.
5. Verifique o DNS/gateway.

## 1. Metas e hashing

| المعرف | الوصف | الاشتقاق |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Verifique se há algum problema (`.sora`, `.nexus`, `.dao`). Isso pode ser feito em [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | يعين بتصويت الحوكمة; Você está em `SuffixPolicyV1`. |
| `SuffixSelector` | الشكل النصي القياسي للسوفكس (ASCII, minúsculas). | Exemplo: `.sora` -> `sora`. |
| `NameSelectorV1` | selector ثنائي للوسم المسجل. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Função NFC + minúsculas حسب Norm v1. |
| `NameHash` (`[u8;32]`) | Você pode usar o telefone para obter informações sobre o produto e o produto. | `blake3(NameSelectorV1_bytes)`. |

متطلبات الحتمية:

- يتم تطبيع الوسوم عبر Norma v1 (UTS-46 estrito, STD3 ASCII, NFC). Você pode usar o hashing.
- O código de barras (a partir de `SuffixPolicyV1.reserved_labels`) para a instalação; O código de segurança do produto é `ReservedNameAssigned`.

## 2. Nome Norito

### 2.1 NomeRegistroV1| الحقل | النوع | الملاحظات |
|-------|------|-----------|
| `suffix_id` | `u16` | Modelo `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Selecione o selector para /debug. |
| `name_hash` | `[u8; 32]` | مفتاح للخرائط/الاحداث. |
| `normalized_label` | `AsciiString` | وسم قابل للقراءة (Norm v1). |
| `display_label` | `AsciiString` | تنسيق احرف يقدمه mordomo; تجميلي. |
| `owner` | `AccountId` | يتحكم في التجديدات/التحويلات. |
| `controllers` | `Vec<NameControllerV1>` | Eles fornecem recursos e resolvedores e metadados. |
| `status` | `NameStatus` | حالة دورة الحياة (انظر القسم 4). |
| `pricing_class` | `u8` | Camadas de nível padrão (padrão, premium, reservado). |
| `registered_at` | `Timestamp` | وقت تفعيل التسجيل الاول. |
| `expires_at` | `Timestamp` | Não. |
| `grace_expires_at` | `Timestamp` | نهاية Grace للتجديد التلقائي (padrão +30 يوما). |
| `redemption_expires_at` | `Timestamp` | Não há resgate (padrão +60 dias). |
| `auction` | `Option<NameAuctionStateV1>` | يظهر عند Holandês reabre e مزادات premium النشطة. |
| `last_tx_hash` | `Hash` | Isso pode ser feito por meio de uma mensagem de erro. |
| `metadata` | `Metadata` | metadados اختيارية للـ registrador (registros de texto, provas). |

O que fazer:

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

### 2.2 Política de SufixoV1

| الحقل | النوع | الملاحظات |
|-------|------|-----------|
| `suffix_id` | `u16` | Isso pode ser feito por conta própria. |
| `suffix` | `AsciiString` | Exemplo `sora`. |
| `steward` | `AccountId` | mordomo معرف في charter الحوكمة. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Você pode obter um acordo de liquidação (como `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | معاملات التسعير حسب níveis e قواعد المدة. |
| `min_term_years` | `u8` | Não há substituições. |
| `grace_period_days` | `u16` | Padrão 30. |
| `redemption_period_days` | `u16` | Padrão 60. |
| `max_term_years` | `u8` | Não há problema em usá-lo. |
| `referral_cap_bps` | `u16` | <=1000 (10%) حسب charter. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Isso é o que você está fazendo. |
| `fee_split` | `SuffixFeeSplitV1` | حصص tesouraria / administrador / referência (pontos básicos). |
| `fund_splitter_account` | `AccountId` | حساب يمسك escrow e ويقسم الاموال. |
| `policy_version` | `u16` | Não há problema. |
| `metadata` | `Metadata` | ملاحظات موسعة (convênio KPI, hashes e conformidade). |

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

### 2.3 سجلات الايرادات e liquidação| Estrutura | الحقول | الغرض |
|--------|--------|-------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | سجل حتمي لمدفوعات موزعة حسب حقبة liquidação (اسبوعيا). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Você não pode fazer nada (ou não). |

Use o `TokenValue` para obter informações sobre o `SuffixPolicyV1` المرتبط.

### 2.4 احداث السجل

O replay log é o DNS/gateway e o log de reprodução.

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

يجب اضافة الاحداث الى log قابل للاعادة (مثل نطاق `RegistryEvents`) وعكسها الى feeds الخاصة بالبوابة حتى É possível caches DNS por SLA.

## 3. تخطيط التخزين والفهارس

| المفتاح | الوصف |
|-----|-------------|
| `Names::<name_hash>` | Use o `name_hash` para `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Carteira فهرس ثانوي لواجهة (paginação amigável). |
| `NamesByLabel::<suffix_id, normalized_label>` | كشف التعارضات وتمكين البحث الحتمي. |
| `SuffixPolicies::<suffix_id>` | Use `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | É `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | log apenas com acréscimos بمفتاح تسلسل احادي. |

É importante que as tuplas Norito sejam usadas para hashing. Você pode fazer isso sem parar.

## 4. الة حالات دورة الحياة

| الحالة | شروط الدخول | الانتقالات المسموحة | الملاحظات |
|-------|----------------|----------|-----------|
| Disponível | O produto é `NameRecord`. | `PendingAuction` (premium), `Active` (registro padrão). | البحث عن التوفر يقرأ الفهارس فقط. |
| Leilão Pendente | Selecione `PriceTierV1.auction_kind` != none. | `Active` (referência), `Tombstoned` (referência). | As configurações são `AuctionOpened` e `AuctionSettled`. |
| Ativo | تسجيل او تجديد ناجح. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` não é compatível. |
| Período de graça | Este é `now > expires_at`. | `Active` (não disponível), `Redemption`, `Tombstoned`. | Padrão +30 dias; Não há nada que você possa fazer. |
| Redenção | `now > grace_expires_at` por `< redemption_expires_at`. | `Active` (referência), `Tombstoned`. | Você pode fazer isso sem problemas. |
| Congelado | تجميد حوكمة e guardião. | `Active` (referência), `Tombstoned`. | Não há controladores de terceiros e de terceiros. |
| Lápide | تنازل طوعي, نتيجة نزاع دائم, او انتهاء redenção. | `PendingAuction` (reabertura holandesa) e lápide. | O `NameTombstoned` não está disponível. |

O código `RegistryEventKind` pode ser armazenado em cache downstream. الاسماء tombstoned التي تدخل مزادات Holandês reabre ترفق carga útil `AuctionKind::DutchReopen`.

## 5. Faça o download e sincronize o aplicativo

A configuração de `RegistryEventV1` e DNS/SoraFS é executada:

1. Insira `NameRecordV1` no lugar certo.
2. Crie modelos de resolvedor (I105 المفضلة + arquivos de texto compactados (`sora`)).
3. Verifique o código SoraDNS em [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).ضمانات تسليم الاحداث:

- Você pode usar o `NameRecordV1` *يجب* para obter informações sobre o `version` Obrigado.
- Use `RevenueSharePosted` para substituir o `RevenueShareRecordV1`.
- Para congelar/descongelar/tombstone تتضمن hashes لقطع الحوكمة داخل `metadata` لاعادة تدقيق الاحداث.

## 6. Como usar cargas úteis Norito

### 6.1 Nome NameRecord

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

### 6.2 Guia SuffixPolicy

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

## 7. الخطوات التالية

- **SN-2b (API do registrador e ganchos de governança):** عرض هذه structs عبر Torii (Norito e ligações JSON) e verificações de admissão بقطع الحوكمة.
- **SN-3 (mecanismo de leilão e registro):** اعادة استخدام `NameAuctionStateV1` لتنفيذ منطق confirmar/revelar e reabrir holandês.
- **SN-5 (Pagamento e liquidação):** استخدام `RevenueShareRecordV1` للتسوية المالية واتوتمة التقارير.

Você pode usar o SNS em `roadmap.md` e usar o SNS em `roadmap.md`. `status.md` é um problema.