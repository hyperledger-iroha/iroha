---
lang: pt
direction: ltr
source: docs/portal/docs/sns/registrar-api.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Esta página reflete `docs/source/sns/registrar_api.md` e é desormada de
cópia canônica do portal. O arquivo fonte é conservado para o fluxo de
tradução.
:::

# API do registrador SNS e ganchos de governança (SN-2b)

**Estatuto:** Redige 2026-03-24 -- en revue par Nexus Core  
**Roteiro de garantia:** SN-2b "API do registrador e ganchos de governança"  
**Pré-requisitos:** Definições de esquema em [`registry-schema.md`](./registry-schema.md)

Esta nota especifica os endpoints Torii, serviços gRPC, DTOs de solicitação/resposta
e artefatos de governo necessários para operar o registrador do Sora Name
Serviço (SNS). Este é o contrato de referência para SDKs, carteiras e outros
automatização que deve registrar, renovar ou gerar nomes SNS.

## 1. Transporte e autenticação

| Exigência | Detalhe |
|----------|--------|
| Protocolos | REST sob `/v1/sns/*` e serviço gRPC `sns.v1.Registrar`. Os dois aceitam Norito-JSON (`application/json`) e o binário Norito-RPC (`application/x-norito`). |
| Autenticação | Jetons `Authorization: Bearer` ou certificados mTLS emis par suffix steward. Os endpoints sensíveis à governança (congelar/descongelar, afetações reservadas) exigem `scope=sns.admin`. |
| Limites de débito | Os registradores compartilham os buckets `torii.preauth_scheme_limits` com os recorrentes JSON mais os limites de rafale por sufixo: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Telemetria | Torii expõe `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` para os manipuladores do registrador (filtro `scheme="norito_rpc"`); A API é incrementada também em `sns_registrar_status_total{result, suffix_id}`. |

## 2. Abra o DTO

Os campos referem-se às estruturas canônicas definidas em [`registry-schema.md`](./registry-schema.md). Todas as taxas úteis são integrais `NameSelectorV1` + `SuffixId` para evitar um caminho ambíguo.

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
| `/v1/sns/names` | POSTAR | `RegisterNameRequestV1` | Registre ou registre um nome. Resout le tier de prix, valide les preuves de payement/gouvernance, emet des eventes de registre. |
| `/v1/sns/names/{namespace}/{literal}/renew` | POSTAR | `RenewNameRequestV1` | Prolongue o termo. Aplique as janelas de graça/redenção pela política. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POSTAR | `TransferNameRequestV1` | Transfira a propriedade uma vez que as aprovações de governo conjunto. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | COLOCAR | `UpdateControllersRequestV1` | Substitua o conjunto de controladores; valide os endereços dos signatários da conta. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POSTAR | `FreezeNameRequestV1` | Congelar guardião/conselho. Exige um guardião de ingressos e uma referência ao dossiê de governo. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | EXCLUIR | `GovernanceHookV1` | Descongelar apres remediação; certifique-se de que a substituição do conselho esteja registrada. |
| `/v1/sns/reserved/{selector}` | POSTAR | `ReservedAssignmentRequestV1` | Afetação de noms reservas por administrador/conselho. |
| `/v1/sns/policies/{suffix_id}` | OBTER | -- | Recupere o corrente `SuffixPolicyV1` (armazenável em cache). |
| `/v1/sns/names/{namespace}/{literal}` | OBTER | -- | Retourne le `NameRecordV1` courant + etat effectif (Active, Grace, etc.). |**Codificação do seletor:** o segmento `{selector}` aceita i105, comprimido ou hexadecimal canônico conforme ADDR-5; Torii normaliza via `NameSelectorV1`.

**Modelos de erros:** todos os endpoints retornam Norito JSON com `code`, `message`, `details`. Os códigos incluem `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Auxiliares CLI (exigência do registrador manual N0)

Os administradores da versão beta encerrada podem usar o registrador via CLI sem fabricar JSON no caminho principal:

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

- `--owner` assume por padrão o cálculo de configuração da CLI; repita `--controller` para adicionar controladores complementares de contas (por padrão `[owner]`).
- As bandeiras inline de pagamento mapeadas diretamente para `PaymentProofV1`; passe `--payment-json PATH` quando você deixar uma estrutura de recuperação. As metadonas (`--metadata-json`) e os ganchos de governo (`--governance-json`) seguem o esquema do meme.

Os auxiliares na palestra apenas completam as repetições:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Veja `crates/iroha_cli/src/commands/sns.rs` para implementação; Os comandos reutilizam os DTOs Norito escritos neste documento para que a classificação CLI corresponda byte para byte nas respostas Torii.

Os auxiliares complementares cobrem renovações, transferências e ações guardiãs:

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
  --new-owner soraカタカナ... \
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

`--governance-json` contém um registro `GovernanceHookV1` válido (id da proposta, hashes de voto, assinaturas do administrador/responsável). Este comando reflete simplesmente o endpoint `/v1/sns/names/{namespace}/{literal}/...` correspondente para que os operadores beta possam repetir exatamente as superfícies Torii que os SDKs chamam.

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

Formato de fio: hash do esquema Norito no momento da compilação registrada em
`fixtures/norito_rpc/schema_hashes.json` (linhas `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, etc.).

