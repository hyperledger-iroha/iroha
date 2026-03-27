---
lang: pt
direction: ltr
source: docs/portal/docs/sns/registrar-api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 14373f09e9a691a910fbde08548c5fdfe03581049c85af425c27c94fc4fcafd8
source_last_modified: "2025-11-10T20:06:34.010395+00:00"
translation_last_reviewed: 2026-01-01
---

:::note Fonte canonica
Esta pagina espelha `docs/source/sns/registrar_api.md` e agora serve como a
copia canonica do portal. O arquivo fonte permanece para fluxos de traducao.
:::

# API do registrar SNS e hooks de governanca (SN-2b)

**Status:** Redigido 2026-03-24 -- sob revisao do Nexus Core  
**Link do roadmap:** SN-2b "Registrar API & governance hooks"  
**Pre-requisitos:** Definicoes de esquema em [`registry-schema.md`](./registry-schema.md)

Esta nota especifica os endpoints Torii, servicos gRPC, DTOs de requisicao/resposta e
artefatos de governanca necessarios para operar o registrar do Sora Name Service (SNS).
E o contrato autoritativo para SDKs, wallets e automacao que precisam registrar,
renovar ou gerenciar nomes SNS.

## 1. Transporte e autenticacao

| Requisito | Detalhe |
|-----------|---------|
| Protocolos | REST sob `/v1/sns/*` e servico gRPC `sns.v1.Registrar`. Ambos aceitam Norito-JSON (`application/json`) e Norito-RPC binario (`application/x-norito`). |
| Auth | Tokens `Authorization: Bearer` ou certificados mTLS emitidos por suffix steward. Endpoints sensiveis a governanca (freeze/unfreeze, atribuicoes reservadas) exigem `scope=sns.admin`. |
| Limites de taxa | Registrars compartilham os buckets `torii.preauth_scheme_limits` com chamadores JSON mais limites de burst por suffix: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Telemetria | Torii expoe `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` para handlers do registrar (filtrar `scheme="norito_rpc"`); a API tambem incrementa `sns_registrar_status_total{result, suffix_id}`. |

## 2. Visao geral de DTO

Os campos referenciam os structs canonicos definidos em [`registry-schema.md`](./registry-schema.md). Todas as cargas incluem `NameSelectorV1` + `SuffixId` para evitar roteamento ambiguo.

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

## 3. Endpoints REST

| Endpoint | Metodo | Payload | Descricao |
|----------|--------|---------|-----------|
| `/v1/sns/names` | POST | `RegisterNameRequestV1` | Registrar ou reabrir um nome. Resolve o tier de precos, valida provas de pagamento/governanca, emite eventos de registro. |
| `/v1/sns/names/{namespace}/{literal}/renew` | POST | `RenewNameRequestV1` | Estende o termo. Aplica janelas de grace/redemption da politica. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POST | `TransferNameRequestV1` | Transfere propriedade quando aprovacoes de governanca forem anexadas. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | PUT | `UpdateControllersRequestV1` | Substitui o conjunto de controllers; valida enderecos de conta assinados. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POST | `FreezeNameRequestV1` | Freeze de guardian/council. Requer ticket guardian e referencia ao docket de governanca. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | DELETE | `GovernanceHookV1` | Unfreeze apos remediacao; garante override do council registrado. |
| `/v1/sns/reserved/{selector}` | POST | `ReservedAssignmentRequestV1` | Atribuicao de nomes reservados por steward/council. |
| `/v1/sns/policies/{suffix_id}` | GET | -- | Busca `SuffixPolicyV1` atual (cacheavel). |
| `/v1/sns/names/{namespace}/{literal}` | GET | -- | Retorna `NameRecordV1` atual + estado efetivo (Active, Grace, etc.). |

**Codificacao de selector:** o segmento `{selector}` aceita i105, comprimido ou hex canonico conforme ADDR-5; Torii normaliza via `NameSelectorV1`.

**Modelo de erros:** todos os endpoints retornam Norito JSON com `code`, `message`, `details`. Os codigos incluem `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Helpers CLI (requisito de registrar manual N0)

Stewards de beta fechada agora podem operar o registrar via CLI sem montar JSON manualmente:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` padrao e a conta de configuracao da CLI; repita `--controller` para anexar contas controller adicionais (padrao `[owner]`).
- Os flags inline de pagamento mapeiam direto para `PaymentProofV1`; passe `--payment-json PATH` quando voce ja tiver um recibo estruturado. Metadados (`--metadata-json`) e hooks de governanca (`--governance-json`) seguem o mesmo padrao.

Helpers de leitura somente completam os ensaios:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Veja `crates/iroha_cli/src/commands/sns.rs` para a implementacao; os comandos reutilizam os DTOs Norito descritos neste documento para que a saida da CLI coincida byte por byte com as respostas do Torii.

Helpers adicionais cobrem renovacoes, transferencias e acoes de guardian:

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership once governance approves
iroha sns transfer \
  --selector makoto.sora \
  --new-owner <i105-account-id> \
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

