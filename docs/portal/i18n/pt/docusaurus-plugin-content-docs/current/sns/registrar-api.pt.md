---
lang: pt
direction: ltr
source: docs/portal/docs/sns/registrar-api.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Esta página espelha `docs/source/sns/registrar_api.md` e agora serve como a
cópia canônica do portal. O arquivo fonte permanece para fluxos de tradução.
:::

# API do registrador SNS e ganchos de governança (SN-2b)

**Status:** Redigido 2026-03-24 -- sob revisão do Nexus Core  
**Link do roteiro:** SN-2b "API do registrador e ganchos de governança"  
**Pré-requisitos:** Definições de esquema em [`registry-schema.md`](./registry-schema.md)

Esta nota especifica os endpoints Torii, serviços gRPC, DTOs de requisição/resposta e
parceiros de governança necessários para operar o registrador do Sora Name Service (SNS).
E o contrato autoritativo para SDKs, carteiras e automação que precisam de registrador,
renovar ou gerenciar nomes SNS.

## 1. Transporte e autenticação

| Requisito | Detalhe |
|-----------|---------|
| Protocolos | REST sob `/v2/sns/*` e serviço gRPC `sns.v1.Registrar`. Ambos aceitam Norito-JSON (`application/json`) e Norito-RPC binário (`application/x-norito`). |
| Autenticação | Tokens `Authorization: Bearer` ou certificados mTLS emitidos por sufixo steward. Endpoints sensíveis a governança (congelar/descongelar, atribuições reservadas) desativar `scope=sns.admin`. |
| Limites de taxa | Registradores agrupam os buckets `torii.preauth_scheme_limits` com chamadores JSON mais limites de burst por sufixo: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Telemetria | Torii expoe `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` para manipuladores do registrador (filtrar `scheme="norito_rpc"`); a API também incrementa `sns_registrar_status_total{result, suffix_id}`. |

## 2. Visão geral do DTO

Os campos referenciam as estruturas canônicas definidas em [`registry-schema.md`](./registry-schema.md). Todas as cargas incluem `NameSelectorV1` + `SuffixId` para evitar roteamento ambíguo.

```text
Struct RegisterNameRequestV1 {
    selector: NameSelectorV1,
    owner: AccountId,
    controllers: Vec<NameControllerV1>,
    term_years: u8,                     // 1..=max_term_years
    pricing_class_hint: Option<u8>,     // steward-advertised tier id
    payment: PaymentProofV1,
    governance: GovernanceHookV1,
    metadata: Metadata,
}

Struct RegisterNameResponseV1 {
    name_record: NameRecordV1,
    registry_event: RegistryEventV1,
    revenue_accrual: RevenueAccrualEventV1,
}

Struct PaymentProofV1 {
    asset_id: AssetId,
    gross_amount: TokenValue,
    net_amount: TokenValue,
    settlement_tx: Hash,
    payer: AccountId,
    signature: Signature,               // steward/treasury cosign
}

Struct GovernanceHookV1 {
    proposal_id: String,
    council_vote_hash: Hash,
    dao_vote_hash: Hash,
    steward_ack: Signature,
    guardian_clearance: Option<Signature>,
}

Struct RenewNameRequestV1 {
    selector: NameSelectorV1,
    term_years: u8,
    payment: PaymentProofV1,
}

Struct TransferNameRequestV1 {
    selector: NameSelectorV1,
    new_owner: AccountId,
    governance: GovernanceHookV1,
}

Struct UpdateControllersRequestV1 {
    selector: NameSelectorV1,
    controllers: Vec<NameControllerV1>,
}

Struct FreezeNameRequestV1 {
    selector: NameSelectorV1,
    reason: String,
    until: Timestamp,
    guardian_ticket: Signature,
}

Struct ReservedAssignmentRequestV1 {
    selector: NameSelectorV1,
    reserved_label: ReservedNameV1,
    governance: GovernanceHookV1,
}
```

## 3. Terminais REST

