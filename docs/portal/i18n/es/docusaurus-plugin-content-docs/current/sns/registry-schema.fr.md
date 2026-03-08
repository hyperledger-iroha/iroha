---
lang: es
direction: ltr
source: docs/portal/docs/sns/registry-schema.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Esta página refleja `docs/source/sns/registry_schema.md` y es una copia desordenada del portal. Le fichier source reste pour les mises a jour de traduction.
:::

# Esquema de registro del servicio de nombres de Sora (SN-2a)

**Estado:** Redige 2026-03-24 -- soumis a la revue du program SNS  
**Hoja de ruta del gravamen:** SN-2a "Esquema de registro y diseño de almacenamiento"  
**Portee:** Definir las estructuras Norito canónicas, los estados de ciclo de vida y los eventos emitidos para el Servicio de nombres de Sora (SNS) después de que las implementaciones de registro y registro estén determinadas en los contratos, SDK y puertas de enlace.

Este documento completa el esquema de alojamiento para SN-2a en particular:

1. Identificadores y reglas de hash (`SuffixId`, `NameHash`, derivación de selectores).
2. Estructuras/enums Norito para registros de nombres, políticas de sufijos, niveles de precios, reparto de ingresos y eventos de registro.
3. Diseño de almacenamiento y prefijos de índices para determinar la reproducción.
4. Une machine d'etats couvrant l'enregistrement, le renouvellement, gracia/redención, congelaciones y lápidas.
5. Eventos canónicos relacionados con la automatización DNS/gateway.

## 1. Identificadores y hash| Identificador | Descripción | Derivación |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Identificador de registro para los sufijos de primer nivel (`.sora`, `.nexus`, `.dao`). Alinee el catálogo de sufijos en [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Atribue par vote de gobernance; almacenado en `SuffixPolicyV1`. |
| `SuffixSelector` | Forma canónica en cadena de sufijo (ASCII, minúscula). | Ejemplo: `.sora` -> `sora`. |
| `NameSelectorV1` | Selector binario para registrar la etiqueta. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. La etiqueta es NFC + minúsculas según Norma v1. |
| `NameHash` (`[u8;32]`) | Cle primaire de recherche utilisee par contrats,evenements et caches. | `blake3(NameSelectorV1_bytes)`. |

Exigencias de determinismo:

- Las etiquetas se normalizan mediante la norma v1 (UTS-46 estricta, STD3 ASCII, NFC). Las cadenas utilizadas por DOIVENT están normalizadas antes del hash.
- Les etiquetas reservadas (de `SuffixPolicyV1.reserved_labels`) n'entrent jamais dans le registre; Las anulaciones de la gobernanza única provocadas por eventos `ReservedNameAssigned`.

## 2. Estructuras Norito

### 2.1 NombreRegistroV1| Campeón | Tipo | Notas |
|-------|------|-------|
| `suffix_id` | `u16` | Referencia `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Octetos del selector bruto para auditoría/depuración. |
| `name_hash` | `[u8; 32]` | Cle pour mapas/eventos. |
| `normalized_label` | `AsciiString` | Etiqueta lisible pour l'humain (posterior a la norma v1). |
| `display_label` | `AsciiString` | Casing fourni par le steward; Opción cosmética. |
| `owner` | `AccountId` | Controle les renouvellements/transfers. |
| `controllers` | `Vec<NameControllerV1>` | Referencias a direcciones de compte cibles, resolutores o metadatos de aplicación. |
| `status` | `NameStatus` | Indicador de ciclo de vida (ver Sección 4). |
| `pricing_class` | `u8` | Índice en los niveles de precio del sufijo (estándar, premium, reservado). |
| `registered_at` | `Timestamp` | Marca de tiempo del bloque de activación inicial. |
| `expires_at` | `Timestamp` | Fin du terme paye. |
| `grace_expires_at` | `Timestamp` | Fin de gracia de renovación automática (predeterminado +30 días). |
| `redemption_expires_at` | `Timestamp` | Fin de la fenetre de redemption (predeterminado +60 días). |
| `auction` | `Option<NameAuctionStateV1>` | Presente cuando los holandeses reabren o encheres premium sont actives. |
| `last_tx_hash` | `Hash` | Pointeur deterministe vers la transacción qui a produit esta versión. || `metadata` | `Metadata` | Metadata arbitraire du registrador (registros de texto, pruebas). |

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

### 2.2 Política de sufijo V1| Campeón | Tipo | Notas |
|-------|------|-------|
| `suffix_id` | `u16` | Cle primaire; estable entre versiones de política. |
| `suffix` | `AsciiString` | por ejemplo, `sora`. |
| `steward` | `AccountId` | Steward defini dans le charter de gouvernance. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Identificador de activo de liquidación por defecto (por ejemplo `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | Coeficientes de precio entre partes y reglas de duración. |
| `min_term_years` | `u8` | Planifique para eliminar los términos que anulan el nivel. |
| `grace_period_days` | `u16` | Predeterminado 30. |
| `redemption_period_days` | `u16` | Predeterminado 60. |
| `max_term_years` | `u8` | Máxima anticipación de renovación. |
| `referral_cap_bps` | `u16` | <=1000 (10%) según el alquiler. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Liste fournie par la gouvernance avec instrucciones de afectación. |
| `fee_split` | `SuffixFeeSplitV1` | Partes tesorería / azafata / remisión (puntos básicos). |
| `fund_splitter_account` | `AccountId` | Compte qui detient l'escrow + distribue les fonds. |
| `policy_version` | `u16` | Incrementa cada cambio. |
| `metadata` | `Metadata` | Notas etendues (pacto de KPI, hashes de documentos de cumplimiento). |

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
```### 2.3 Registros de ingresos y liquidación

| Estructura | Campeones | Pero |
|--------|--------|-----|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Registro determinante de las rutas de pago por época de liquidación (hebdomadaire). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Emis a chaque paiement poste (enregistrement, renouvellement, enchere). |

Todos los campeones `TokenValue` utilizan la codificación fija canónica de Norito con el código diseñado en la asociación `SuffixPolicyV1`.

### 2.4 Eventos de registro

Los eventos canónicos incluyen un registro de reproducción para la automatización de DNS/gateway y el análisis.

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

Los eventos deben incluir un registro que se puede actualizar (por ejemplo, el dominio `RegistryEvents`) y se refleja en la puerta de enlace de feeds para que los cachés DNS no sean válidos en el SLA.

## 3. Diseño de stockage e índices| Cle | Descripción |
|-----|-------------|
| `Names::<name_hash>` | Mapa primario de `name_hash` a `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Índice secundario para billetera UI (amigable con la paginación). |
| `NamesByLabel::<suffix_id, normalized_label>` | Detecte les conflits, alimente la recherche deterministe. |
| `SuffixPolicies::<suffix_id>` | Último `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Histórico `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Registrar secuencia de par de archivos de solo agregar monótono. |

