---
lang: es
direction: ltr
source: docs/portal/docs/sns/registry-schema.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Esta página espelha `docs/source/sns/registry_schema.md` y ahora sirve como copia canónica del portal. La fuente del archivo permanece para actualizar la traducción.
:::

# Esquema do registro do Sora Name Service (SN-2a)

**Estado:** Redigido 2026-03-24 -- submetido para revisaro do programa SNS  
**Enlace a hoja de ruta:** SN-2a "Esquema de registro y diseño de almacenamiento"  
**Escopo:** Definir las estructuras Norito canónicas, los estados de ciclo de vida y los eventos emitidos para el Sora Name Service (SNS) para que como implementaciones de registro y registrador fiquem determinísticas en contratos, SDK y puertas de enlace.

Este documento completo o entregavel de esquema para SN-2a ao especifica:

1. Identificadores e registros de hash (`SuffixId`, `NameHash`, derivacao de seletores).
2. Structs/enums Norito para registros de nombres, políticas de sufijos, niveles de preco, reparticos de recepción y eventos de registro.
3. Diseño de armazenamento y prefijos de índices para reproducción determinística.
4. Uma maquina de estados cobrindo registro, renovacao, gracia/redención, congela e lápidas.
5. Eventos canónicos consumidos en automático DNS/gateway.

## 1. Identificadores y hash| Identificador | Descripción | Derivacao |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Identificador de registro para sufijos de nivel superior (`.sora`, `.nexus`, `.dao`). Alinhado com o catalogo de sufixos em [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Atribuido por voto de gobernancia; armado en `SuffixPolicyV1`. |
| `SuffixSelector` | Forma canónica en cadena de sufijo (ASCII, minúscula). | Ejemplo: `.sora` -> `sora`. |
| `NameSelectorV1` | Selector binario para el rotulo registrado. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. O rotulo e NFC + minúscula segunda Norma v1. |
| `NameHash` (`[u8;32]`) | Chave primaria de busca usada por contratos, eventos y cachés. | `blake3(NameSelectorV1_bytes)`. |

Requisitos de determinismo:

- Os rotulos sao normalizados vía Norm v1 (UTS-46 estricto, STD3 ASCII, NFC). As strings de usuario DEVEM ser normalizadas antes de hacer hash.
- Rotulos reservados (de `SuffixPolicyV1.reserved_labels`) nunca entram no registro; anula apenas de gobierno emitem eventos `ReservedNameAssigned`.

## 2. Estructuras Norito

### 2.1 NombreRegistroV1| Campo | Tipo | Notas |
|-------|------|-------|
| `suffix_id` | `u16` | Referencia `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Bytes del selector bruto para auditoría/depuración. |
| `name_hash` | `[u8; 32]` | Chave para mapas/eventos. |
| `normalized_label` | `AsciiString` | Rotulo legivel por humanos (post Norm v1). |
| `display_label` | `AsciiString` | Casing fornecido pelo mayordomo; cosmética opcional. |
| `owner` | `AccountId` | Controla renovações/transferencias. |
| `controllers` | `Vec<NameControllerV1>` | Referencias a enderecos de conta alvo, resolutores o metadatos de aplicaciones. |
| `status` | `NameStatus` | Indicador de ciclo de vida (ver Secao 4). |
| `pricing_class` | `u8` | Indice nos tiers de preco do sufixo (estándar, premium, reservado). |
| `registered_at` | `Timestamp` | Marca de tiempo del bloque de activación inicial. |
| `expires_at` | `Timestamp` | Fim do termo pago. |
| `grace_expires_at` | `Timestamp` | Fim da gracia de auto-renovacao (predeterminado +30 días). |
| `redemption_expires_at` | `Timestamp` | Fim da janela de redemption (predeterminado +60 días). |
| `auction` | `Option<NameAuctionStateV1>` | Presente quando ha Dutch reapertura ou leiloes premium ativos. |
| `last_tx_hash` | `Hash` | Ponteiro determinista para a transacao que gerou esta versao. || `metadata` | `Metadata` | Metadatos arbitraria do registrador (registros de texto, pruebas). |

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
| `suffix_id` | `u16` | Chávez primaria; estavel entre versos de politica. |
| `suffix` | `AsciiString` | Por ejemplo, `sora`. |
| `steward` | `AccountId` | Steward no ha definido una carta de gobierno. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Identificador de ativo de asentamiento por padrao (por ejemplo `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | Coeficientes de preco por tiers e regras de duracao. |
| `min_term_years` | `u8` | Piso para o termo comprado independientemente de anulaciones de nivel. |
| `grace_period_days` | `u16` | Predeterminado 30. |
| `redemption_period_days` | `u16` | Predeterminado 60. |
| `max_term_years` | `u8` | Máximo de renovación antecipada. |
| `referral_cap_bps` | `u16` | <=1000 (10%) segundo o charter. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Lista fornecida pelagobernanza com instrucoes de atribuicao. |
| `fee_split` | `SuffixFeeSplitV1` | Partes tesouraria / azafata / remisión (puntos básicos). |
| `fund_splitter_account` | `AccountId` | Conta que mantem escrow + distribui fundos. |
| `policy_version` | `u16` | Incrementa em cada mudanza. |
| `metadata` | `Metadata` | Notas estendidas (KPI covenant, hashes de docs de cumplimiento). |

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
```### 2.3 Registros de recibo y liquidación

| Estructura | Campos | propuesta |
|--------|--------|-----------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Registro determinístico de pagos roteados por época de liquidación (semanal). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Emitido cada vez que um pagomento e postado (registro, renovacao, leilao). |

Todos los campos `TokenValue` usan la codificación fija canónica de Norito con el código de moneda declarado no `SuffixPolicyV1` asociado.

### 2.4 Eventos de registro

Los eventos canónicos requieren un registro de reproducción para DNS/gateway automático y análisis.

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

Los eventos deben ser anexos a una reproducción de registros (por ejemplo, el dominio `RegistryEvents`) y reflejados en los feeds de la puerta de enlace para que los cachés DNS no sean válidos dentro del SLA.

## 3. Diseño de armazenamento e índices| Chávez | Descripción |
|-----|-------------|
| `Names::<name_hash>` | Mapa primario de `name_hash` para `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Índice secundario para UI de wallet (paginacao amigavel). |
| `NamesByLabel::<suffix_id, normalized_label>` | Detecta conflictos, habilita busca determinística. |
| `SuffixPolicies::<suffix_id>` | Último `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Histórico de `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Registro de solo agregar con chave de secuencia monotona. |

