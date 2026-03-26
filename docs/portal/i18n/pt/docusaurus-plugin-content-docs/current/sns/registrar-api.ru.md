---
lang: pt
direction: ltr
source: docs/portal/docs/sns/registrar-api.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota História Canônica
Esta página funciona `docs/source/sns/registrar_api.md` e tem uma conexão rápida
канонической копией portal. Este arquivo foi criado para PR переводов.
:::

# API de registro SNS e atualização (SN-2b)

**Estado:** Atualização 2026-03-24 — na configuração Nexus Core  
**Roteiro do Ссылка:** SN-2b "API do registrador e ganchos de governança"  
**Explicações:** Configurações de configurações em [`registry-schema.md`](./registry-schema.md)

Este é o nome do ponto de venda Torii, serviços gRPC, DTO para execução/obtenção e
Artefatos atualizados, necessários para o trabalho de registro Sora Name Service
(SNS). Este é um contrato automático para SDK, software e automação, site
Não é necessário registrar-se, fornecer ou atualizar o SNS-именами.

## 1. Transporte e autenticação

| Treino | Детали |
|------------|--------|
| Protocolos | REST para `/v1/sns/*` e serviço gRPC `sns.v1.Registrar`. Eu criei Norito-JSON (`application/json`) e o novo Norito-RPC (`application/x-norito`). |
| Autenticação | `Authorization: Bearer` tokens ou certificados mTLS, usados ​​​​por sufixo steward. Чувствительные к управлению эндпоинты (congelar/descongelar, atribuições reservadas) требуют `scope=sns.admin`. |
| Organização | registrador para buckets `torii.preauth_scheme_limits` com JSON вызывающими плюс лимиты всплеска по suffix: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Telemetria | Torii publica `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` para registrador oficial (фильтр по `scheme="norito_rpc"`); API também é usada `sns_registrar_status_total{result, suffix_id}`. |

## 2. Operador DTO

Para configurar uma estrutura canônica, consulte [`registry-schema.md`](./registry-schema.md). Todas as cargas úteis são `NameSelectorV1` + `SuffixId`, portanto, é necessário realizar uma nova operação.

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

## 3. REST эндпоинты

| Endpoint | Método | Carga útil | Descrição |
|----------|-------|---------|----------|
| `/v1/sns/names` | POSTAR | `RegisterNameRequestV1` | Регистрирует или повторно открывает имя. Altere o nível de qualidade, verifique a distribuição da placa/transferência e verifique a configuração. |
| `/v1/sns/names/{namespace}/{literal}/renew` | POSTAR | `RenewNameRequestV1` | Продлевает срок. Применяет окна graça/redenção из политики. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POSTAR | `TransferNameRequestV1` | Certifique-se de que as instruções de operação estejam atualizadas. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | COLOCAR | `UpdateControllersRequestV1` | Controladores de controle; проверяет подписанные адреса аккаунтов. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POSTAR | `FreezeNameRequestV1` | Congelar guardião/conselho. Obtenha o ticket do guardião e consulte a súmula de governança. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | EXCLUIR | `GovernanceHookV1` | Descongelar após a operação; убеждается, что Council Override зафиксирован. |
| `/v1/sns/reserved/{selector}` | POSTAR | `ReservedAssignmentRequestV1` | Назначение reservado para administrador/conselho. |
| `/v1/sns/policies/{suffix_id}` | OBTER | -- | Use a tecnologia `SuffixPolicyV1` (requer). |
| `/v1/sns/names/{namespace}/{literal}` | OBTER | -- | Возвращает текущий `NameRecordV1` + эффективное состояние (Active, Grace, и т. д.). |**Seletor de conversão:** segmento `{selector}` принимает i105, compactado (`sora`) ou канонический hexadecimal em ADDR-5; Torii é normalizado pelo `NameSelectorV1`.

**Modely ошибок:** Você pode usar Norito JSON com `code`, `message`, `details`. Os códigos são `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 CLI помощники (требование ручного регистратора N0)

Steward закрытой беты теперь могут использовать registrador через CLI без ручной сборки JSON:

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

- `--owner` para usar a CLI de configuração da conta; повторяйте `--controller`, чтобы добавить дополнительные controlador аккаунты (por умолчанию `[owner]`).
- Placa de sinalização em linha instalada em `PaymentProofV1`; передайте `--payment-json PATH`, exceto se for uma cozinha estrutural. Metadados (`--metadata-json`) e ganchos de governança (`--governance-json`) são algo que está definido.

Somente leitura помощники дополняют репетиции:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Sim. `crates/iroha_cli/src/commands/sns.rs` para realização; Os comandos podem ser usados ​​Norito DTO neste documento, usando CLI para fornecer suporte ao cliente Torii bateria.

Дополнительные помощники покрывают продления, передачи и действия guardião:

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
  --new-owner <katakana-i105-account-id> \
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

`--governance-json` é um código correto para `GovernanceHookV1` (ID da proposta, hashes de voto, administrador/guardião). Каждая команда просто отражает соответствующий энддпоинт `/v1/sns/names/{namespace}/{literal}/...` чтобы операторы беты могли репетировать Se você usar Torii, ele será instalado no SDK.

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

Formato de fio: хеш схемы Norito на этапе компиляции записан в
`fixtures/norito_rpc/schema_hashes.json` (rede `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, etc. d.).

