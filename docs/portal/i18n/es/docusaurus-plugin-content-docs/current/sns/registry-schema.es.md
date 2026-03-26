---
lang: es
direction: ltr
source: docs/portal/docs/sns/registry-schema.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Esta pagina refleja `docs/source/sns/registry_schema.md` y ahora sirve como la copia canónica del portal. El archivo fuente se mantiene para actualizaciones de traducción.
:::

# Esquema del registro del Sora Name Service (SN-2a)

**Estado:** Redactado 2026-03-24 -- enviado una revisión del programa SNS  
**Enlace del roadmap:** SN-2a "Esquema de registro y diseño de almacenamiento"  
**Alcance:** Definir las estructuras Norito canónicas, los estados de ciclo de vida y los eventos emitidos para el Sora Name Service (SNS) de modo que las implementaciones de registro y registrador se mantengan deterministas en contratos, SDK y gateways.

Este documento completa el entregable de esquema para SN-2a al especificar:

1. Identificadores y reglas de hash (`SuffixId`, `NameHash`, derivacion de selectores).
2. Structs/enums Norito para registros de nombres, políticas de sufijos, niveles de precios, repartos de ingresos y eventos del registro.
3. Diseño de almacenamiento y prefijos de índices para reproducción determinista.
4. Una maquina de estados que cubre registro, renovacion, gracia/redencion,congela y tombstones.
5. Eventos canónicos consumidos por la automatización DNS/gateway.

## 1. Identificadores y hash| Identificador | Descripción | Derivación |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Identificador del registro para sufijos de nivel superior (`.sora`, `.nexus`, `.dao`). Alineado con el catalogo de sufijos en [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Asignado por voto de gobernanza; almacenado en `SuffixPolicyV1`. |
| `SuffixSelector` | Forma canónica en cadena del sufijo (ASCII, minúscula). | Ejemplo: `.sora` -> `sora`. |
| `NameSelectorV1` | Selector binario para la etiqueta registrada. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. La etiqueta es NFC + minúsculas según Norma v1. |
| `NameHash` (`[u8;32]`) | Clave primaria de busqueda usada por contratos, eventos y cachés. | `blake3(NameSelectorV1_bytes)`. |

Requisitos de determinismo:

- Las etiquetas se normalizan vía Norm v1 (UTS-46 estricto, STD3 ASCII, NFC). Las cadenas de usuario DEBEN normalizarse antes del hash.
- Las etiquetas reservadas (de `SuffixPolicyV1.reserved_labels`) nunca ingresan en el registro; los overrides solo de gobernanza emiten eventos `ReservedNameAssigned`.

## 2. Estructuras Norito

### 2.1 NombreRegistroV1| Campo | Tipo | Notas |
|-------|------|-------|
| `suffix_id` | `u16` | Referencia `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Bytes de selector sin procesar para auditoría/depuración. |
| `name_hash` | `[u8; 32]` | Clave para mapas/eventos. |
| `normalized_label` | `AsciiString` | Etiqueta legible por humanos (post Norma v1). |
| `display_label` | `AsciiString` | Carcasa provista por mayordomo; cosmética opcional. |
| `owner` | `AccountId` | Controla renovaciones/transferencias. |
| `controllers` | `Vec<NameControllerV1>` | Referencias a direcciones de cuenta objetivo, resolutores o metadatos de aplicación. |
| `status` | `NameStatus` | Bandera de ciclo de vida (ver Sección 4). |
| `pricing_class` | `u8` | Índice en niveles de precios del sufijo (estándar, premium, reservado). |
| `registered_at` | `Timestamp` | Marca de tiempo del bloque de la activación inicial. |
| `expires_at` | `Timestamp` | Fin del termino pagado. |
| `grace_expires_at` | `Timestamp` | Fin de gracia de auto-renovación (predeterminado +30 días). |
| `redemption_expires_at` | `Timestamp` | Fin de ventana de redencion (predeterminado +60 días). |
| `auction` | `Option<NameAuctionStateV1>` | Presente cuando se reabre Dutch o subastas premium están activadas. |
| `last_tx_hash` | `Hash` | Puntero determinista a la transacción que produjo esta versión. || `metadata` | `Metadata` | Metadatos arbitraria del registrador (registros de texto, pruebas). |

Estructuras de soporte:

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

### 2.2 Política de sufijo V1| Campo | Tipo | Notas |
|-------|------|-------|
| `suffix_id` | `u16` | Clave primaria; estable entre versiones de política. |
| `suffix` | `AsciiString` | por ejemplo, `sora`. |
| `steward` | `AccountId` | Steward definido en la carta de gobernanza. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Identificador de activo de asentamiento por defecto (por ejemplo `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | Coeficientes de precios por niveles y reglas de duración. |
| `min_term_years` | `u8` | Piso para el término comprado sin importar anulaciones de nivel. |
| `grace_period_days` | `u16` | Predeterminado 30. |
| `redemption_period_days` | `u16` | Predeterminado 60. |
| `max_term_years` | `u8` | Máximo de renovación por adelantado. |
| `referral_cap_bps` | `u16` | <=1000 (10%) según el charter. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Lista suministrada por gobernanza con instrucciones de asignación. |
| `fee_split` | `SuffixFeeSplitV1` | Porciones de tesoreria / azafata / remisión (puntos básicos). |
| `fund_splitter_account` | `AccountId` | Cuenta que mantiene escrow + distribuye fondos. |
| `policy_version` | `u16` | Incrementa en cada cambio. |
| `metadata` | `Metadata` | Notas extendidas (KPI covenant, hashes de documentos de cumplimiento). |```text
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

### 2.3 Registros de ingresos y liquidación

| Estructura | Campos | propuesta |
|--------|--------|---------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Registro determinista de pagos encaminados por época de liquidación (semanal). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Emitido cada vez que un pago se registra (registro, renovación, subasta). |

Todos los campos `TokenValue` usan la codificación fija canónica de Norito con el código de moneda declarado en el `SuffixPolicyV1` asociado.

### 2.4 Eventos del registro

Los eventos canónicos proporcionan un registro de reproducción para automatización DNS/gateway y analíticas.

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

Los eventos deben agregarse a un log reproducible (por ejemplo, el dominio `RegistryEvents`) y reflejarse en feeds de gateway para que las cachés DNS invaliden dentro del SLA.

## 3. Diseño de almacenamiento e índices| Clave | Descripción |
|-----|-------------|
| `Names::<name_hash>` | Mapa primario de `name_hash` a `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Índice secundario para UI de wallet (paginacion amigable). |
| `NamesByLabel::<suffix_id, normalized_label>` | Detecta conflictos, habilita búsqueda determinista. |
| `SuffixPolicies::<suffix_id>` | Último `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Historial de `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Registro de solo adición con clave de secuencia monotónica. |