| Ponto final | Método | Carga útil | Descrição |
|----------|--------|---------|-----------|
| `/v2/sns/registrations` | POSTAR | `RegisterNameRequestV1` | Registrador ou reabrir um nome. Resolver o nível de preços, validar provas de pagamento/governança, emitir eventos de registro. |
| `/v2/sns/registrations/{selector}/renew` | POSTAR | `RenewNameRequestV1` | Estende o termo. Aplica janelas de graça/redenção da política. |
| `/v2/sns/registrations/{selector}/transfer` | POSTAR | `TransferNameRequestV1` | Transfere propriedade quando aprovações de governança forem anexas. |
| `/v2/sns/registrations/{selector}/controllers` | COLOCAR | `UpdateControllersRequestV1` | Substitui o conjunto de controladores; valida endereços de contatos assinados. |
| `/v2/sns/registrations/{selector}/freeze` | POSTAR | `FreezeNameRequestV1` | Congelar o guardião/conselho. Solicite ticket de guardião e referência ao súmula de governança. |
| `/v2/sns/registrations/{selector}/freeze` | EXCLUIR | `GovernanceHookV1` | Descongelar após remediação; garantia override do conselho registrado. |
| `/v2/sns/reserved/{selector}` | POSTAR | `ReservedAssignmentRequestV1` | Atribuição de nomes reservados por administrador/conselho. |
| `/v2/sns/policies/{suffix_id}` | OBTER | -- | Busca `SuffixPolicyV1` atual (cacheavel). |
| `/v2/sns/registrations/{selector}` | OBTER | -- | Retorna `NameRecordV1` atual + estado efetivo (Active, Grace, etc.). |**Codificação de seletor:** o segmento `{selector}` aceita I105, comprimido ou hex canônico conforme ADDR-5; Torii normaliza via `NameSelectorV1`.

**Modelo de erros:** todos os endpoints retornam Norito JSON com `code`, `message`, `details`. Os códigos incluem `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Helpers CLI (requisito de registrador manual N0)

Stewards de beta fechado agora podem operar o registrador via CLI sem montar JSON manualmente:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id xor#sora \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- padrão `--owner` e conta de configuração da CLI; repita `--controller` para fixação de contas controlador adicionais (padrão `[owner]`).
- As bandeiras inline de pagamento mapeiam direto para `PaymentProofV1`; Passe `--payment-json PATH` quando você tiver um recibo estruturado. Metadados (`--metadata-json`) e ganchos de governança (`--governance-json`) seguem o mesmo padrão.

Helpers de leitura somente completam os ensaios:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Veja `crates/iroha_cli/src/commands/sns.rs` para implementação; os comandos reutilizam os DTOs Norito descritos neste documento para que a dita da CLI coincida byte por byte com as respostas do Torii.

Helpers adicionais cobrem renovações, transferências e ações de guardião:

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id xor#sora \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership once governance approves
iroha sns transfer \
  --selector makoto.sora \
  --new-owner i105... \
  --governance-json /path/to/hook.json

# Freeze/unfreeze flows
iroha sns freeze \
  --selector makoto.sora \
  --reason "guardian investigation" \
  --until-ms 1750000000000 \
  --guardian-ticket '{"sig":"guardian"}'

iroha sns unfreeze \
  --selector makoto.sora \
  --governance-json /path/to/unfreeze_hook.json
```

`--governance-json` deve conter um registro `GovernanceHookV1` valido (id da proposta, hashes de voto, assinaturas do administrador/responsável). Cada comando simplesmente espelha o endpoint `/v2/sns/registrations/{selector}/...` correspondente para que os operadores de beta ensaiem exatamente como as superfícies Torii que os SDKs chamamao.

## 4. Serviço gRPC

```text
service Registrar {
    rpc Register(RegisterNameRequestV1) returns (RegisterNameResponseV1);
    rpc Renew(RenewNameRequestV1) returns (NameRecordV1);
    rpc Transfer(TransferNameRequestV1) returns (NameRecordV1);
    rpc UpdateControllers(UpdateControllersRequestV1) returns (NameRecordV1);
    rpc Freeze(FreezeNameRequestV1) returns (NameRecordV1);
    rpc Unfreeze(GovernanceHookV1) returns (NameRecordV1);
    rpc AssignReserved(ReservedAssignmentRequestV1) returns (NameRecordV1);
    rpc GetRegistration(NameSelectorV1) returns (NameRecordV1);
    rpc GetPolicy(SuffixId) returns (SuffixPolicyV1);
}
```

