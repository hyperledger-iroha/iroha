---
lang: pt
direction: ltr
source: docs/portal/docs/sns/registry-schema.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Esta página reflete `docs/source/sns/registry_schema.md` e é uma cópia canônica do portal. Le fichier source reste pour les mises a jour de traduction.
:::

# Esquema de registro do Sora Name Service (SN-2a)

**Estatuto:** Redige 2026-03-24 -- soumis a la revue du program SNS  
**Roteiro de garantia:** SN-2a "Esquema de registro e layout de armazenamento"  
**Porte:** Defina as estruturas Norito canônicas, os estados de ciclo de vida e os eventos emitidos para o Sora Name Service (SNS) para que as implementações de registro e registrador permaneçam determinadas nos contratos, SDKs e gateways.

Este documento preenche o esquema disponível para SN-2a e precisa:

1. Identificadores e regras de hash (`SuffixId`, `NameHash`, derivação dos seletores).
2. Estruturas/enums Norito para registros de nomes, políticas de sufixos, níveis de preços, repartições de receitas e eventos de registro.
3. Layout de armazenamento e prefixos de índices para uma repetição determinada.
4. Uma máquina de estado cobre o registro, o renovo, a graça/redenção, o congelamento e as lápides.
5. Eventos canônicos consumidos pela automação DNS/gateway.

## 1. Identificadores e hashing

