---
lang: pt
direction: ltr
source: docs/portal/docs/sns/registry-schema.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota História Canônica
Esta página abre `docs/source/sns/registry_schema.md` e abre o portal de cópia canônica. Esta é a opção certa para a operação atual.
:::

# Схема реестра Sora Name Service (SN-2a)

**Статус:** Черновик 2026-03-24 -- отправлено на ревью программы SNS  
**Guia do roteiro:** SN-2a "Esquema de registro e layout de armazenamento"  
**Exemplo:** Construa estruturas canônicas Norito, use uma bicicleta e uma solução para Sora Name Service (SNS), registros reais e registradores determinam a determinação do contrato, SDK e gateways.

Este documento foi criado para fornecer esquemas para SN-2a, incluindo:

1. Идентификаторы e правила хеширования (`SuffixId`, `NameHash`, derivação селекторов).
2. Estruturas/enums Norito para criar um exemplo, adicionar sufixos políticos, níveis de configuração, adicionar dados e fornecer реестра.
3. Layout de layout e configurações de índice para determinar a repetição.
4. Машину состояний, охватывающую регистрацию, продление, graça/redenção, congelamento e lápide.
5. Канонические события, потребляемые DNS/gateway automático.

## 1. Identificadores e хеширование

| Identificador | Descrição | Produzir |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Identificador de recuperação para um número suficiente de usuários (`.sora`, `.nexus`, `.dao`). Selecione o catálogo suficiente em [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Назначается голосованием по governança; verificado em `SuffixPolicyV1`. |
| `SuffixSelector` | Каноническая строковая форма суффикса (ASCII, minúsculas). | Exemplo: `.sora` -> `sora`. |
| `NameSelectorV1` | O seletor binário é classificado como padrão. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Letra em NFC + minúsculas no Norm v1. |
| `NameHash` (`[u8;32]`) | Abra a chave, use contratos, contratos e compras. | `blake3(NameSelectorV1_bytes)`. |

Determinação da determinação:

- Лейблы нормализуются через Norma v1 (UTS-46 estrito, STD3 ASCII, NFC). O conjunto de instruções padrão é normal durante a configuração.
- Зарезервированные лейблы (ou `SuffixPolicyV1.reserved_labels`) никогда не входят в реестр; substituições somente de governança выпускают события `ReservedNameAssigned`.

## 2. Estruturas Norito

### 2.1 NomeRegistroV1| Pólo | Tipo | Nomeação |
|-------|------|-----------|
| `suffix_id` | `u16` | Selecione `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Selecione o seletor para audit/debug. |
| `name_hash` | `[u8; 32]` | Ключ для карт/событий. |
| `normalized_label` | `AsciiString` | Leia a etiqueta (por Norm v1). |
| `display_label` | `AsciiString` | Invólucro do mordomo; cosméticos. |
| `owner` | `AccountId` | Atualize o produto/transferência. |
| `controllers` | `Vec<NameControllerV1>` | Selecione contas de endereço, resolvedores ou recursos de metadados. |
| `status` | `NameStatus` | Флаг жизненного цикла (ver. Seção 4). |
| `pricing_class` | `u8` | Os níveis indexados são suficientes (padrão, premium, reservado). |
| `registered_at` | `Timestamp` | Ative o bloco de ativação. |
| `expires_at` | `Timestamp` | A linha está aberta. |
| `grace_expires_at` | `Timestamp` | Конец graça renovação automática (padrão +30 dias). |
| `redemption_expires_at` | `Timestamp` | Конец окна resgate (padrão +60 dias). |
| `auction` | `Option<NameAuctionStateV1>` | Присутствует при reabertura holandesa ou аукционах premium. |
| `last_tx_hash` | `Hash` | Детерминированный указатель на транзакцию versa. |
| `metadata` | `Metadata` | Registrador de metadados Произвольная (registros de texto, provas). |

Estrutura de suporte:

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

| Pólo | Tipo | Nomeação |
|-------|------|-----------|
| `suffix_id` | `u16` | Первичный ключ; стабилен между версиями политики. |
| `suffix` | `AsciiString` | por exemplo, `sora`. |
| `steward` | `AccountId` | Steward, определенный na carta de governança. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Liquidação ativa por умолчанию (por exemplo `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | A confiança é feita por níveis e serviços de entrega. |
| `min_term_years` | `u8` | O valor mínimo de substituição não permite substituições de camada. |
| `grace_period_days` | `u16` | Padrão 30. |
| `redemption_period_days` | `u16` | Padrão 60. |
| `max_term_years` | `u8` | Максимальная длительность предоплаты. |
| `referral_cap_bps` | `u16` | <=1000 (10%) por fretamento. |
| `reserved_labels` | `Vec<ReservedNameV1>` | A descrição da governança com instruções de governança. |
| `fee_split` | `SuffixFeeSplitV1` | Доли tesouraria / administrador / referência (pontos básicos). |
| `fund_splitter_account` | `AccountId` | Garantia de conta + crédito garantido. |
| `policy_version` | `u16` | Увеличивается при каждом изменении. |
| `metadata` | `Metadata` | Расширенные заметки (acordo de KPI, documentos de hashes para conformidade). |

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

### 2.3 Informações sobre pagamentos e liquidação| Estrutura | Política | Atualizado |
|--------|------|------------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Детерминированная запись распределенных выплат по эпохам liquidação (неделя). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Эмитируется при каждом платеже (registro, renovação, leilão). |

O polígono `TokenValue` usa a codificação física canônica do Norito com o código de valores de связанного `SuffixPolicyV1`.

### 2.4 События реестра

A ferramenta canônica fornece log de repetição para DNS/gateway automatizados e analíticos.

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

События должны добавляться в replayable log (por exemplo, domínio `RegistryEvents`) e зеркалироваться в gateway feeds, чтобы caches DNS invalidado no SLA fornecido.

## 3. Layout de layout e índices

| Ключ | Descrição |
|-----|----------|
| `Names::<name_hash>` | Selecione o cartão `name_hash` -> `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Вторичный индекс для carteira UI (paginação amigável). |
| `NamesByLabel::<suffix_id, normalized_label>` | Ao abrir os conectores, determine a situação. |
| `SuffixPolicies::<suffix_id>` | Atual `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | História `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Log somente anexado com uma configuração mono. |

Existem tuplas seriais Norito para determinar o valor da configuração. Os índices de segurança serão obtidos automaticamente com o registro correto.

## 4. Máquina de ciclo de armazenamento

| Composição | Условия входа | Pré-requisitos de entrega | Nomeação |
|-------|----------------|----------|-----------|
| Disponível | Por favor, verifique `NameRecord`. | `PendingAuction` (premium), `Active` (registro padrão). | Поиск доступности читает только индексы. |
| Leilão Pendente | Создается, когда `PriceTierV1.auction_kind` != nenhum. | `Active` (liquidação do leilão), `Tombstoned` (sem lances). | Auctions эмитируют `AuctionOpened` e `AuctionSettled`. |
| Ativo | Регистрация или продление успешно. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` foi instalado novamente. |
| Período de graça | Automático por `now > expires_at`. | `Active` (renovação dentro do prazo), `Redemption`, `Tombstoned`. | Padrão +30 dias; резолвится, não há problema. |
| Redenção | `now > grace_expires_at` e `< redemption_expires_at`. | `Active` (renovação tardia), `Tombstoned`. | Команды требуют штрафного платежа. |
| Congelado | Congelar a governança ou guardião. | `Active` (para correção), `Tombstoned`. | Não instale ou desative os controladores. |
| Lápide | É uma questão de redenção, isto é uma recompensa ou uma redenção. | `PendingAuction` (reabertura holandesa) или остается marcado para lápide. | A chave `NameTombstoned` está instalada corretamente. |Переходы состояний ДОЛЖНЫ эмитировать соответствующий `RegistryEventKind`, чтобы downstream caches оставались согласованными. Tombstoned имена, входящие в Dutch reabrir аукционы, прикрепляют payload `AuctionKind::DutchReopen`.

## 5. Gateway de conexão e sincronização canônica

Os gateways são enviados para `RegistryEventV1` e sincronizados DNS/SoraFS, por exemplo:

1. Verifique o `NameRecordV1` para que você possa usar o produto.
2. Modelos de resolução de configuração (i105 предпочтительно + compactado (`sora`) как второй выбор, registros de texto).
3. Fixe o fluxo de trabalho SoraDNS em [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Garantia de envio:

- Каждая транзакция, влияющая на `NameRecordV1`, *должна* добавить ровно одно событие со строго возрастающей `version`.
- `RevenueSharePosted` события сссылаются em assentamentos ou `RevenueShareRecordV1`.
- События congelar/descongelar/tombstone включают хеши governança артефактов в `metadata` para auditoria de repetição.

## 6. Exemplos de cargas úteis Norito

### 6.1 Exemplo NameRecord

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

### 6.2 Exemplo SuffixPolicy

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

## 7. Следующие шаги

- **SN-2b (API do registrador e ganchos de governança):** открыть эти structs через Torii (Norito e ligações JSON) e привязать verificações de admissão no artefato de governança.
- **SN-3 (mecanismo de leilão e registro):** переиспользовать `NameAuctionStateV1` para lógica de confirmação/revelação e reabertura holandesa.
- **SN-5 (Pagamento e liquidação):** использовать `RevenueShareRecordV1` para serviços financeiros e automáticos.

Вопросы e запросы на изменения следует фиксировать вместе обновлениями SNS roadmap em `roadmap.md` e отражать в `status.md` por conta própria.