Todas las claves se serializan a través de las tuplas Norito para guardar un hash determinante entre los hoteles. Les mises a jour d'index se font atomiquement avec l'enregistrement principal.

## 4. Máquina de estados del ciclo de vida| Estado | Condiciones de entrada | Permisos de transiciones | Notas |
|-------|--------------------|---------------------|-------|
| Disponible | Deriva cuando `NameRecord` está ausente. | `PendingAuction` (premium), `Active` (estándar de registro). | La búsqueda de disponibilidad se realiza únicamente en los índices. |
| Subasta Pendiente | Cree cuando `PriceTierV1.auction_kind` != ninguno. | `Active` (enchere reglee), `Tombstoned` (aucune enchere). | Les encheres emetttent `AuctionOpened` et `AuctionSettled`. |
| Activo | Registro o renovación reussi. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` pilote la transición. |
| Período de Gracia | Automático cuando `now > expires_at`. | `Active` (renovación temporal), `Redemption`, `Tombstoned`. | Predeterminado +30 días; resolución más marca. |
| Redención | `now > grace_expires_at` más `< redemption_expires_at`. | `Active` (renovación tardía), `Tombstoned`. | Les commandes exigent des frais de penalité. |
| Congelado | Freeze de gouvernance ou guardian. | `Active` (después de la corrección), `Tombstoned`. | Ne peut pas transferer ni mettre a jour les controladores. |
| Lápida | El abandono voluntario, resultado de litigio permanente o vencimiento del rescate. | `PendingAuction` (reapertura holandesa) o resto desechado. | El evento `NameTombstoned` debe incluir una razón. |Las transiciones de estado DOIVENT emmettre le `RegistryEventKind` correspondiente para que los cachés posteriores sean coherentes. Les noms tombstoned entrante en encheres Dutch reabre adjunto una carga útil `AuctionKind::DutchReopen`.

## 5. Eventos canónicos y puerta de enlace de sincronización

Las puertas de enlace se conectan a `RegistryEventV1` y sincronizan DNS/SoraFS a través de:

1. Recuperar la última referencia `NameRecordV1` por la secuencia de acontecimientos.
2. Regenerar las plantillas de resolución (direcciones preferidas de IH58 + comprimido (`sora`) en segunda opción, registros de texto).
3. Inserte los donnees de zona perdidos un día a través del flujo de trabajo SoraDNS escrito en [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Garantías de vida de eventos:

- Cada transacción que afecta un `NameRecordV1` *doit* agrega exactamente un evento con `version` estrictamente croissante.
- Los eventos `RevenueSharePosted` hacen referencia a los acuerdos emitidos por `RevenueShareRecordV1`.
- Los eventos de congelación/descongelación/lápida incluyen hashes de artefactos de gobierno en `metadata` para la reproducción de auditoría.

## 6. Ejemplos de cargas útiles Norito

### 6.1 Ejemplo de registro de nombre

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

### 6.2 Ejemplo de política de sufijos

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

## 7. Etapas de prochaines- **SN-2b (API de registro y ganchos de gobierno):** expone estas estructuras a través de Torii (enlaces Norito y JSON) y conecta las comprobaciones de admisión a los artefactos de gobierno.
- **SN-3 (motor de subasta y registro):** reutilizador `NameAuctionStateV1` para implementar la lógica de confirmación/revelación y reapertura holandesa.
- **SN-5 (Pago y liquidación):** explotador `RevenueShareRecordV1` para la reconciliación financiera y la automatización de relaciones.

Les questions ou demandes de changement doivent etre deposees avec les mises a jour du roadmap SNS dans `roadmap.md` et refletees dans `status.md` lors de la fusion.