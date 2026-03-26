---
lang: he
direction: rtl
source: docs/portal/docs/sns/registry-schema.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::שימו לב פואנטה קנוניקה
Esta page refleja `docs/source/sns/registry_schema.md` y ahora sirve como la copia canonica del portal. El archivo fuente se mantiene para actualizaciones de traduccion.
:::

# Esquema del registro del Sora Name Service (SN-2a)

**Estado:** Redactado 2026-03-24 -- עדכון עדכון לתוכנית SNS  
**Enlace del Roadmap:** SN-2a "סכימת רישום ופריסת אחסון"  
**Alcance:** Definir las estructuras Norito canonicas, los estados de ciclo de vida y los eventos emitidos para el Sora Name Service (SNS) de modo que las implementaciones de registro y registrar se mantengan deterministas and contratos, SDK.

ראה את התעודה המפורטת של הסקירה עבור SN-2a באופן ספציפי:

1. Identificadores y reglas de hashing (`SuffixId`, `NameHash`, גזירת בוררים).
2. מבנים/מבנים Norito עבור רשומות שמות, פוליטיקאות סופיות, שכבות מחיר, רשימות רשומות ואירועי רישום.
3. Layout de almacenamiento y prefijos de indices para replay determinista.
4. Una maquina de estados que cubre registro, renovacion, gracia/redencion, מקפיא מצבות.
5. Eventos canonicos consumidos por la automatizacion DNS/gateway.

## 1. זיהוי הגיבוב

