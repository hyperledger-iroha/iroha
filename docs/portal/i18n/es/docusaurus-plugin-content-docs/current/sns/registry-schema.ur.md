---
lang: es
direction: ltr
source: docs/portal/docs/sns/registry-schema.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sns/registry_schema.md` کی عکاسی کرتا ہے اور پورٹل کی کینونیکل کاپی کے طور پر کام کرتا ہے۔ سورس فائل ترجمہ اپ ڈیٹس کے لیے برقرار رہے گی۔
:::

# Servicio de nombres Sora رجسٹری اسکیمہ (SN-2a)

**حیثیت:** Actualización 2026-03-24 -- SNS پروگرام ریویو کے لیے جمع کیا گیا  
**روڈمیپ لنک:** SN-2a "Esquema de registro y diseño de almacenamiento"  
**دائرہ:** Sora Name Service (SNS) کے لیے Norito اسٹرکچرز، لائف سائیکل اسٹیٹس اور اخراجی ایونٹس متعین کرنا تاکہ registro اور registrador کی implementaciones کنٹریکٹس، SDK اور gateways میں determinista رہیں۔

یہ دستاویز SN-2a کے اسکیمہ ڈیلیورایبل کو مکمل کرتی ہے، جس میں درج ذیل شامل ہیں:

1. شناختیں اور hash قواعد (`SuffixId`, `NameHash`, derivación del selector) ۔
2. نام ریکارڈز، sufijo پالیسیز، قیمت niveles, ریونیو سپلٹس اور رجسٹری ایونٹس کے لیے Norito estructuras / enumeraciones ۔
3. reproducción determinista کے لیے diseño de almacenamiento اور prefijos de índice۔
4. رجسٹریشن، تجدید، gracia/redención, congelación اور lápida پر مشتمل máquina de estado۔
5. Automatización de DNS/gateway کے لیے eventos canónicos۔

## 1. شناختیں اور hash| شناخت | وضاحت | اخذ |
|------------|-------------|------------|
| `SuffixId` (`u16`) | ٹاپ لیول sufijos (`.sora`, `.nexus`, `.dao`) کے لیے رجسٹری شناخت۔ [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) کے catálogo de sufijos کے مطابق۔ | votación de gobernanza سے مختص; `SuffixPolicyV1` Pantalla táctil |
| `SuffixSelector` | sufijo کی cadena canónica شکل (ASCII, minúscula)۔ | Nombre: `.sora` -> `sora`. |
| `NameSelectorV1` | رجسٹر شدہ لیبل کے لیے بائنری selector۔ | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. لیبل Norm v1 کے مطابق NFC + minúsculas۔ |
| `NameHash` (`[u8;32]`) | کنٹریکٹس، ایونٹس اور cachés کے لیے بنیادی سرچ key۔ | `blake3(NameSelectorV1_bytes)`. |

Determinismo کی ضروریات:

- لیبلز Norm v1 (UTS-46 estricto, STD3 ASCII, NFC) کے ذریعے normalizar ہوتے ہیں۔ صارف کی cadenas کو hash سے پہلے normalizar کرنا لازم ہے۔
- Etiquetas reservadas (`SuffixPolicyV1.reserved_labels`) کبھی registro میں داخل نہیں ہوتے؛ anulaciones de solo gobernanza `ReservedNameAssigned` ایونٹس خارج کرتے ہیں۔

## 2. Norito ساختیں

### 2.1 NombreRegistroV1| فیلڈ | قسم | نوٹس |
|-------|------|-------|
| `suffix_id` | `u16` | `SuffixPolicyV1` کی ریفرنس۔ |
| `selector` | `NameSelectorV1` | auditoría/depuración کے لیے bytes del selector sin formato۔ |
| `name_hash` | `[u8; 32]` | mapas/eventos کے لیے tecla۔ |
| `normalized_label` | `AsciiString` | انسانی قابلِ پڑھائی لیبل (Norma v1 کے بعد)۔ |
| `display_label` | `AsciiString` | mayordomo کی tripa؛ اختیاری cosméticos۔ |
| `owner` | `AccountId` | renovaciones/traspasos کنٹرول کرتا ہے۔ |
| `controllers` | `Vec<NameControllerV1>` | اکاؤنٹ ایڈریسز، solucionadores یا ایپ metadatos کے حوالہ جات۔ |
| `status` | `NameStatus` | لائف سائیکل فلیگ (سیکشن 4 دیکھیں)۔ |
| `pricing_class` | `u8` | sufijo کے niveles de precios کا índice (estándar, premium, reservado)۔ |
| `registered_at` | `Timestamp` | Activación de ابتدائی کا بلاک ٹائم۔ |
| `expires_at` | `Timestamp` | ادا شدہ مدت کا اختام۔ |
| `grace_expires_at` | `Timestamp` | gracia de renovación automática کا اختتام (predeterminado +30 días) ۔ |
| `redemption_expires_at` | `Timestamp` | ventana de canje کا اختتام (predeterminado +60 دن)۔ |
| `auction` | `Option<NameAuctionStateV1>` | Los holandeses reabren las subastas premium کی صورت میں موجود۔ |
| `last_tx_hash` | `Hash` | اس ورژن کو پیدا کرنے والی ٹرانزیکشن کا puntero determinista۔ |
| `metadata` | `Metadata` | registrador کی metadatos arbitrarios (registros de texto, pruebas) ۔ |

Otras estructuras:

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

### 2.2 Política de sufijo V1| فیلڈ | قسم | نوٹس |
|-------|------|-------|
| `suffix_id` | `u16` | بنیادی clave؛ پالیسی ورژنز میں مستحکم۔ |
| `suffix` | `AsciiString` | مثال کے طور پر `sora`۔ |
| `steward` | `AccountId` | carta de gobernanza میں متعین administrador۔ |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | identificador de activo de liquidación predeterminado (مثلا `61CtjvNd9T3THAR65GsMVHr82Bjc`)۔ |
| `pricing` | `Vec<PriceTierV1>` | قیمت کے niveles کے coeficientes اور مدت کے قواعد۔ |
| `min_term_years` | `u8` | خریدی گئی مدت کے لیے کم از کم حد۔ |
| `grace_period_days` | `u16` | Predeterminado 30. |
| `redemption_period_days` | `u16` | Predeterminado 60. |
| `max_term_years` | `u8` | پیشگی تجدید کی زیادہ سے زیادہ مدت۔ |
| `referral_cap_bps` | `u16` | <=1000 (10%) carta کے مطابق۔ |
| `reserved_labels` | `Vec<ReservedNameV1>` | gobernanza کی فراہم کردہ فہرست مع asignar ہدایات۔ |
| `fee_split` | `SuffixFeeSplitV1` | tesorería / administrador / referencia حصص (puntos básicos) ۔ |
| `fund_splitter_account` | `AccountId` | depósito de garantía رکھنے اور فنڈز تقسیم کرنے والا اکاؤنٹ۔ |
| `policy_version` | `u16` | ہر تبدیلی پر بڑھتا ہے۔ |
| `metadata` | `Metadata` | توسیعی نوٹس (pacto de KPI, hashes de documentos de cumplimiento) ۔ |

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

### 2.3 ریونیو اور asentamiento ریکارڈز| Estructura | فیلڈز | مقصد |
|--------|-------|------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | época de asentamiento (ہفتہ وار) کے حساب سے enrutado ادائیگیوں کا determinista ریکارڈ۔ |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | ہر ادائیگی پوسٹ ہونے پر emitir (registro, renovación, subasta)۔ |

تمام `TokenValue` فیلڈز Norito کی codificación canónica de punto fijo استعمال کرتی ہیں اور کرنسی کوڈ متعلقہ `SuffixPolicyV1` میں declarar ہوتا ہے۔

### 2.4 رجسٹری ایونٹس

Eventos canónicos Automatización de DNS/puerta de enlace Análisis y registro de reproducción Registro de reproducción

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

ایونٹس کو registro reproducible (dominio `RegistryEvents`) میں anexar کرنا ضروری ہے اور feeds de puerta de enlace میں mirror کرنا ضروری ہے تاکہ Cachés DNS SLA کے اندر invalidar ہوں۔

## 3. Diseño de almacenamiento e índices

| Clave | وضاحت |
|-----|-------------|
| `Names::<name_hash>` | `name_hash` سے `NameRecordV1` تک بنیادی mapa۔ |
| `NamesByOwner::<AccountId, suffix_id>` | billetera UI کے لیے ثانوی índice (paginación amigable) ۔ |
| `NamesByLabel::<suffix_id, normalized_label>` | detección de conflictos کرتا ہے اور búsqueda determinista فعال بناتا ہے۔ |
| `SuffixPolicies::<suffix_id>` | تازہ ترین `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` ہسٹری۔ |
| `RegistryEvents::<u64>` | registro de solo agregar جس کی secuencia de teclas monótona ہے۔ |تمام claves Norito tuplas کے ساتھ serializar ہوتی ہیں تاکہ hosts کے درمیان hash determinista رہے۔ actualizaciones de índice بنیادی ریکارڈ کے ساتھ atómicamente ہوتی ہیں۔

## 4. Máquina de estado del ciclo de vida

| Estado | Condiciones de entrada | Transiciones permitidas | Notas |
|-------|------------------|--------------------|-------|
| Disponible | جب `NameRecord` Hombre de ہو۔ | `PendingAuction` (premium), `Active` (registro estándar). | búsqueda de disponibilidad صرف índices پڑھتی ہے۔ |
| Subasta Pendiente | جب `PriceTierV1.auction_kind` != ninguno ہو۔ | `Active` (se liquida la subasta), `Tombstoned` (sin ofertas). | subastas `AuctionOpened` اور `AuctionSettled` emiten کرتی ہیں۔ |
| Activo | رجسٹریشن یا تجدید کامیاب ہو۔ | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` transición چلاتا ہے۔ |
| Período de Gracia | جب `now > expires_at` ہو۔ | `Active` (renovación puntual), `Redemption`, `Tombstoned`. | Predeterminado +30 días; resolver ہوتا ہے مگر bandera ہوتا ہے۔ |
| Redención | `now > grace_expires_at` en lugar de `< redemption_expires_at`. | `Active` (renovación tardía), `Tombstoned`. | کمانڈز پر penalización درکار ہے۔ |
| Congelado | gobernanza یا guardián congelado۔ | `Active` (remediación کے بعد), `Tombstoned`. | transferir یا controladores actualizar نہیں کر سکتے۔ |
| Lápida | رضاکارانہ rendición, مستقل disputa نتیجہ، یا redención ختم۔ | `PendingAuction` (reapertura holandesa) یا desechado رہتا ہے۔ | `NameTombstoned` ایونٹ میں وجہ شامل ہونی چاہیے۔ |Las transiciones de estado لازمی طور پر متعلقہ `RegistryEventKind` emiten کریں تاکہ cachés posteriores ہم آہنگ رہیں۔ Tombstoned نام جو Dutch reabre subastas میں داخل ہوں وہ `AuctionKind::DutchReopen` carga útil شامل کرتے ہیں۔

