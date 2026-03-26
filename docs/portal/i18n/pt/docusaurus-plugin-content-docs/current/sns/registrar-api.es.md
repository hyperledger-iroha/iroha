---
lang: pt
direction: ltr
source: docs/portal/docs/sns/registrar-api.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Esta página reflete `docs/source/sns/registrar_api.md` e agora sirve como la
cópia canônica do portal. O arquivo fonte é mantido para fluxos de
tradução.
:::

# API do registrador SNS e ganchos de governo (SN-2b)

**Estado:** Borrador 2026-03-24 -- revisão inferior do Nexus Core  
**Incluir roteiro:** SN-2b "API do registrador e ganchos de governança"  
**Pré-requisitos:** Definições de esquema em [`registry-schema.md`](./registry-schema.md)

Esta nota especifica os endpoints Torii, serviços gRPC, DTOs de solicitação
respostas e artefatos de governo necessários para operar o registrador do
Serviço de nomes Sora (SNS). É o contrato autoritativo para SDKs, carteiras e
automatização que precisa registrar, renovar ou gerenciar nomes SNS.

## 1. Transporte e autenticação

| Requisito | Detalhes |
|-----------|---------|
| Protocolos | REST abaixo `/v1/sns/*` e serviço gRPC `sns.v1.Registrar`. Ambos aceitam Norito-JSON (`application/json`) e Norito-RPC binário (`application/x-norito`). |
| Autenticação | Tokens `Authorization: Bearer` ou certificados mTLS emitidos por sufixo steward. Endpoints sensíveis a governança (congelar/descongelar, atribuições reservadas) requerem `scope=sns.admin`. |
| Limites de tasa | Os registradores comparam os buckets `torii.preauth_scheme_limits` com chamadores JSON mas limites de rafaga por sufixo: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Telemetria | Torii expõe `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` para manipuladores do registrador (filtrar `scheme="norito_rpc"`); a API também incrementa `sns_registrar_status_total{result, suffix_id}`. |

## 2. Resumo do DTO

Os campos referenciam as estruturas canônicas definidas em [`registry-schema.md`](./registry-schema.md). Todas as cargas incluem `NameSelectorV1` + `SuffixId` para evitar ambiguidade de rota.

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
|----------|--------|---------|-------------|
| `/v1/sns/names` | POSTAR | `RegisterNameRequestV1` | Registrador ou reabrir um nome. Resuelve la tier de precios, valida pruebas de pago/gobernanza, emite eventos de registro. |
| `/v1/sns/names/{namespace}/{literal}/renew` | POSTAR | `RenewNameRequestV1` | Estenda o término. Aplica janelas de graça/redenção segundo a política. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POSTAR | `TransferNameRequestV1` | Transfiere propiedad uma vez junto com as aprovações de governo. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | COLOCAR | `UpdateControllersRequestV1` | Substitua o conjunto de controladores; valida direções de conta firmadas. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POSTAR | `FreezeNameRequestV1` | Congelar o guardião/conselho. Requer ticket de guardião e referência ao docket de gobernanza. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | EXCLUIR | `GovernanceHookV1` | Descongelar remediação de tras; asegura a substituição do conselho registrado. |
| `/v1/sns/reserved/{selector}` | POSTAR | `ReservedAssignmentRequestV1` | Atribuição de nomes reservados pelo administrador/conselho. |
| `/v1/sns/policies/{suffix_id}` | OBTER | -- | Obtenha `SuffixPolicyV1` real (armazenável em cache). |
| `/v1/sns/names/{namespace}/{literal}` | OBTER | -- | Devuelve `NameRecordV1` atual + estado efectivo (Ativo, Graça, etc.). |**Codificação do seletor:** o segmento `{selector}` aceita i105, comprimido ou hexadecimal canônico após ADDR-5; Torii normaliza via `NameSelectorV1`.

**Modelo de erros:** todos os endpoints retornam Norito JSON com `code`, `message`, `details`. Os códigos incluem `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Helpers CLI (requisito do manual do registrador N0)

Os administradores da versão beta fechada podem operar o registrador via CLI sem armar JSON manualmente:

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

- `--owner` toma por defeito a conta de configuração da CLI; repita `--controller` para adicionar informações adicionais ao controlador (padrão `[owner]`).
- As bandeiras inline de pagamento mapeadas direto para `PaymentProofV1`; usa `--payment-json PATH` quando você tiver um recibo estruturado. Metadados (`--metadata-json`) e ganchos de governança (`--governance-json`) seguem o mesmo patrono.

Ajudantes de solo lectura completando os ensaios:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Ver `crates/iroha_cli/src/commands/sns.rs` para implementação; Os comandos reutilizam os DTOs Norito descritos neste documento para que a saída da CLI coincida byte por byte com as respostas de Torii.

Ajudantes adicionais para renovações, transferências e ações de guardião:

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

`--governance-json` deve conter um registro `GovernanceHookV1` válido (ID da proposta, hashes de voto, firmas de administrador/guardião). Cada comando simplesmente reflete o endpoint `/v1/sns/names/{namespace}/{literal}/...` correspondente para que os operadores de beta ensayem exatamente as superfícies Torii que ligam os SDKs.

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

Wire-format: hash do esquema Norito no tempo de compilação registrada em
`fixtures/norito_rpc/schema_hashes.json` (filas `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, etc.).