| מזהה | תיאור | גזירה |
|------------|----------------|-------------|
| `SuffixId` (`u16`) | זיהוי רישום עבור סופריור ניבל (`.sora`, `.nexus`, `.dao`). Alineado con el catalogo de sufijos en [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Asignado por voto de gobernanza; almacenado en `SuffixPolicyV1`. |
| `SuffixSelector` | Forma canonica en string del sufijo (ASCII, אותיות קטנות). | דוגמה: `.sora` -> `sora`. |
| `NameSelectorV1` | בורר binario para la etiqueta registrada. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. כללי ההתנהגות של NFC + אותיות קטנות, Norm v1. |
| `NameHash` (`[u8;32]`) | Clave primaria de busqueda usada por contratos, eventos y caches. | `blake3(NameSelectorV1_bytes)`. |

דרישות קבע:

- כללי כללי נורמל דרך Norm v1 (UTS-46 strict, STD3 ASCII, NFC). Las cadenas de usuario DEBEN normalizarse antes del hash.
- Las etiquetas reservadas (de `SuffixPolicyV1.reserved_labels`) nunca entran en el registro; los מבטל את solo de gobernanza emiten eventos `ReservedNameAssigned`.

## 2. Estructuras Norito

### 2.1 NameRecordV1| קמפו | טיפו | Notas |
|-------|------|-------|
| `suffix_id` | `u16` | Referencia `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | בורר בתים בעיבוד עבור אודיטוריה/ניפוי באגים. |
| `name_hash` | `[u8; 32]` | Clave עבור מפות/אירועים. |
| `normalized_label` | `AsciiString` | כללי התנהגות קריאים לאדם (פוסט נורמה v1). |
| `display_label` | `AsciiString` | מארז פרוvisto por steward; קוסמטיקה אופציונלית. |
| `owner` | `AccountId` | Controla renovaciones/transferencias. |
| `controllers` | `Vec<NameControllerV1>` | הפניות להנחיות למטרות, פותרות או מטא נתונים של יישום. |
| `status` | `NameStatus` | Bandera de ciclo de vida (Ver Seccion 4). |
| `pricing_class` | `u8` | Indice en tiers de precios del sufijo (סטנדרטי, פרימיום, שמור). |
| `registered_at` | `Timestamp` | חותמת זמן של גוש הפעלה ראשוני. |
| `expires_at` | `Timestamp` | פגדה פינ דל טרמינו. |
| `grace_expires_at` | `Timestamp` | Fin de gracia de auto-renovacion (ברירת מחדל +30 dias). |
| `redemption_expires_at` | `Timestamp` | Fin de ventana de redencion (ברירת מחדל +60 dias). |
| `auction` | `Option<NameAuctionStateV1>` | הצג את ההולנדית או את הפרימיום הבא. |
| `last_tx_hash` | `Hash` | Puntero determinista a la transaccion que produjo esta גרסה. |
| `metadata` | `Metadata` | מטא נתונים שרירותית של הרשם (רשומות טקסט, הוכחות). |

מבני ספורט:

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

| קמפו | טיפו | Notas |
|-------|------|-------|
| `suffix_id` | `u16` | Clave primaria; סטable entre versiones de politica. |
| `suffix` | `AsciiString` | לדוגמא, `sora`. |
| `steward` | `AccountId` | דייל definido en el charter de gobernanza. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Identificador de Activo de Settlement por defecto (por ejemplo `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | Coeficientes de precios por tiers y regglas de duracion. |
| `min_term_years` | `u8` | Piso para el termino comprado sin importar עוקף את ה-tier. |
| `grace_period_days` | `u16` | ברירת מחדל 30. |
| `redemption_period_days` | `u16` | ברירת מחדל 60. |
| `max_term_years` | `u8` | Maximo de renovacion por adelantado. |
| `referral_cap_bps` | `u16` | <=1000 (10%) סגן אל צ'רטר. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Lista suministrada por gobernanza con instrucciones de assignacion. |
| `fee_split` | `SuffixFeeSplitV1` | Porciones de tesoreria / דייל / הפניה (נקודות בסיס). |
| `fund_splitter_account` | `AccountId` | פקדון מתנה + חלוקת פנקסים. |
| `policy_version` | `u16` | Incrementa en cada cambio. |
| `metadata` | `Metadata` | Notas extendidas (ברית KPI, hashes de documentos de cumplimiento). |

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

### 2.3 Registros de ingresos y הסדר| מבנה | קמפוס | פרופוזיטו |
|--------|--------|--------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, Norito, Norito. | Registro determinista de pagos enroutados por Epoca de Settlement (Semanal). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Emitido cada vez que un pago se registra (registro, renovacion, subasta). |

Todos los campos `TokenValue` usan la codificacion fija canonica de Norito con el codigo de moneda declarado en el `SuffixPolicyV1` asociado.

### 2.4 אירועים ברישום

אירועי קנוניקוס הוכחו עם יומן שידור חוזר עבור אוטומטיזציה של DNS/שער ואנליטיקה.

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

Los eventos deben agregarse un log reproducible (por ejemplo, el dominio `RegistryEvents`) y reflejarse en feeds de gateway para que las caches DNS invaliden dentro del SLA.

## 3. פריסה של מדדי e almacenamiento

| קלאב | תיאור |
|-----|-------------|
| `Names::<name_hash>` | מפה ראשונית של `name_hash` ו-`NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | מדד שני עבור ממשק משתמש לארנק (ניתן להתאים לדף). |
| `NamesByLabel::<suffix_id, normalized_label>` | Detecta conflictos, habilita busqueda determinista. |
| `SuffixPolicies::<suffix_id>` | Ultimo `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | היסטוריית דה `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | יומן הוספה בלבד con clave de secuencia monotonica. |

Todas las claves se serializan usando tuplas Norito para mantener el hashing determinista entre hosts. Las actualizaciones de indices ocurren de forma atomica junto con el registro primario.

## 4. Maquina de estados del ciclo de vida

| Estado | Condiciones de entrada | Transiciones permitidas | Notas |
|-------|------------------------|------------------------|------|
| זמין | Derivado cuando `NameRecord` esta ausente. | `PendingAuction` (פרימיום), `Active` (registro estandar). | La busqueda de disponibilidad לי סולו מדדי. |
| מכירה פומבית בהמתנה | Creado cuando `PriceTierV1.auction_kind` != אין. | `Active` (la subasta se liquida), `Tombstoned` (sin pujas). | Las subastas emiten `AuctionOpened` y `AuctionSettled`. |
| פעיל | רישום או יציאה לשיפוץ. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` impulsa la transicion. |
| תקופת חסד | אוטומטי cuando `now > expires_at`. | `Active` (שיפוץ לזמן), `Redemption`, `Tombstoned`. | ברירת מחדל +30 dias; aun resuelve pero marcado. |
| גאולה | `now > grace_expires_at` לעומת `< redemption_expires_at`. | `Active` (שיפוץ טרדיה), `Tombstoned`. | Los comandos דורש תשלום דה penalidad. |
| קפוא | להקפיא את הגוברננסה או האפוטרופוס. | `Active` (תיקון אשפה), `Tombstoned`. | אין העברת בקרים בפועל. |
| מצבה | Rendicion voluntaria, resultado de disputa permanente, o redencion expirada. | `PendingAuction` (פתיחה מחודשת בהולנד) או מצבה קבועה. | El evento `NameTombstoned` debe כולל razon. |Las transiciones de estado DEBEN emitir el correspondiente `RegistryEventKind` para que las caches downstream se mantengan coherentes. Los nombres tombstoned que entran en subastas הולנדית פתיחה מחדש adjuntan un מטען `AuctionKind::DutchReopen`.

## 5. Eventos canonicos y sync de gateways

שערי שערים נרשמו ב-`RegistryEventV1` ובאמצעות DNS/SoraFS:

1. Obtener el ultimo `NameRecordV1` referenciado por la secuencia de eventos.
2. Regenererar templates de resolver (directions i105 preferidas + דחוס (`sora`) como segunda opcion, רשומות טקסט).
3. Pnear datas de zona actualizados via el flujo SoraDNS descrito en [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Garantias de entrega de eventos:

- Cada transaccion que afecta un `NameRecordV1` *debe* agregar exactamente un evento con `version` estrictamente creciente.
- Los eventos `RevenueSharePosted` referencian liquidaciones emitidas por `RevenueShareRecordV1`.
- אירועי ההקפאה/ביטול ההקפאה/מצבה כוללות חפצי אמנות של גוברננזה דנטרו דה `metadata` למשחק חוזר של אודיטוריה.

## 6. דוגמה של מטענים Norito

### 6.1 דוגמה של NameRecord

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "<i105-account-id>",
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

### 6.2 דוגמה למדיניות סיומת

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "<i105-account-id>",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("<i105-account-id>"), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "<i105-account-id>",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. Proximos pasos

- **SN-2b (Registrar API & Governance Hooks):** מבני הסברה באמצעות Torii (כריכות Norito ו-JSON) y conectar בדיקות קבלה של artefactos de gobernanza.
- **SN-3 (מנוע מכירות פומביות ורישום):** שימוש מחדש ב-`NameAuctionStateV1` עבור יישום היגיון של התחייבות/חשיפה ופתיחה מחדש בהולנדית.
- **SN-5 (תשלום וסילוק):** aprovechar `RevenueShareRecordV1` para reconciliacion financiera y automatizacion de reportes.

Las preguntas o solicitudes de cambio deben registrarse junto con las actualizaciones del roadmap de SNS en `roadmap.md` y reflejarse en `status.md` cuando se integren.