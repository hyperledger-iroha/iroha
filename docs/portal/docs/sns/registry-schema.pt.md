---
lang: pt
direction: ltr
source: docs/portal/docs/sns/registry-schema.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f8df534d156b6a3e516cdd71b4c2f8ea2c6473e45afc643000e450d6d331190
source_last_modified: "2025-11-15T16:28:04.299911+00:00"
translation_last_reviewed: 2026-01-01
---

:::note Fonte canonica
Esta pagina espelha `docs/source/sns/registry_schema.md` e agora serve como a copia canonica do portal. O arquivo fonte permanece para atualizacoes de traducao.
:::

# Esquema do registro do Sora Name Service (SN-2a)

**Status:** Redigido 2026-03-24 -- submetido para revisao do programa SNS  
**Link do roadmap:** SN-2a "Registry schema & storage layout"  
**Escopo:** Definir as estruturas Norito canonicas, os estados de ciclo de vida e os eventos emitidos para o Sora Name Service (SNS) para que as implementacoes de registro e registrar fiquem deterministicas em contratos, SDKs e gateways.

Este documento completa o entregavel de esquema para SN-2a ao especificar:

1. Identificadores e regras de hashing (`SuffixId`, `NameHash`, derivacao de seletores).
2. Structs/enums Norito para registros de nomes, politicas de sufixos, tiers de preco, reparticoes de receita e eventos do registro.
3. Layout de armazenamento e prefixes de indices para replay deterministico.
4. Uma maquina de estados cobrindo registro, renovacao, grace/redemption, freezes e tombstones.
5. Eventos canonicos consumidos pela automacao DNS/gateway.

## 1. Identificadores e hashing