Todas las chaves serializan usando tuplas Norito para mantener o hash determinístico entre hosts. Actualización de índices de forma atómica junto con el registro primario.

## 4. Máquina de estados del ciclo de vida| Estado | Condicos de entrada | Transicos permitidos | Notas |
|-------|----------------------|----------------------|-------|
| Disponible | Derivado cuando `NameRecord` está ausente. | `PendingAuction` (premium), `Active` (registro estándar). | A busca de disponibilidade le apenas índices. |
| Subasta Pendiente | Criado cuando `PriceTierV1.auction_kind` != none. | `Active` (leilao liquida), `Tombstoned` (sem lanzas). | Los mensajes emiten `AuctionOpened` e `AuctionSettled`. |
| Activo | Registro ou renovacao bem-sucedida. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` guia a transicao. |
| Período de Gracia | Automático cuando `now > expires_at`. | `Active` (renovación en diámetro), `Redemption`, `Tombstoned`. | Predeterminado +30 días; Ainda resuelve mas sinalizado. |
| Redención | `now > grace_expires_at` más `< redemption_expires_at`. | `Active` (renovación tardía), `Tombstoned`. | Los comandos exigen taxa de penalidad. |
| Congelado | Freeze degobernanca o guardián. | `Active` (apos remediacao), `Tombstoned`. | No puede transferir ni actualizar controladores. |
| Lápida | Renuncia voluntaria, resultado de disputa permanente o redención vencida. | `PendingAuction` (reapertura holandesa) o permanece desechado. | O evento `NameTombstoned` debe incluir razao. |As transicoes de estado DEVEM emiten o `RegistryEventKind` correspondientes a manter caches downstream coerentes. Los nombres desechados que entram em leiloes holandeses reabren un examen de carga útil `AuctionKind::DutchReopen`.

## 5. Eventos canónicos y sincronización de puertas de enlace

Puertas de enlace asignadas `RegistryEventV1` y sincronizadas DNS/SoraFS ao:

1. Buscar o ultimo `NameRecordV1` referenciado pela secuencia de eventos.
2. Regenerar plantillas de resolución (enderecos i105 preferidos + comprimidos (`sora`) como segunda opción, registros de texto).
3. Pincer los datos de zona actualizados a través del flujo SoraDNS descrito en [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Garantías de entrega de eventos:

- Cada transacao que afeta um `NameRecordV1` *deve* anexar exactamente un evento con `version` estritamente creciente.
- Eventos `RevenueSharePosted` referenciam liquidacoes emitidos por `RevenueShareRecordV1`.
- Los eventos de congelación/descongelación/tombstone incluyen hashes de artefactos de gobierno dentro de `metadata` para reproducción de auditorios.

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

## 7. Próximos pasos- **SN-2b (API de registro y ganchos de gobernanza):** exporta estas estructuras a través de Torii (bindings Norito y JSON) y ligar controles de admisión a artefatos de gobernanza.
- **SN-3 (motor de subasta y registro):** reutilizar `NameAuctionStateV1` para implementar la lógica de confirmación/revelación y reapertura holandesa.
- **SN-5 (Pago y liquidación):** aproveitar `RevenueShareRecordV1` para reconciliacao Financeira e automacao de relatorios.

Perguntas ou solicitacoes de mudanca deben ser registradas junto con las actualizadas en roadmap SNS en `roadmap.md` y refletidas en `status.md` cuando están integradas.