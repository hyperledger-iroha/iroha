---
lang: pt
direction: ltr
source: docs/portal/docs/sns/registry-schema.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Esta página reflete `docs/source/sns/registry_schema.md` e agora sirve como a cópia canônica do portal. O arquivo fonte é mantido para atualizações de tradução.
:::

# Esquema de registro do Sora Name Service (SN-2a)

**Estado:** Redactado 2026-03-24 -- enviada a revisão do programa SNS  
**Incluir roteiro:** SN-2a "Esquema de registro e layout de armazenamento"  
**Alcance:** Defina as estruturas Norito canônicas, os estados de ciclo de vida e os eventos emitidos para o Sora Name Service (SNS) de forma que as implementações de registro e registrador sejam mantidas deterministas em contratos, SDKs e gateways.

Este documento completa a entrega do esquema para SN-2a conforme especificação:

1. Identificadores e regras de hash (`SuffixId`, `NameHash`, derivação de seletores).
2. Estruturas/enums Norito para registros de nomes, políticas de sufijos, níveis de preços, partes de ingressos e eventos de registro.
3. Layout de armazenamento e prefijos de índices para reprodução determinista.
4. Uma máquina de estados que armazena registro, renovação, graça/redenção, congelamento e lápide.
5. Eventos canônicos consumidos pela automação DNS/gateway.

## 1. Identificadores e hashing

