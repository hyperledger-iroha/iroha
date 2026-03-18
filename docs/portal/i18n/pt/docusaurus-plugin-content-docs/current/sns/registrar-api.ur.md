---
lang: pt
direction: ltr
source: docs/portal/docs/sns/registrar-api.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sns/registrar_api.md` کی عکاسی کرتا ہے اور اب پورٹل
کی کینونیکل کاپی ہے۔ سورس فائل ترجمہ PRs کے لئے برقرار رہتی ہے۔
:::

# API do registrador SNS e ganchos (SN-2b)

**حالت:** 2026-03-24 مسودہ - Nexus Core ریویو کے تحت  
**روڈمیپ لنک:** SN-2b "API do registrador e ganchos de governança"  
**پیشگی شرائط:** اسکیمہ تعریفیں [`registry-schema.md`](./registry-schema.md) میں۔

Não há endpoints Torii, serviços gRPC, DTOs de solicitação/resposta e governança
artefatos کی وضاحت کرتا ہے جو registrador Sora Name Service (SNS) چلانے کے لئے
درکار ہیں۔ یہ SDKs, carteiras e automação کے لئے مستند معاہدہ ہے جو SNS نام
رجسٹر، renovar یا gerenciar کرنا چاہتے ہیں۔

## 1. ٹرانسپورٹ اور توثیق

| شرط | تفصیل |
|-----|-------|
| پروٹوکولز | REST `/v1/sns/*` کے تحت اور serviço gRPC `sns.v1.Registrar`۔ O Norito-JSON (`application/json`) e o Norito-RPC são de alta qualidade (`application/x-norito`) |
| Autenticação | Tokens `Authorization: Bearer` یا certificados mTLS ہر suffix steward کی طرف سے جاری۔ endpoints sensíveis à governança (congelar/descongelar, atribuições reservadas) کے لئے `scope=sns.admin` لازم ہے۔ |
| ریٹ حدود | Registradores `torii.preauth_scheme_limits` agrupam chamadores JSON کے ساتھ شیئر کرتے ہیں اور ہر sufixo کے لئے burst caps: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`۔ |
| ٹیلیمیٹری | Manipuladores de registrador Torii کے لئے `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` ظاہر کرتا ہے (`scheme="norito_rpc"` filtro de filtro) ؛ API بھی `sns_registrar_status_total{result, suffix_id}` بڑھاتی ہے۔ |

## 2. DTO خلاصہ

فیلڈز [`registry-schema.md`](./registry-schema.md) میں متعین estruturas canônicas کو consulte کرتی ہیں۔ Cargas úteis `NameSelectorV1` + `SuffixId` são usadas para roteamento de dados

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

| Ponto final | طریقہ | Carga útil | تفصیل |
|----------|-------|---------|-------|
| `/v1/sns/registrations` | POSTAR | `RegisterNameRequestV1` | Não há nenhum problema com você nível de preços حل کرتا ہے، provas de pagamento/governança کی توثیق کرتا ہے، eventos de registro emitem کرتا ہے۔ |
| `/v1/sns/registrations/{selector}/renew` | POSTAR | `RenewNameRequestV1` | مدت بڑھاتا ہے۔ پالیسی سے janelas de graça/redenção نافذ کرتا ہے۔ |
| `/v1/sns/registrations/{selector}/transfer` | POSTAR | `TransferNameRequestV1` | حکمرانی aprovações لگنے کے بعد propriedade منتقل کرتا ہے۔ |
| `/v1/sns/registrations/{selector}/controllers` | COLOCAR | `UpdateControllersRequestV1` | controladores endereços de contas assinadas کی توثیق کرتا ہے۔ |
| `/v1/sns/registrations/{selector}/freeze` | POSTAR | `FreezeNameRequestV1` | guardião/conselho congelar۔ bilhete de guardião ou súmula de governança کا حوالہ درکار۔ |
| `/v1/sns/registrations/{selector}/freeze` | EXCLUIR | `GovernanceHookV1` | remediação کے بعد descongelar؛ substituição do conselho ریکارڈ ہونے کو یقینی بناتا ہے۔ |
| `/v1/sns/reserved/{selector}` | POSTAR | `ReservedAssignmentRequestV1` | nomes reservados کی administrador/conselho کی طرف سے atribuição۔ |
| `/v1/sns/policies/{suffix_id}` | OBTER | -- | `SuffixPolicyV1` موجودہ حاصل کرتا ہے (armazenável em cache)۔ |
| `/v1/sns/registrations/{selector}` | OBTER | -- | موجودہ `NameRecordV1` + موثر حالت (Active, Grace وغیرہ) واپس کرتا ہے۔ |

**Codificação do seletor:** `{selector}` segmento de caminho I105, compactado (`sora`) یا hexadecimal canônico ADDR-5 کے مطابق قبول کرتا ہے؛ Torii `NameSelectorV1` سے normalizar کرتا ہے۔**Modelo de erro:** Defina os endpoints Norito JSON `code`, `message`, `details` واپس کرتے ہیں۔ Códigos میں `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing` شامل ہیں۔

### 3.1 Auxiliares CLI (N0 دستی registrador ضرورت)

Administradores beta fechados اب CLI کے ذریعے registrador استعمال کر سکتے ہیں بغیر ہاتھ سے JSON بنانے کے:

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

- `--owner` بطور ڈیفالٹ conta de configuração CLI; As contas do controlador são `--controller` e دہرائيں (padrão `[owner]`).
- Sinalizadores de pagamento em linha براہ راست `PaymentProofV1` سے mapa ہوتے ہیں؛ جب recibo estruturado ہو تو `--payment-json PATH` دیں۔ Metadados (`--metadata-json`) e ganchos de governança (`--governance-json`) بھی اسی انداز میں ہیں۔

Ensaios de ajudantes somente leitura کو مکمل کرتے ہیں:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Implementação کے لئے `crates/iroha_cli/src/commands/sns.rs` دیکھیں؛ comandos اس دستاویز میں بیان کردہ Norito DTOs دوبارہ استعمال کرتے ہیں تاکہ CLI saída Torii respostas O que significa byte por byte

Renovações, transferências de ajudantes adicionais e ações do guardião کو کور کرتے ہیں:

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

`--governance-json` میں درست `GovernanceHookV1` ریکارڈ ہونا چاہیے (id da proposta, hashes de voto, assinaturas do administrador/guardião)۔ ہر کمانڈ متعلقہ `/v1/sns/registrations/{selector}/...` endpoint کی عکاسی کرتی ہے تاکہ beta operadores بالکل وہی Torii superfícies ensaiam کر O que há de melhor em SDKs

## 4. serviço gRPC

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

Formato de conexão: hash de esquema Norito em tempo de compilação
`fixtures/norito_rpc/schema_hashes.json` میں درج ہے (linhas `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, وغیرہ).

## 5. Ganchos de governança e evidências

ہر chamada mutante کو repetição کے لئے موزوں evidência منسلک کرنا ہوتا ہے:

| Ação | Dados de governação necessários |
|--------|--------------------------|
| Registo/renovação padrão | Instruções de liquidação کو consulte کرنے والی comprovante de pagamento؛ votação do conselho کی ضرورت نہیں جب تک nível کو aprovação do administrador نہ چاہئے۔ |
| Registro de nível premium/atribuição reservada | `GovernanceHookV1` ou ID da proposta + confirmação do administrador, consulte کرتا ہے۔ |
| Transferência | Hash de votação do conselho + hash de sinal DAO؛ autorização do guardião جب resolução de disputas de transferência سے gatilho ہو۔ |
| Congelar/descongelar | Assinatura do tíquete do guardião کے ساتھ substituição do conselho (descongelar)۔ |

Provas Torii کو جانچتے ہوئے چیک کرتا ہے:

1. livro razão de governança de identificação de proposta (`/v1/governance/proposals/{id}`) موجود ہے اور status `Approved` ہے۔
2. hashes ریکارڈ شدہ artefatos de votação سے correspondência کرتے ہیں۔
3. assinaturas do administrador/guardião `SuffixPolicyV1` سے متوقع chaves públicas کو consulte کرتے ہیں۔

Verificações falhadas `sns_err_governance_missing` واپس کرتے ہیں۔

## 6. Exemplos de fluxo de trabalho

### 6.1 Registro Padrão1. Cliente `/v1/sns/policies/{suffix_id}` کو consulta کرتا ہے تاکہ preços, graça e دستیاب níveis حاصل کرے۔
2. Cliente `RegisterNameRequestV1` بناتا ہے:
   - `selector` ترجیحی I105 یا segundo melhor rótulo compactado (`sora`) سے derivado ہے۔
   - `term_years` پالیسی حدود میں۔
   - `payment` transferência de divisão de tesouraria/administrador کو consulte کرتا ہے۔
3. Torii validar o valor:
   - Normalização de rótulos + lista reservada۔
   - Prazo/preço bruto vs `PriceTierV1`.
   - Valor do comprovante de pagamento >= preço calculado + taxas۔
4. کامیابی پر Torii:
   - `NameRecordV1` محفوظ کرتا ہے۔
   - `RegistryEventV1::NameRegistered` emite کرتا ہے۔
   - `RevenueAccrualEventV1` emite کرتا ہے۔
   - Nenhum registro + eventos واپس کرتا ہے۔

### 6.2 Renovação durante a graça

Renovações de carência میں solicitação padrão کے ساتھ detecção de penalidade شامل ہے:

- Torii `now` vs `grace_expires_at` چیک کرتا ہے اور `SuffixPolicyV1` سے tabelas de sobretaxa شامل کرتا ہے۔
- Comprovante de pagamento کو cobertura de sobretaxa کرنا ہوگا۔ Falha => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` `expires_at` ریکارڈ کرتا ہے۔

### 6.3 Congelamento do Guardião e Substituição do Conselho

1. Guardian `FreezeNameRequestV1` enviar کرتا ہے جس میں ID do incidente کا حوالہ دینے والا ticket ہوتا ہے۔
2. Registro Torii کو `NameStatus::Frozen` میں منتقل کرتا ہے, `NameFrozen` emite کرتا ہے۔
3. Remediação کے بعد substituição do conselho جاری کرتا ہے؛ operador DELETE `/v1/sns/registrations/{selector}/freeze` کو `GovernanceHookV1` کے ساتھ بھیجتا ہے۔
4. Torii substituir validar کرتا ہے, `NameUnfrozen` emitir کرتا ہے۔

## 7. Validação e códigos de erro

| Código | Descrição | http |
|------|-------------|------|
| `sns_err_reserved` | Rótulo reservado یا bloqueado ہے۔ | 409 |
| `sns_err_policy_violation` | Prazo, controladores de nível یا کا سیٹ پالیسی کی خلاف ورزی کرتا ہے۔ | 422 |
| `sns_err_payment_mismatch` | Comprovante de pagamento میں valor یا incompatibilidade de ativos۔ | 402 |
| `sns_err_governance_missing` | Artefatos de governança necessários غائب/ہیں۔ inválidos | 403 |
| `sns_err_state_conflict` | موجودہ estado do ciclo de vida میں operação permitida نہیں۔ | 409 |

Códigos `X-Iroha-Error-Code` e envelopes Norito JSON/NRPC estruturados کے ذریعے superfície ہوتے ہیں۔

## 8. Notas de implementação

- Torii leilões pendentes کو `NameRecordV1.auction` میں رکھتا ہے اور `PendingAuction` کے دوران tentativas diretas de registro کو rejeitar کرتا ہے۔
- Comprovantes de pagamento Norito recibos contábeis دوبارہ استعمال کرتے ہیں؛ APIs auxiliares de serviços de tesouraria (`/v1/finance/sns/payments`) فراہم کرتی ہیں۔
- SDKs کو ان endpoints کو auxiliares fortemente digitados سے wrap کرنا چاہئے تاکہ carteiras واضح motivos de erro (`ERR_SNS_RESERVED`, وغیرہ) دکھا سکیں۔

## 9. Próximas etapas

- Leilões SN-3 کے بعد manipuladores Torii کو اصل contrato de registro سے wire کریں۔
- Guias específicos do SDK (Rust/JS/Swift) شائع کریں جو اس API کو consulte کریں۔
- [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) کو campos de evidência de gancho de governança کے links cruzados کے ساتھ estender کریں۔