## 5. Ganchos de governança e evidência

Cada chamada que muda de estado deve ser adjunta de evidência adequada para reprodução:

| Ação | Dados de governança exigidos |
|--------|------------------------------------------|
| Registro/renovação padrão | Teste de pagamento que se refere a uma instrução de liquidação; não se exige voto do conselho salvo que la tier exige aprovação do administrador. |
| Registro premium / atribuição reservada | `GovernanceHookV1` que referencia o id da proposta + reconhecimento do administrador. |
| Transferência | Hash de voto do conselho + hash de senal DAO; autorização do guardião quando a transferência é ativada para resolução de disputa. |
| Congelar/descongelar | Firma de ticket Guardian mas override del Council (descongelar). |

Torii verifica os testes verificados:

1. O ID da proposta existe no razão de governo (`/v1/governance/proposals/{id}`) e o status é `Approved`.
2. Os hashes coincidem com os artefatos de voto registrados.
3. Firmas de steward/guardian referenciam as claves públicas esperadas de `SuffixPolicyV1`.

Fallos devuelven `sns_err_governance_missing`.

## 6. Exemplos de fluxo

### 6.1 Registro padrão1. O cliente consulte `/v1/sns/policies/{suffix_id}` para obter preços, graça e níveis disponíveis.
2. Arma cliente `RegisterNameRequestV1`:
   - `selector` derivado do rótulo i105 (preferido) ou comprimido (segunda melhor opção).
   - `term_years` dentro dos limites da política.
   - `payment` que referencia a transferência do splitter tesoreria/steward.
3. Validação Torii:
   - Normalização de etiqueta + lista reservada.
   - Prazo/preço bruto vs `PriceTierV1`.
   - Valor do comprovante de pagamento >= preço calculado + taxas.
4. Na saída Torii:
   - Persistir `NameRecordV1`.
   - Emita `RegistryEventV1::NameRegistered`.
   - Emita `RevenueAccrualEventV1`.
   - Retorne o novo registro + eventos.

### 6.2 Renovação durante graça

Las renovaciones en gracia incluem a solicitação padrão, mas a detecção de penalidade:

- Torii compara `now` vs `grace_expires_at` e adiciona tabelas de recarga de `SuffixPolicyV1`.
- A verificação de pagamento deve ser realizada. Fallo => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` registra o novo `expires_at`.

### 6.3 Congelamento do guardião e substituição do conselho

1. Guardian envia `FreezeNameRequestV1` com ticket que refere id de incidente.
2. Torii mova o registro para `NameStatus::Frozen`, emita `NameFrozen`.
3. Tras remediação, el conselho emite override; o operador envia DELETE `/v1/sns/names/{namespace}/{literal}/freeze` com `GovernanceHookV1`.
4. Torii valida a substituição, emite `NameUnfrozen`.

## 7. Validação e códigos de erro

| Código | Descrição | http |
|--------|-------------|------|
| `sns_err_reserved` | Etiqueta reservada ou bloqueada. | 409 |
| `sns_err_policy_violation` | Prazo, nível ou conjunto de controladores viola a política. | 422 |
| `sns_err_payment_mismatch` | Incompatibilidade de valor ou ativo na tentativa de pagamento. | 402 |
| `sns_err_governance_missing` | Artefatos de governo exigidos ausentes/inválidos. | 403 |
| `sns_err_state_conflict` | Operação não permitida no estado atual do ciclo de vida. | 409 |

Todos os códigos são vendidos via `X-Iroha-Error-Code` e envelopes Norito JSON/NRPC estruturados.

## 8. Notas de implementação

- Torii guarda subastas pendentes em `NameRecordV1.auction` e rechaza intenções de registro diretamente enquanto estiver em `PendingAuction`.
- As verificações de pagamento reutilizam recibos do livro-razão Norito; serviços de tesouraria comprovados APIs helper (`/v1/finance/sns/payments`).
- Os SDKs devem envolver esses endpoints com ajudantes tipados para que as carteiras mostrem motivos claros de erro (`ERR_SNS_RESERVED`, etc.).

## 9. Próximos passos

- Conecte os manipuladores de Torii ao contrato de registro real uma vez que conecte as subastas SN-3.
- Publicar guias específicos de SDK (Rust/JS/Swift) que fazem referência a esta API.
- Extender [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) com links cruzados nos campos de evidência de governança gancho.