## 5. Eventos canónicos y sincronización de puerta de enlace

Puertas de enlace `RegistryEventV1` Configuración de sincronización y DNS/SoraFS Configuración de sincronización:

1. Secuencia ایونٹ میں حوالہ کردہ تازہ ترین `NameRecordV1` حاصل کریں۔
2. plantillas de resolución دوبارہ بنائیں (i105 ترجیحی + comprimido (`sora`) segunda mejor dirección, registros de texto) ۔
3. [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) میں بیان کردہ Flujo de trabajo SoraDNS کے ذریعے pin de datos de zona کریں۔

Garantías de entrega de eventos:

- ہر ٹرانزیکشن جو `NameRecordV1` پر اثر ڈالے *لازمی* طور پر `version` کے ساتھ صرف ایک ایونٹ شامل کرے جو سختی سے بڑھتی ہو۔
- `RevenueSharePosted` ایونٹس `RevenueShareRecordV1` سے asentamientos emitidos کو referencia کرتے ہیں۔
- congelar/descongelar/lápida ایونٹس repetición de auditoría کے لیے `metadata` میں hashes de artefactos de gobernanza شامل کرتے ہیں۔

## 6. Cargas útiles Norito کی مثالیں

### 6.1 Registro de nombre مثال

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

### 6.2 Política de sufijos

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

## 7. اگلے اقدامات- **SN-2b (API de registrador y ganchos de gobernanza):** Las estructuras Torii exponen la información (Norito con enlaces JSON) y las comprobaciones de admisión y los artefactos de gobernanza.
- **SN-3 (motor de subasta y registro):** confirmar/revelar Lógica de reapertura holandesa کے لیے `NameAuctionStateV1` دوبارہ استعمال کریں۔
- **SN-5 (Pago y liquidación):** مالی conciliación اور رپورٹنگ automatización کے لیے `RevenueShareRecordV1` استعمال کریں۔

سوالات یا تبدیلی کی درخواستیں `roadmap.md` میں SNS اپ ڈیٹس کے ساتھ درج کریں اور merge کے وقت `status.md` میں شامل کریں۔