Wire-format: hash do esquema Norito em tempo de compilação registrada em
`fixtures/norito_rpc/schema_hashes.json` (linhas `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, etc.).

## 5. Ganchos de governança e evidências

Toda chamada que altera estado deve garantir evidências adequadas para reprodução:

| Ação | Dados de governança necessários |
|------|------------------------------------------|
| Registro/renovação padrão | Prova de pagamento referenciando uma instrução de liquidação; não exige voto do conselho a menos que o tier exija aprovação do mordomo. |
| Registo de nível premium / atribuição reservada | `GovernanceHookV1` referenciando id da proposta + reconhecimento do administrador. |
| Transferência | Hash de voto do conselho + hash de sinal DAO; autorização de tutela quando a transferência é acionada por resolução de disputa. |
| Congelar/descongelar | Assinatura do ticket Guardian mais override do Council (descongelar). |

Torii verifica as provas conferindo:

1. A proposta id não existe ledger de governança (`/v2/governance/proposals/{id}`) e o status e `Approved`.
2. Hashes solicitados aos representantes de voto registrados.
3. Assinaturas de mordomo/tutor referenciam as chaves públicas esperadas de `SuffixPolicyV1`.

Falhas retornam `sns_err_governance_missing`.

## 6. Exemplos de fluxo de trabalho

### 6.1 Registro padrão1. O cliente consulte `/v2/sns/policies/{suffix_id}` para obter preços, graça e níveis disponíveis.
2. O cliente monta `RegisterNameRequestV1`:
   - `selector` derivado do rótulo I105 (preferido) ou comprimido (segunda melhor opção).
   - `term_years` dentro dos limites da política.
   - `payment` referenciando a transferência do splitter tesouraria/steward.
3. Validação Torii:
   - Normalização de etiqueta + lista reservada.
   - Prazo/preço bruto vs `PriceTierV1`.
   - Valor da prova de pagamento >= pré-cálculo + taxas.
4. Em sucesso Torii:
   - Persistir `NameRecordV1`.
   - Emita `RegistryEventV1::NameRegistered`.
   - Emita `RevenueAccrualEventV1`.
   - Retorno ao novo registro + eventos.

### 6.2 Renovação durante a graça

As renovações durante a graça incluem a requisição padrão mais detecção de tempestade:

- Torii compara `now` vs `grace_expires_at` e adiciona tabelas de sobretaxa de `SuffixPolicyV1`.
- A prova de pagamento deve cobrir a sobretaxa. Falha => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` registra o novo `expires_at`.

### 6.3 Congelar o guardião e substituir o conselho

1. Guardian envia `FreezeNameRequestV1` com ticket referenciando id de incidente.
2. Torii mova o registro para `NameStatus::Frozen`, emita `NameFrozen`.
3. Após remediação, o conselho emite override; o operador envia DELETE `/v2/sns/registrations/{selector}/freeze` com `GovernanceHookV1`.
4. Torii valida ou substitui, emite `NameUnfrozen`.

## 7. Validação e códigos de erro

| Código | Descrição | http |
|--------|-----------|------|
| `sns_err_reserved` | Etiqueta reservada ou bloqueada. | 409 |
| `sns_err_policy_violation` | Termo, nível ou conjunto de controladores viola a política. | 422 |
| `sns_err_payment_mismatch` | Incompatibilidade de valor ou ativo na prova de pagamento. | 402 |
| `sns_err_governance_missing` | Artefatos de governança exigidos ausentes/inválidos. | 403 |
| `sns_err_state_conflict` | Operação não permitida no estado atual do ciclo de vida. | 409 |

Todos os códigos aparecem via `X-Iroha-Error-Code` e envelopes Norito JSON/NRPC estruturados.

## 8. Notas de implementação

- Torii armazena leiloes pendentes em `NameRecordV1.auction` e tentativas de registro direto enquanto estiver `PendingAuction`.
- Provas de pagamento reutilizam recibos do razão Norito; serviços de tesouraria fornecem APIs helper (`/v2/finance/sns/payments`).
- SDKs envolvem esses endpoints com helpers fortemente tipados para que carteiras apresentem motivos claros de erro (`ERR_SNS_RESERVED`, etc.).

## 9. Próximos passos

- Conectar os manipuladores do Torii ao contrato de registro real quando os leiloes SN-3 chegarem.
- Publicar guias específicos de SDK (Rust/JS/Swift) referenciando esta API.
- Estender [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) com links cruzados para os campos de evidência de governança hook.