## 5. Como atualizar e doar

Каждый изменяющий вызов должен приложить доказательства, пригодные для воспроизведения:

| Destino | Atualização de dados |
|----------|----------------------------|
| Registro/produção padrão | Доказательство платежа со ссылкой на инструкцию; O conselho administrativo não é novo, mas o nível não é o administrador. |
| Registro nível premium/atribuição reservada | `GovernanceHookV1` contém o ID da proposta + reconhecimento do administrador. |
| Predação | Хеш голосования conselho + хеш сигнала DAO; autorização do guardião, когда передача инициирована разрешением спора. |
| Congelar/descongelar | Подпись bilhete do guardião mais substituição do conselho (descongelar). |

Torii fornece suporte, prova:

1. O ID da proposta é encontrado na atualização do razão (`/v1/governance/proposals/{id}`) e no status `Approved`.
2. Verifique os artefatos de arte originais.
3. Selecione o mordomo/tutor na chave pública de `SuffixPolicyV1`.

A nova versão é `sns_err_governance_missing`.

## 6. Primeiros passos do processo

### 6.1 Registro padrão1. O cliente запрашивает `/v1/sns/policies/{suffix_id}` oferece mais dinheiro, graça e níveis de entrega.
2. Estrutura do cliente `RegisterNameRequestV1`:
   - `selector` é usado para fornecer rótulo i105 ou второго по предпочтению comprimido (`sora`).
   - `term_years` na política de segurança.
   - `payment` ссылается на перевод divisor tesouraria/administrador.
3. Prova Torii:
   - Нормализацию метки + список reservado.
   - Prazo/preço bruto vs `PriceTierV1`.
   - Сумма доказательства платежа >= рассчитанной цены + комиссии.
4. Na nova versão Torii:
   -Sofre `NameRecordV1`.
   - Verifique `RegistryEventV1::NameRegistered`.
   - Escreva `RevenueAccrualEventV1`.
   - Возвращает новую запись + события.

### 6.2 Desenvolvimento do período graça

A conveniência da graça é padrão para detectar a tela:

- Torii substitui `now` por `grace_expires_at` e cobra sobretaxa de tabelas por `SuffixPolicyV1`.
- Sobretaxa de pagamento adicional. Ошибка => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` é o novo `expires_at`.

### 6.3 Congelamento do guardião e substituição do conselho

1. Guardian отправляет `FreezeNameRequestV1` с ticket, ссылающимся на id инцидента.
2. Torii é substituído por `NameStatus::Frozen`, escrito `NameFrozen`.
3. Substituição do conselho de administração; O operador executa DELETE `/v1/sns/names/{namespace}/{literal}/freeze` com `GovernanceHookV1`.
4. Torii substitui a substituição, ispuскает `NameUnfrozen`.

## 7. Validação e códigos de verificação

| Código | Descrição | http |
|-----|----------|------|
| `sns_err_reserved` | A chave de segurança está instalada ou fechada. | 409 |
| `sns_err_policy_violation` | Срок, tier ou набор controladores нарушает политику. | 422 |
| `sns_err_payment_mismatch` | Não desligue ou ative a placa de entrega. | 402 |
| `sns_err_governance_missing` | Отсутствуют/некорректны требуемые артефакты управления. | 403 |
| `sns_err_state_conflict` | A operação não é necessária na tecnologia de ciclo fechado. | 409 |

Esses códigos são projetados para `X-Iroha-Error-Code` e estrutura Norito JSON/NRPC conversores.

## 8. Instruções para realização

- Torii хранит аукционы pendente em `NameRecordV1.auction` e отклоняет прямые попытки регистрации, пока `PendingAuction`.
- Доказательства платежа переиспользуют Norito recibos contábeis; API auxiliar de serviços de tesouraria (`/v1/finance/sns/payments`).
- O SDK permite que você use este tipo de configuração, que pode ser usado понятные причины ошибок (`ERR_SNS_RESERVED`, e т. д.).

## 9. Следующие шаги

- Use o Torii para restaurar o contrato real após o uso do SN-3.
- Abra o SDK-руководства (Rust/JS/Swift), baseado nesta API.
- Расширить [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) перекрестными ссылками на поля доказательств gancho de governança.