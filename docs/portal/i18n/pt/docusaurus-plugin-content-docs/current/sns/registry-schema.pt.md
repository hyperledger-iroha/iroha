---
lang: pt
direction: ltr
source: docs/portal/docs/sns/registry-schema.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Esta página espelha `docs/source/sns/registry_schema.md` e agora serve como uma cópia canônica do portal. O arquivo fonte permanece para atualizações de tradução.
:::

# Esquema de registro do Sora Name Service (SN-2a)

**Status:** Redigido 2026-03-24 -- designado para revisão do programa SNS  
**Link do roteiro:** SN-2a "Esquema de registro e layout de armazenamento"  
**Escopo:** Definir as estruturas Norito canônicas, os estados de ciclo de vida e os eventos emitidos para o Sora Name Service (SNS) para que as implementações de registro e registrador tornem-se determinísticas em contratos, SDKs e gateways.

Este documento completo o esquema de entrega para SN-2a ao especificar:

1. Identificadores e regras de hash (`SuffixId`, `NameHash`, derivação de seletores).
2. Estruturas/enums Norito para registros de nomes, políticas de sufixos, níveis de preço, repartições de receita e eventos do registro.
3. Layout de armazenamento e prefixos de índices para reprodução determinística.
4. Uma maquina de estados cobrindo registro, renovação, carência/resgate, congelamento e lápides.
5. Eventos canônicos consumidos pela automação DNS/gateway.

## 1. Identificadores e hashing