`--governance-json` deve conter um registro `GovernanceHookV1` valido (proposal id, vote hashes, assinaturas steward/guardian). Cada comando simplesmente espelha o endpoint `/v1/sns/names/{namespace}/{literal}/...` correspondente para que operadores de beta ensaiem exatamente as superficies Torii que os SDKs chamarao.

## 4. Servico gRPC

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

Wire-format: hash do esquema Norito em tempo de compilacao registrado em
`fixtures/norito_rpc/schema_hashes.json` (linhas `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, etc.).

## 5. Hooks de governanca e evidencias

Toda chamada que altera estado deve anexar evidencias adequadas para replay:

| Acao | Dados de governanca requeridos |
|------|-------------------------------|
| Registro/renovacao padrao | Prova de pagamento referenciando uma instrucao de settlement; nao exige voto do council a menos que o tier exija aprovacao do steward. |
| Registro de tier premium / atribuicao reservada | `GovernanceHookV1` referenciando proposal id + steward acknowledgement. |
| Transferencia | Hash de voto do council + hash de sinal DAO; guardian clearance quando a transferencia e acionada por resolucao de disputa. |
| Freeze/Unfreeze | Assinatura do ticket guardian mais override do council (unfreeze). |

Torii verifica as provas conferindo:

1. Proposal id existe no ledger de governanca (`/v1/governance/proposals/{id}`) e o status e `Approved`.
2. Hashes correspondem aos artefatos de voto registrados.
3. Assinaturas steward/guardian referenciam as chaves publicas esperadas de `SuffixPolicyV1`.

Falhas retornam `sns_err_governance_missing`.

## 6. Exemplos de fluxo de trabalho

### 6.1 Registro padrao

1. O cliente consulta `/v1/sns/policies/{suffix_id}` para obter precos, grace e tiers disponiveis.
2. O cliente monta `RegisterNameRequestV1`:
   - `selector` derivado de label i105 (preferido) ou comprimido (segunda melhor opcao).
   - `term_years` dentro dos limites da politica.
   - `payment` referenciando a transferencia do splitter tesouraria/steward.
3. Torii valida:
   - Normalizacao de label + lista reservada.
   - Term/gross price vs `PriceTierV1`.
   - Prova de pagamento amount >= preco calculado + fees.
4. Em sucesso Torii:
   - Persiste `NameRecordV1`.
   - Emite `RegistryEventV1::NameRegistered`.
   - Emite `RevenueAccrualEventV1`.
   - Retorna o novo registro + eventos.

### 6.2 Renovacao durante grace

Renovacoes durante grace incluem a requisicao padrao mais deteccao de penalidade:

- Torii compara `now` vs `grace_expires_at` e adiciona tabelas de sobretaxa de `SuffixPolicyV1`.
- A prova de pagamento deve cobrir a sobretaxa. Falha => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` registra o novo `expires_at`.

### 6.3 Freeze de guardian e override do council

1. Guardian envia `FreezeNameRequestV1` com ticket referenciando id de incidente.
2. Torii move o registro para `NameStatus::Frozen`, emite `NameFrozen`.
3. Apos remediacao, o council emite override; o operador envia DELETE `/v1/sns/names/{namespace}/{literal}/freeze` com `GovernanceHookV1`.
4. Torii valida o override, emite `NameUnfrozen`.

## 7. Validacao e codigos de erro

| Codigo | Descricao | HTTP |
|--------|-----------|------|
| `sns_err_reserved` | Label reservado ou bloqueado. | 409 |
| `sns_err_policy_violation` | Termo, tier ou conjunto de controllers viola a politica. | 422 |
| `sns_err_payment_mismatch` | Mismatch de valor ou asset na prova de pagamento. | 402 |
| `sns_err_governance_missing` | Artefatos de governanca requeridos ausentes/invalidos. | 403 |
| `sns_err_state_conflict` | Operacao nao permitida no estado atual do ciclo de vida. | 409 |

Todos os codigos aparecem via `X-Iroha-Error-Code` e envelopes Norito JSON/NRPC estruturados.

## 8. Notas de implementacao

- Torii armazena leiloes pendentes em `NameRecordV1.auction` e rejeita tentativas de registro direto enquanto estiver `PendingAuction`.
- Provas de pagamento reutilizam recibos do ledger Norito; servicos de tesouraria fornecem APIs helper (`/v1/finance/sns/payments`).
- SDKs devem envolver esses endpoints com helpers fortemente tipados para que wallets apresentem motivos claros de erro (`ERR_SNS_RESERVED`, etc.).

## 9. Proximos passos

- Conectar os handlers do Torii ao contrato de registro real quando os leiloes SN-3 chegarem.
- Publicar guias especificos de SDK (Rust/JS/Swift) referenciando esta API.
- Estender [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) com links cruzados para os campos de evidencia de governance hook.