| Identificador | Descricao | Derivacao |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Identificador do registro para sufixos de nivel superior (`.sora`, `.nexus`, `.dao`). Alinhado com o catalogo de sufixos em [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Atribuido por voto de governanca; armazenado em `SuffixPolicyV1`. |
| `SuffixSelector` | Forma canonica em string do sufixo (ASCII, lower-case). | Exemplo: `.sora` -> `sora`. |
| `NameSelectorV1` | Seletor binario para o rotulo registrado. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. O rotulo e NFC + lower-case segundo Norm v1. |
| `NameHash` (`[u8;32]`) | Chave primaria de busca usada por contratos, eventos e caches. | `blake3(NameSelectorV1_bytes)`. |

Requisitos de determinismo:

- Os rotulos sao normalizados via Norm v1 (UTS-46 strict, STD3 ASCII, NFC). As strings de usuario DEVEM ser normalizadas antes do hash.
- Rotulos reservados (de `SuffixPolicyV1.reserved_labels`) nunca entram no registro; overrides apenas de governanca emitem eventos `ReservedNameAssigned`.

## 2. Estruturas Norito

### 2.1 NameRecordV1

| Campo | Tipo | Notas |
|-------|------|-------|
| `suffix_id` | `u16` | Referencia `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Bytes do seletor bruto para auditoria/debug. |
| `name_hash` | `[u8; 32]` | Chave para mapas/eventos. |
| `normalized_label` | `AsciiString` | Rotulo legivel por humanos (post Norm v1). |
| `display_label` | `AsciiString` | Casing fornecido pelo steward; cosmetica opcional. |
| `owner` | `AccountId` | Controla renovacoes/transferencias. |
| `controllers` | `Vec<NameControllerV1>` | Referencias a enderecos de conta alvo, resolvers ou metadata de aplicacao. |
| `status` | `NameStatus` | Indicador de ciclo de vida (ver Secao 4). |
| `pricing_class` | `u8` | Indice nos tiers de preco do sufixo (standard, premium, reserved). |
| `registered_at` | `Timestamp` | Timestamp de bloco da ativacao inicial. |
| `expires_at` | `Timestamp` | Fim do termo pago. |
| `grace_expires_at` | `Timestamp` | Fim da grace de auto-renovacao (default +30 dias). |
| `redemption_expires_at` | `Timestamp` | Fim da janela de redemption (default +60 dias). |
| `auction` | `Option<NameAuctionStateV1>` | Presente quando ha Dutch reopen ou leiloes premium ativos. |
| `last_tx_hash` | `Hash` | Ponteiro determinista para a transacao que gerou esta versao. |
| `metadata` | `Metadata` | Metadata arbitraria do registrar (text records, proofs). |

Structs de suporte:

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

| Campo | Tipo | Notas |
|-------|------|-------|
| `suffix_id` | `u16` | Chave primaria; estavel entre versoes de politica. |
| `suffix` | `AsciiString` | por exemplo, `sora`. |
| `steward` | `AccountId` | Steward definido no charter de governanca. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Identificador de ativo de settlement por padrao (por exemplo `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | Coeficientes de preco por tiers e regras de duracao. |
| `min_term_years` | `u8` | Piso para o termo comprado independentemente de overrides de tier. |
| `grace_period_days` | `u16` | Default 30. |
| `redemption_period_days` | `u16` | Default 60. |
| `max_term_years` | `u8` | Maximo de renovacao antecipada. |
| `referral_cap_bps` | `u16` | <=1000 (10%) segundo o charter. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Lista fornecida pela governanca com instrucoes de atribuicao. |
| `fee_split` | `SuffixFeeSplitV1` | Partes tesouraria / steward / referral (basis points). |
| `fund_splitter_account` | `AccountId` | Conta que mantem escrow + distribui fundos. |
| `policy_version` | `u16` | Incrementa em cada mudanca. |
| `metadata` | `Metadata` | Notas estendidas (KPI covenant, hashes de docs de compliance). |

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

### 2.3 Registros de receita e settlement

| Struct | Campos | Proposito |
|--------|--------|-----------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Registro deterministico de pagamentos roteados por epoca de settlement (semanal). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Emitido cada vez que um pagamento e postado (registro, renovacao, leilao). |

Todos os campos `TokenValue` usam a codificacao fixa canonica de Norito com o codigo de moeda declarado no `SuffixPolicyV1` associado.

### 2.4 Eventos do registro

Eventos canonicos fornecem um log de replay para automacao DNS/gateway e analiticas.

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

Os eventos devem ser anexados a um log reprodivel (por exemplo, o dominio `RegistryEvents`) e refletidos nos feeds de gateway para que os caches DNS invalidem dentro do SLA.

## 3. Layout de armazenamento e indices

| Chave | Descricao |
|-----|-------------|
| `Names::<name_hash>` | Mapa primario de `name_hash` para `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Indice secundario para UI de wallet (paginacao amigavel). |
| `NamesByLabel::<suffix_id, normalized_label>` | Detecta conflitos, habilita busca deterministica. |
| `SuffixPolicies::<suffix_id>` | Ultimo `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Historico de `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Log append-only com chave de sequencia monotona. |

Todas as chaves serializam usando tuplas Norito para manter o hashing deterministico entre hosts. Atualizacoes de indice ocorrem de forma atomica junto com o registro primario.

## 4. Maquina de estados do ciclo de vida

| Estado | Condicoes de entrada | Transicoes permitidas | Notas |
|-------|----------------------|----------------------|-------|
| Available | Derivado quando `NameRecord` esta ausente. | `PendingAuction` (premium), `Active` (registro standard). | A busca de disponibilidade le apenas indices. |
| PendingAuction | Criado quando `PriceTierV1.auction_kind` != none. | `Active` (leilao liquida), `Tombstoned` (sem lances). | Leiloes emitem `AuctionOpened` e `AuctionSettled`. |
| Active | Registro ou renovacao bem-sucedida. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` guia a transicao. |
| GracePeriod | Automatico quando `now > expires_at`. | `Active` (renovacao em dia), `Redemption`, `Tombstoned`. | Default +30 dias; ainda resolve mas sinalizado. |
| Redemption | `now > grace_expires_at` mas `< redemption_expires_at`. | `Active` (renovacao tardia), `Tombstoned`. | Comandos exigem taxa de penalidade. |
| Frozen | Freeze de governanca ou guardian. | `Active` (apos remediacao), `Tombstoned`. | Nao pode transferir nem atualizar controllers. |
| Tombstoned | Renuncia voluntaria, resultado de disputa permanente ou redemption expirada. | `PendingAuction` (Dutch reopen) ou permanece tombstoned. | O evento `NameTombstoned` deve incluir razao. |

As transicoes de estado DEVEM emitir o `RegistryEventKind` correspondente para manter caches downstream coerentes. Nomes tombstoned que entram em leiloes Dutch reopen anexam um payload `AuctionKind::DutchReopen`.

## 5. Eventos canonicos e sync de gateways

Gateways assinam `RegistryEventV1` e sincronizam DNS/SoraFS ao:

1. Buscar o ultimo `NameRecordV1` referenciado pela sequencia de eventos.
2. Regenerar templates de resolver (enderecos I105 preferidos + I105 como segunda opcao, text records).
3. Pinnear dados de zona atualizados via o fluxo SoraDNS descrito em [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Garantias de entrega de eventos:

- Cada transacao que afeta um `NameRecordV1` *deve* anexar exatamente um evento com `version` estritamente crescente.
- Eventos `RevenueSharePosted` referenciam liquidacoes emitidas por `RevenueShareRecordV1`.
- Eventos de freeze/unfreeze/tombstone incluem hashes de artefatos de governanca dentro de `metadata` para replay de auditoria.

## 6. Exemplos de payloads Norito

### 6.1 Exemplo de NameRecord

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

### 6.2 Exemplo de SuffixPolicy

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

## 7. Proximos passos

- **SN-2b (Registrar API & governance hooks):** expor estes structs via Torii (bindings Norito e JSON) e ligar checks de admission a artefatos de governanca.
- **SN-3 (Auction & registration engine):** reutilizar `NameAuctionStateV1` para implementar logica de commit/reveal e Dutch reopen.
- **SN-5 (Payment & settlement):** aproveitar `RevenueShareRecordV1` para reconciliacao financeira e automacao de relatorios.

Perguntas ou solicitacoes de mudanca devem ser registradas junto com as atualizacoes do roadmap SNS em `roadmap.md` e refletidas em `status.md` quando integradas.