| Identificador | Descrição | Derivação |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Identificador do registro para sufixos de nível superior (`.sora`, `.nexus`, `.dao`). Alinhado com o catálogo de sufixos em [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Atribuido por voto de governança; armazenado em `SuffixPolicyV1`. |
| `SuffixSelector` | Forma canônica em string do sufixo (ASCII, minúscula). | Exemplo: `.sora` -> `sora`. |
| `NameSelectorV1` | Seletor binário para o rotulo registrado. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. O rotulo e NFC + minúsculo segundo Norm v1. |
| `NameHash` (`[u8;32]`) | Chave primária de busca usada por contratos, eventos e caches. | `blake3(NameSelectorV1_bytes)`. |

Requisitos de determinismo:

- Os rotulos são normalizados via Norma v1 (UTS-46 strict, STD3 ASCII, NFC). As strings do usuário DEVEM ser normalizadas antes do hash.
- Rótulos reservados (de `SuffixPolicyV1.reserved_labels`) nunca entram no registro; substitui apenas de governança emitem eventos `ReservedNameAssigned`.

## 2. Estruturas Norito

### 2.1 NomeRegistroV1| Campo | Tipo | Notas |
|-------|------|-------|
| `suffix_id` | `u16` | Referência `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Bytes do seletor bruto para auditoria/debug. |
| `name_hash` | `[u8; 32]` | Chave para mapas/eventos. |
| `normalized_label` | `AsciiString` | Rotulo legível por humanos (pós Norma v1). |
| `display_label` | `AsciiString` | Invólucro fornecido pelo administrador; cosmética opcional. |
| `owner` | `AccountId` | Controle de renovações/transferências. |
| `controllers` | `Vec<NameControllerV1>` | Referências a endereços de contato, resolvedores ou metadados de aplicação. |
| `status` | `NameStatus` | Indicador de ciclo de vida (ver Seção 4). |
| `pricing_class` | `u8` | Índice nos níveis de preço do sufixo (padrão, premium, reservado). |
| `registered_at` | `Timestamp` | Timestamp do bloco de ativação inicial. |
| `expires_at` | `Timestamp` | Fim do termo pago. |
| `grace_expires_at` | `Timestamp` | Fim da graça de auto-renovacao (padrão +30 dias). |
| `redemption_expires_at` | `Timestamp` | Fim da janela de resgate (padrão +60 dias). |
| `auction` | `Option<NameAuctionStateV1>` | Presente quando há reabertura holandesa ou leiloes premium ativos. |
| `last_tx_hash` | `Hash` | Ponteiro determinista para a transação que gerou esta versão. |
| `metadata` | `Metadata` | Metadados arbitrários do registrador (registros de texto, provas). |

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
| `suffix_id` | `u16` | Chave primária; estavel entre versos de política. |
| `suffix` | `AsciiString` | por exemplo, `sora`. |
| `steward` | `AccountId` | Steward não definiu nenhuma carta de governança. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Identificador de ativo de liquidação por padrão (por exemplo `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | Coeficientes de preço por níveis e regras de duração. |
| `min_term_years` | `u8` | Piso para o termo adquirido independentemente de substituições de nível. |
| `grace_period_days` | `u16` | Padrão 30. |
| `redemption_period_days` | `u16` | Padrão 60. |
| `max_term_years` | `u8` | Máximo de renovação antecipada. |
| `referral_cap_bps` | `u16` | <=1000 (10%) segundo o afretamento. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Lista fornecida pela governança com instruções de atribuição. |
| `fee_split` | `SuffixFeeSplitV1` | Partes tesouraria / steward / encaminhamento (pontos base). |
| `fund_splitter_account` | `AccountId` | Conta que mantém depósito + distribuição de fundos. |
| `policy_version` | `u16` | Incremente em cada mudança. |
| `metadata` | `Metadata` | Notas contínuas (conveniência de KPI, hashes de documentos de conformidade). |

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

### 2.3 Registros de receita e liquidação| Estrutura | Campos | Proposta |
|--------|--------|-----------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Registro determinístico de pagamentos roteados por época de liquidação (semanal). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Emitido cada vez que um pagamento e postado (registro, renovação, leilão). |

Todos os campos `TokenValue` usam a codificação fixa canônica de Norito com o código de moeda declarado no `SuffixPolicyV1` associado.

### 2.4 Eventos de registro

Eventos canônicos fornecem um log de replay para automação de DNS/gateway e análise.

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

Os eventos devem ser anexados a um log reproduzido (por exemplo, o domínio `RegistryEvents`) e refletidos nos feeds de gateway para que os caches DNS sejam invalidados dentro do SLA.

## 3. Layout de armazenamento e índices

| Chave | Descrição |
|-----|-------------|
| `Names::<name_hash>` | Mapa primário de `name_hash` para `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Índice secundário para UI de carteira (página amigavel). |
| `NamesByLabel::<suffix_id, normalized_label>` | Detecta conflitos, habilita busca determinística. |
| `SuffixPolicies::<suffix_id>` | Último `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Histórico de `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Log append-only com chave de sequencia monotona. |

Todas as chaves serializam usando tuplas Norito para manter o hash determinístico entre hosts. As atualizações de índice ocorrem de forma atômica junto com o registro primário.

## 4. Máquina de estados do ciclo de vida

| Estado | Condições de entrada | Transições permitidas | Notas |
|-------|-----------|----------------------|-------|
| Disponível | Derivado quando `NameRecord` está ausente. | `PendingAuction` (premium), `Active` (padrão de registro). | A busca de disponibilidade apenas de índices. |
| Leilão Pendente | Criado quando `PriceTierV1.auction_kind` != none. | `Active` (leilão líquido), `Tombstoned` (sem lanças). | Leilões emitem `AuctionOpened` e `AuctionSettled`. |
| Ativo | Registro ou renovação bem-sucedida. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` guia a transição. |
| Período de graça | Automaticamente quando `now > expires_at`. | `Active` (renovação em dia), `Redemption`, `Tombstoned`. | Padrão +30 dias; ainda resolvo mas sinalizado. |
| Redenção | `now > grace_expires_at` mas `< redemption_expires_at`. | `Active` (renovação tardia), `Tombstoned`. | Comandos desativam taxa de horário. |
| Congelado | Freeze de governança ou guardião. | `Active` (após remediação), `Tombstoned`. | Não é possível transferir nem atualizar controladores. |
| Lápide | Renúncia voluntária, resultado de disputa permanente ou resgate expirado. | `PendingAuction` (reabertura holandesa) ou permanece lápide. | O evento `NameTombstoned` deve incluir razão. |As transições de estado DEVEM emitir o `RegistryEventKind` correspondente para manter caches downstream coerentes. Nomes marcados para exclusão que ambos em leiloes Dutch reabrem o exame um payload `AuctionKind::DutchReopen`.

## 5. Eventos canônicos e sincronização de gateways

Gateways assinam `RegistryEventV1` e sincronizam DNS/SoraFS ao:

1. Buscar o último `NameRecordV1` referenciado pela sequência de eventos.
2. Regenerar templates de resolvedor (endereços IH58 preferidos + compactados (`sora`) como segunda opção, registros de texto).
3. Pinar dados de zona atualizados por meio do fluxo SoraDNS descrito em [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Garantias de entrega de eventos:

- Cada transação que afeta um `NameRecordV1` *deve* correção exatamente um evento com `version` intensidade crescente.
- Eventos `RevenueSharePosted` referenciam liquidações emitidas por `RevenueShareRecordV1`.
- Eventos de congelamento/descongelamento/tombstone incluem hashes de artistas de governança dentro de `metadata` para reprodução de auditoria.

## 6. Exemplos de cargas úteis Norito

### 6.1 Exemplo de NameRecord

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

### 6.2 Exemplo de SuffixPolicy

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

## 7. Próximos passos

- **SN-2b (API do registrador e ganchos de governança):** exportar essas structs via Torii (bindings Norito e JSON) e ligar verificações de admissão a artistas de governança.
- **SN-3 (mecanismo de leilão e registro):** reutilizar `NameAuctionStateV1` para implementar lógica de commit/reveal e Dutch opening.
- **SN-5 (Pagamento e liquidação):** aproveite `RevenueShareRecordV1` para reconciliação financeira e automação de relatórios.

Perguntas ou solicitações de mudança devem ser registradas junto com as atualizações do roadmap SNS em `roadmap.md` e refletidas em `status.md` quando integradas.