Todas las claves se serializan usando tuplas Norito para mantener el hash determinista entre hosts. Las actualizaciones de índices ocurren de forma atómica junto con el registro primario.

## 4. Máquina de estados del ciclo de vida| Estado | Condiciones de entrada | Transiciones permitidas | Notas |
|-------|------------------------|-------------------------|-------|
| Disponible | Derivado cuando `NameRecord` esta ausente. | `PendingAuction` (premium), `Active` (registro estándar). | La busqueda de disponibilidad lee solo indices. |
| Subasta Pendiente | Creado cuando `PriceTierV1.auction_kind` != none. | `Active` (la subasta se liquida), `Tombstoned` (sin pujas). | Las subastas emiten `AuctionOpened` y `AuctionSettled`. |
| Activo | Registro de renovación exitosa. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` impulsa la transición. |
| Período de Gracia | Automático cuando `now > expires_at`. | `Active` (renovación a tiempo), `Redemption`, `Tombstoned`. | Predeterminado +30 días; aun resuelve pero marcado. |
| Redención | `now > grace_expires_at` pero `< redemption_expires_at`. | `Active` (renovacion tardia), `Tombstoned`. | Los comandos requieren tarifa de penalidad. |
| Congelado | Freeze de gobernanza o guardián. | `Active` (tras remediacion), `Tombstoned`. | No se pueden transferir ni actualizar controladores. |
| Lápida | Rendición voluntaria, resultado de disputa permanente, o redención expirada. | `PendingAuction` (reapertura holandesa) o permanece tombstoneed. | El evento `NameTombstoned` debe incluir razón. |Las transiciones de estado DEBEN emiten el correspondiente `RegistryEventKind` para que las cachés aguas abajo se mantengan coherentes. Los nombres tombstoned que entran en subastas holandesas reabren adjuntan un payload `AuctionKind::DutchReopen`.

## 5. Eventos canónicos y sincronización de gateways

Los gateways se suscriben a `RegistryEventV1` y sincronizan a DNS/SoraFS mediante:

1. Obtener el último `NameRecordV1` referenciado por la secuencia de eventos.
2. Regenerar plantillas de resolución (direcciones i105 preferidas + comprimidas (`sora`) como segunda opción, registros de texto).
3. Pinnear datos de zona actualizados vía el flujo SoraDNS descrito en [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Garantías de entrega de eventos:

- Cada transacción que afecta un `NameRecordV1` *debe* agregar exactamente un evento con `version` estrictamente creciente.
- Los eventos `RevenueSharePosted` referencian liquidaciones emitidas por `RevenueShareRecordV1`.
- Los eventos de congelación/descongelación/tombstone incluyen hashes de artefactos de gobernanza dentro de `metadata` para repetición de audiencias.

## 6. Ejemplos de cargas útiles Norito

### 6.1 Ejemplo de NameRecord

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

### 6.2 Ejemplo de SuffixPolicy

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

## 7. Proximos pasos- **SN-2b (API de registro y ganchos de gobernanza):** exponer estas estructuras a través de Torii (bindings Norito y JSON) y conectar comprobaciones de admisión a artefactos de gobernanza.
- **SN-3 (motor de subasta y registro):** reutilizar `NameAuctionStateV1` para implementar la lógica de confirmación/revelación y reapertura holandesa.
- **SN-5 (Pago y liquidación):** aprovecha `RevenueShareRecordV1` para reconciliación financiera y automatización de reportes.

Las preguntas o solicitudes de cambio deben registrarse junto con las actualizaciones del roadmap de SNS en `roadmap.md` y reflejarse en `status.md` cuando se integran.