| Identificador | Descrição | Derivação |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Identificador de registro para os sufixos do primeiro nível (`.sora`, `.nexus`, `.dao`). Alinhe-se no catálogo de sufixos em [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Atributo por voto de governo; estoque em `SuffixPolicyV1`. |
| `SuffixSelector` | Forme canonique en chaine du suffixe (ASCII, minúsculas). | Exemplo: `.sora` -> `sora`. |
| `NameSelectorV1` | Selecione o binário para registrar o rótulo. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. O rótulo é NFC + minúsculas conforme Norm v1. |
| `NameHash` (`[u8;32]`) | A primeira pesquisa utilizada por contratos, eventos e caches. | `blake3(NameSelectorV1_bytes)`. |

Exigências de determinismo:

- Les rótulos são normalizados via Norm v1 (UTS-46 strict, STD3 ASCII, NFC). As cadeias de uso DOIVENT são normalizadas antes do hash.
- Les rótulos reservas (de `SuffixPolicyV1.reserved_labels`) n'entrent jamais dans le registre; as substituições de governança exclusiva emetem des eventos `ReservedNameAssigned`.

## 2. Estruturas Norito

### 2.1 NomeRegistroV1| Campeão | Tipo | Notas |
|-------|------|-------|
| `suffix_id` | `u16` | Referência `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Octetos do seletor bruto para auditoria/depuração. |
| `name_hash` | `[u8; 32]` | Cle para mapas/eventos. |
| `normalized_label` | `AsciiString` | Label lisible pour l'humain (pós Norma v1). |
| `display_label` | `AsciiString` | Invólucro fourni par le steward; opção cosmética. |
| `owner` | `AccountId` | Controle as renovações/transferências. |
| `controllers` | `Vec<NameControllerV1>` | Referências a endereços de contas, resolvedores ou metadados de aplicativos. |
| `status` | `NameStatus` | Indicador de ciclo de vida (veja a Seção 4). |
| `pricing_class` | `u8` | Índice nos níveis de preço do sufixo (padrão, premium, reservado). |
| `registered_at` | `Timestamp` | Timestamp do bloco inicial de ativação. |
| `expires_at` | `Timestamp` | Fin du terme paye. |
| `grace_expires_at` | `Timestamp` | Fin de grace d'auto-renouvellement (padrão +30 dias). |
| `redemption_expires_at` | `Timestamp` | Final da janela de resgate (padrão +60 dias). |
| `auction` | `Option<NameAuctionStateV1>` | Presente quando os holandeses reabrem ou enchem premium são ativos. |
| `last_tx_hash` | `Hash` | O ponteiro determina a transação que produz esta versão. |
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

| Campeão | Tipo | Notas |
|-------|------|-------|
| `suffix_id` | `u16` | Cle primário; estável entre versões de política. |
| `suffix` | `AsciiString` | por exemplo, `sora`. |
| `steward` | `AccountId` | Steward definido na carta de governo. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Identificador do ato de liquidação por padrão (por exemplo `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | Coeficientes de preço por parte e regras de duração. |
| `min_term_years` | `u8` | Plancher pour le terme achete quel que soit l'override de tier. |
| `grace_period_days` | `u16` | Padrão 30. |
| `redemption_period_days` | `u16` | Padrão 60. |
| `max_term_years` | `u8` | Máximo de renovo antecipado. |
| `referral_cap_bps` | `u16` | <=1000 (10%) de acordo com o contrato. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Liste as informações do governo com instruções de afetação. |
| `fee_split` | `SuffixFeeSplitV1` | Peças tresorerie / steward / encaminhamento (pontos base). |
| `fund_splitter_account` | `AccountId` | Compte qui detient l'escrow + distribue les fonds. |
| `policy_version` | `u16` | Incremente uma mudança cada. |
| `metadata` | `Metadata` | Notas etendues (conveniência de KPI, hashes de documentos de conformidade). |

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

### 2.3 Registros de receita e liquidação| Estrutura | Campeões | Mas |
|--------|--------|-----|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Enregistrement determinista des paiements route par epoque de liquidação (hebdomadaire). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Emis a chaque paiement poste (registro, renouvellement, enchere). |

Todos os campos `TokenValue` usam a codificação fixa canônica de Norito com o código desenvolvido declarado na associação `SuffixPolicyV1`.

### 2.4 Eventos de registro

Os eventos canônicos fornecem um log de repetição para automação DNS/gateway e análise.

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

Os eventos devem ser adicionados a um log atualizado (por exemplo, o domínio `RegistryEvents`) e refletido sobre o gateway de feeds para que os caches DNS sejam inválidos no SLA.

## 3. Layout de estoque e índices

| Clé | Descrição |
|-----|-------------|
| `Names::<name_hash>` | Mapa primário de `name_hash` versus `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Índice secundário para carteira UI (paginação amigável). |
| `NamesByLabel::<suffix_id, normalized_label>` | Detecte os conflitos, alimente a pesquisa determinada. |
| `SuffixPolicies::<suffix_id>` | Dernier `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Histórico `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Registrar sequência cle par somente anexada monótona. |

Todos os arquivos são serializados através das tuplas Norito para criar um hash determinado entre hotéis. As mises a jour d'index se fonte atomicamente com o registro principal.

## 4. Máquina de estados do ciclo de vida

| Estado | Condições de entrada | Permissões de transições | Notas |
|-------|--------------------|---------------------|-------|
| Disponível | Derive quando `NameRecord` está ausente. | `PendingAuction` (premium), `Active` (padrão de registro). | A pesquisa de disponibilidade abrange apenas os índices. |
| Leilão Pendente | Cree quand `PriceTierV1.auction_kind` != nenhum. | `Active` (encher reglee), `Tombstoned` (aucune enchere). | As aberturas são emitidas `AuctionOpened` e `AuctionSettled`. |
| Ativo | Registro ou renovação reussi. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` piloto de transição. |
| Período de graça | Automático quando `now > expires_at`. | `Active` (renovação temporária), `Redemption`, `Tombstoned`. | Padrão +30 dias; resolução mais marca. |
| Redenção | `now > grace_expires_at` mais `< redemption_expires_at`. | `Active` (renovação tardia), `Tombstoned`. | Les commandes exigem des frais de penalite. |
| Congelado | Freeze de gouvernance ou guardião. | `Active` (após remediação), `Tombstoned`. | Não é possível transferir nem colocar os controladores no dia. |
| Lápide | Abandono voluntário, resultado de litígio permanente, ou resgate expirado. | `PendingAuction` (reabertura holandesa) ou resto lápide. | O evento `NameTombstoned` inclui uma razão. |As transições de estado DOIVENT emitem o `RegistryEventKind` correspondente para que os caches downstream permaneçam coerentes. Os nomes tombstoned entrant em encheres Dutch reabrem anexado uma carga útil `AuctionKind::DutchReopen`.

## 5. Eventos canônicos e gateway de sincronização

Os gateways são transferidos para `RegistryEventV1` e sincronizam DNS/SoraFS via:

1. Recupere a referência `NameRecordV1` anterior pela sequência de eventos.
2. Regenerar os modelos de resolução (endereços I105 preferidos + compactados (`sora`) na segunda opção, registros de texto).
3. Fixe os dados da zona atual por meio do fluxo de trabalho SoraDNS decrit em [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Garantias de entrega de eventos:

- Cada transação que afeta um `NameRecordV1` *doit* adiciona exatamente um evento com `version` estrito croissante.
- Os eventos `RevenueSharePosted` referem-se aos assentamentos emitidos por `RevenueShareRecordV1`.
- Os eventos de congelamento/descongelamento/tombstone incluem hashes de artefatos de governo em `metadata` para reprodução de auditoria.

## 6. Exemplos de cargas úteis Norito

### 6.1 Exemplo de Registro de Nome

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

### 6.2 Exemplo de política de sufixo

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

## 7. Étapes de Prochaines

- **SN-2b (API do registrador e ganchos de governança):** expõe essas estruturas via Torii (ligações Norito e JSON) e conecta as verificações de admissão a artefatos de governo.
- **SN-3 (mecanismo de leilão e registro):** reutilizador `NameAuctionStateV1` para implementar a lógica de confirmação/revelação e reabertura holandesa.
- **SN-5 (Pagamento e liquidação):** explorador `RevenueShareRecordV1` para reconciliação financeira e automação de relatórios.

As perguntas ou demandas de mudança devem ser depostas com as mises a jour du roadmap SNS em `roadmap.md` e refletidas em `status.md` durante a fusão.