| Identificador | Descrição | Derivação |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Identificador do registro para sufixos de nível superior (`.sora`, `.nexus`, `.dao`). Alineado com o catálogo de sufijos em [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Atribuído por voto de governo; armazenado em `SuffixPolicyV1`. |
| `SuffixSelector` | Forma canônica em string del sufijo (ASCII, minúscula). | Exemplo: `.sora` -> `sora`. |
| `NameSelectorV1` | Seletor binário para a marca registrada. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. A etiqueta é NFC + minúsculas segun Norm v1. |
| `NameHash` (`[u8;32]`) | Chave primária de busca usada por contratos, eventos e caches. | `blake3(NameSelectorV1_bytes)`. |

Requisitos de determinismo:

- As etiquetas são normalizadas via Norma v1 (UTS-46 strict, STD3 ASCII, NFC). As cadeias do usuário DEBEN se normalizam antes do hash.
- As etiquetas reservadas (de `SuffixPolicyV1.reserved_labels`) nunca entram no registro; as substituições apenas de governo emitem eventos `ReservedNameAssigned`.

## 2. Estruturas Norito

### 2.1 NomeRegistroV1| Campo | Tipo | Notas |
|-------|------|-------|
| `suffix_id` | `u16` | Referência `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Bytes do seletor sem processamento para auditoria/depuração. |
| `name_hash` | `[u8; 32]` | Clave para mapas/eventos. |
| `normalized_label` | `AsciiString` | Etiqueta legível por humanos (pós Norma v1). |
| `display_label` | `AsciiString` | Invólucro fornecido por mordomo; cosmética opcional. |
| `owner` | `AccountId` | Controle de renovações/transferências. |
| `controllers` | `Vec<NameControllerV1>` | Referências a direções de objetivo, resolvedores ou metadados de aplicação. |
| `status` | `NameStatus` | Bandera de ciclo de vida (ver Seção 4). |
| `pricing_class` | `u8` | Índice em níveis de preços do sufijo (padrão, premium, reservado). |
| `registered_at` | `Timestamp` | Timestamp do bloqueio da ativação inicial. |
| `expires_at` | `Timestamp` | Fin del termino pago. |
| `grace_expires_at` | `Timestamp` | Fin de gracia de auto-renovacion (padrão +30 dias). |
| `redemption_expires_at` | `Timestamp` | Fin de ventana de redencion (padrão +60 dias). |
| `auction` | `Option<NameAuctionStateV1>` | Presente quando se reabre holandês ou subastas premium estão ativados. |
| `last_tx_hash` | `Hash` | Puntero determinista para a transação que produziu esta versão. |
| `metadata` | `Metadata` | Metadados arbitraria del registrador (registros de texto, provas). |

Estruturas de suporte:

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

| Campo | Tipo | Notas |
|-------|------|-------|
| `suffix_id` | `u16` | Clave primária; estável entre versões de política. |
| `suffix` | `AsciiString` | por exemplo, `sora`. |
| `steward` | `AccountId` | Steward definido na carta de governo. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Identificador de ativo de liquidação por defeito (por exemplo `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | Coeficientes de preços por níveis e regras de duração. |
| `min_term_years` | `u8` | Piso para o terminal comprado sem importar substituições de nível. |
| `grace_period_days` | `u16` | Padrão 30. |
| `redemption_period_days` | `u16` | Padrão 60. |
| `max_term_years` | `u8` | Máximo de renovação por adelantado. |
| `referral_cap_bps` | `u16` | <=1000 (10%) após o charter. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Lista fornecida pelo governo com instruções de atribuição. |
| `fee_split` | `SuffixFeeSplitV1` | Porciones de tesoreria / steward / encaminhamento (pontos base). |
| `fund_splitter_account` | `AccountId` | Cuenta que mantiene escrow + distribuição de fundos. |
| `policy_version` | `u16` | Incremente em cada mudança. |
| `metadata` | `Metadata` | Notas estendidas (convênio KPI, hashes de documentos de cumprimento). |

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

### 2.3 Registros de ingressos e liquidações| Estrutura | Campos | Proposta |
|--------|--------|-----|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Registro determinista de pagamentos efetuados por época de liquidação (semanal). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Emitido cada vez que um pagamento se registre (registro, renovação, subasta). |

Todos os campos `TokenValue` usam a codificação fija canônica de Norito com o código de moeda declarado no `SuffixPolicyV1` associado.

### 2.4 Eventos de registro

Os eventos canônicos provaram um log de repetição para automatização de DNS/gateway e análise.

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

Os eventos devem ser agregados a um log reproduzível (por exemplo, o domínio `RegistryEvents`) e refletidos nos feeds do gateway para que os caches DNS sejam invalidados dentro do SLA.

## 3. Layout de armazenamento e índices

| Clave | Descrição |
|-----|-------------|
| `Names::<name_hash>` | Mapa primário de `name_hash` para `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Índice secundário para UI de carteira (pagina amigável). |
| `NamesByLabel::<suffix_id, normalized_label>` | Detecta conflitos, habilita busqueda determinista. |
| `SuffixPolicies::<suffix_id>` | Último `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Histórico de `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Log anexado apenas com chave de sequência monotônica. |

Todas as chaves são serializadas usando tuplas Norito para manter o hash determinista entre hosts. As atualizações de índices ocorrem de forma atômica junto com o registro primário.

## 4. Máquina de estados do ciclo de vida

| Estado | Condições de entrada | Transições permitidas | Notas |
|---|-------------|--------------|-------|
| Disponível | Derivado quando `NameRecord` está ausente. | `PendingAuction` (premium), `Active` (registro padrão). | A busca de disponibilidade de índices individuais. |
| Leilão Pendente | Criado quando `PriceTierV1.auction_kind` != none. | `Active` (la subasta se liquida), `Tombstoned` (sem pujas). | As subastas emitem `AuctionOpened` e `AuctionSettled`. |
| Ativo | Registro de renovação exitosa. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` impulsa a transição. |
| Período de graça | Automático quando `now > expires_at`. | `Active` (renovação temporária), `Redemption`, `Tombstoned`. | Padrão +30 dias; aun resuelve pero marcado. |
| Redenção | `now > grace_expires_at` mas `< redemption_expires_at`. | `Active` (renovação tardia), `Tombstoned`. | Os comandos exigem taxa de penalidade. |
| Congelado | Freeze de gobernanza o guardião. | `Active` (trans remediação), `Tombstoned`. | Não é possível transferir ou atualizar controladores. |
| Lápide | Rendição voluntária, resultado de disputa permanente, ou redenção expirada. | `PendingAuction` (reabertura holandesa) o permanece marcado para exclusão. | O evento `NameTombstoned` deve incluir razão. |As transições de estado DEBEN emitem o correspondente `RegistryEventKind` para que os caches downstream se mantenham coerentes. Os nomes marcados para exclusão que entram nas subastas holandesas reabrem junto com a carga útil `AuctionKind::DutchReopen`.

## 5. Eventos canônicos e sincronização de gateways

Os gateways são inscritos em `RegistryEventV1` e sincronizados em DNS/SoraFS por meio de:

1. Obtenha o último `NameRecordV1` referenciado pela sequência de eventos.
2. Regenerar modelos de resolução (direções i105 preferidas + compactadas (`sora`) como segunda opção, registros de texto).
3. Pesquise dados de zona atualizados por meio do fluxo SoraDNS descrito em [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Garantias de entrega de eventos:

- Cada transação que afeta um `NameRecordV1` *deve* adicionar exatamente um evento com `version` estritamente crente.
- Os eventos `RevenueSharePosted` referem-se a liquidações emitidas por `RevenueShareRecordV1`.
- Os eventos de congelamento/descongelamento/tombstone incluem hashes de artefatos de governança dentro de `metadata` para reprodução de auditórios.

## 6. Exemplos de cargas úteis Norito

### 6.1 Exemplo de NameRecord

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

### 6.2 Exemplo de SuffixPolicy

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

## 7. Próximos passos

- **SN-2b (Registrar API & governance hooks):** exponer estos structs via Torii (bindings Norito y JSON) y conectar admission checks a artefactos de gobernanza.
- **SN-3 (mecanismo de leilão e registro):** reutilizar `NameAuctionStateV1` para implementar lógica de confirmação/revelação e reabertura holandesa.
- **SN-5 (Pagamento e liquidação):** aprovação `RevenueShareRecordV1` para reconciliação financeira e automatização de relatórios.

As perguntas ou solicitações de mudança devem ser registradas junto com as atualizações do roteiro do SNS em `roadmap.md` e refletidas em `status.md` quando se integram.