## 5. Ganchos de governo e segurança

Todas as chamadas que você modifica o estado devem juntar-se às recomendações reutilizáveis para a leitura:

| Ação | Requisitos de governo |
|--------|------------------------------------------|
| Norma de registo/renovação | Preuve de pagamento referente a uma instrução de liquidação; aucun voto conselho requis sauf si le tier exige l'approvation steward. |
| Registo de nível premium/reserva de afectação | `GovernanceHookV1` ID da proposta de referência + reconhecimento do administrador. |
| Transferência | Hash du vote Council + hash du signal DAO; guardião de autorização quando a transferência é cancelada por resolução de litígio. |
| Congelar/descongelar | Assinatura do guardião do bilhete mais substituição do conselho (descongelar). |

Torii verifique os requisitos e verifique:

1. O identificador da proposta existe no livro-razão de governo (`/v1/governance/proposals/{id}`) e o estatuto é `Approved`.
2. Os hashes correspondentes aos artefatos de votação registrados.
3. As assinaturas do administrador/guardião referem-se aos documentos públicos atendidos de `SuffixPolicyV1`.

Os controles enviados por echec `sns_err_governance_missing`.

## 6. Exemplos de fluxo de trabalho### 6.1 Padrão de registro

1. O cliente interroga `/v1/sns/policies/{suffix_id}` para recuperar o preço, a graça e os níveis disponíveis.
2. O cliente constrói `RegisterNameRequestV1`:
   - `selector` deriva do rótulo i105 (preferir) ou compresse (segunda escolha).
   - `term_years` nos limites da política.
   - `payment` refere-se à transferência do divisor de tesouraria/administrador.
3. Torii válido:
   - Normalização do rótulo + lista reservada.
   - Prazo/preço bruto vs `PriceTierV1`.
   - Montant de preuve de paiement >= cálculo de preço + frete.
4. Com sucesso Torii:
   - Persistir `NameRecordV1`.
   - Emet `RegistryEventV1::NameRegistered`.
   - Emet `RevenueAccrualEventV1`.
   - Retourne le nouveau record + eventos.

### 6.2 Renovação pendente la grace

As renovações pendentes da graça incluem a solicitação padrão e a detecção de penalidade:

- Torii compare `now` vs `grace_expires_at` e adicione as tabelas de sobretaxa de `SuffixPolicyV1`.
- A pré-pagamento deve ser cobrada sobretaxa. Echec => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` registre o novo `expires_at`.

### 6.3 Congelar guardião e substituir o conselho

1. Guardian soumet `FreezeNameRequestV1` com um ticket referente ao ID do incidente.
2. Torii substitua o registro em `NameStatus::Frozen`, emet `NameFrozen`.
3. Após a remediação, o conselho emet un override; O operador enviou DELETE `/v1/sns/names/{namespace}/{literal}/freeze` com `GovernanceHookV1`.
4. Torii valida a substituição, emet `NameUnfrozen`.

## 7. Validação e códigos de erro

| Código | Descrição | http |
|------|-------------|------|
| `sns_err_reserved` | Etiqueta reserva ou bloco. | 409 |
| `sns_err_policy_violation` | Termo, nível ou conjunto de controladores violam a política. | 422 |
| `sns_err_payment_mismatch` | Incompatibilidade de valor ou ativo na pré-pagamento. | 402 |
| `sns_err_governance_missing` | Artefactos de governo requisitados ausentes/inválidos. | 403 |
| `sns_err_state_conflict` | Operation non permise dans l'etat de cycle de vie actuel. | 409 |

Todos os códigos são exibidos via `X-Iroha-Error-Code` e os envelopes Norito estruturas JSON/NRPC.

## 8. Notas de implementação

- Torii armazene os leilões em atenção sob `NameRecordV1.auction` e rejeite as tentativas de registro direto como `PendingAuction`.
- As pré-pagamentos reutilizam os recursos do livro-razão Norito; os serviços de tesouraria fornecem APIs auxiliares (`/v1/finance/sns/payments`).
- Os SDKs devem envolver esses endpoints com ajudantes de tipos fortes para que as carteiras possam apresentar razões de erro claras (`ERR_SNS_RESERVED`, etc.).

## 9. Étapes de Prochaines

- Confie nos manipuladores Torii no contrato de registro da bobina nos leilões SN-3 disponíveis.
- Publicar guias específicos do SDK (Rust/JS/Swift) referentes a esta API.
- Etendre [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) com des liens croises vers les champs de preuve des hooks de gouvernance.