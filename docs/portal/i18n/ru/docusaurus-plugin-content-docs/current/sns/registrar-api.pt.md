---
lang: ru
direction: ltr
source: docs/portal/docs/sns/registrar-api.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание Fonte canonica
Эта страница написана `docs/source/sns/registrar_api.md` и агора служит как
Копия каноника до портала. Постоянный архив для потоков перевода.
:::

# API для регистратора SNS и перехватчиков управления (SN-2b)

**Статус:** Redigido от 24 марта 2026 г. -- внесена ревизия для Nexus Core  
**Ссылка на дорожную карту:** SN-2b «API регистратора и механизмы управления».  
**Предварительные требования:** Определения проблемы в [`registry-schema.md`](./registry-schema.md)

Это примечание о конкретных конечных точках Torii, службах gRPC, DTO запроса/ответа и
артефакты управления, необходимые для работы регистратора службы имен Sora (SNS).
Это авторизованный договор для SDK, кошельков и автоматического режима точного регистратора,
обновить или изменить имена SNS.

## 1. Транспортировка и аутентификация

| Реквизито | Подробности |
|-----------|---------|
| Протоколы | REST соб `/v1/sns/*` и сервис gRPC `sns.v1.Registrar`. Содержит Norito-JSON (`application/json`) и бинарный файл Norito-RPC (`application/x-norito`). |
| Авторизация | Токены `Authorization: Bearer` или сертификаты mTLS, выданные для суффикса-стюарда. Конечные точки чувствительны к управлению (замораживание/размораживание, резервирование атрибутов) exigem `scope=sns.admin`. |
| Ограничения таксонов | Регистраторы компилируют сегменты `torii.preauth_scheme_limits` с файлами JSON с наибольшими ограничениями по суффиксу: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Телеметрия | Torii expoe `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` для обработчиков регистратора (фильтр `scheme="norito_rpc"`); API также увеличивается `sns_registrar_status_total{result, suffix_id}`. |

## 2. Генеральная виза DTO

Мы ссылаемся на канонические структуры, определенные в [`registry-schema.md`](./registry-schema.md). В качестве груза в комплект поставки входят `NameSelectorV1` + `SuffixId`, чтобы избежать неоднозначной ситуации.

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

## 3. Конечные точки REST

| Конечная точка | Метод | Полезная нагрузка | Описание |
|----------|--------|---------|-----------|
| `/v1/sns/names` | ПОСТ | `RegisterNameRequestV1` | Регистратор или восстановите имя. Примите решение о предварительном уровне, проверите платежные документы/управление, опубликуйте события регистрации. |
| `/v1/sns/names/{namespace}/{literal}/renew` | ПОСТ | `RenewNameRequestV1` | Estende o termo. Aplica janelas де благодати/искупления да политика. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | ПОСТ | `TransferNameRequestV1` | Передача собственности после одобрения управления в форме анексадас. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | ПУТЬ | `UpdateControllersRequestV1` | Замена или соединение контроллеров; действительный результат убийства. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | ПОСТ | `FreezeNameRequestV1` | Заморозка опекуна/совета. Запросите опекуна по билетам и справочную информацию о досье правительства. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | УДАЛИТЬ | `GovernanceHookV1` | Разморозить после исправления; гарантия отмены регистрации в совете. |
| `/v1/sns/reserved/{selector}` | ПОСТ | `ReservedAssignmentRequestV1` | Зарезервированные имена для стюарда/совета. |
| `/v1/sns/policies/{suffix_id}` | ПОЛУЧИТЬ | -- | Busca `SuffixPolicyV1` настоящий (cacheavel). |
| `/v1/sns/names/{namespace}/{literal}` | ПОЛУЧИТЬ | -- | Retorna `NameRecordV1` atual + estado efetivo (Актив, Грейс и т. д.). |**Кодификация селектора:** или сегмент `{selector}` с кодом i105, включающим или шестнадцатеричное каноническое соответствие ADDR-5; Torii нормализуется через `NameSelectorV1`.

**Модель ошибок:** все конечные точки ОС повторно называются Norito JSON com `code`, `message`, `details`. Коды ОС включают `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Helpers CLI (требования к руководству регистратора N0)

Руководители бета-тестирования перед началом работы с регистратором через CLI с помощью JSON вручную:

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

- `--owner` открывается и содержит информацию о настройке CLI; повтор `--controller` для подключения дополнительного контроллера (padrao `[owner]`).
- Флаги встроенной страницы, отображаемые непосредственно для `PaymentProofV1`; Passe `--payment-json PATH`, когда я говорил и получил ответ. Метададо (`--metadata-json`) и рычаги управления (`--governance-json`) следуют дальше или дальше.

Помощники в отдыхе в некоторых случаях:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Veja `crates/iroha_cli/src/commands/sns.rs` для реализации; Команды повторно используют DTO Norito, описывая следующий документ, который говорит, что CLI совпадает по байту или байту с ответом на Torii.

Дополнительные помощники по ремонту, переносу и опекуну:

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

`--governance-json` deve conter um registro `GovernanceHookV1` valido (идентификатор предложения, хэши голосов, управляющий/опекун assinaturas). Простой командой выберите конечную точку `/v1/sns/names/{namespace}/{literal}/...`, которая соответствует бета-версии, а также поверхностному интерфейсу Torii, который работает с SDK.

## 4. Сервис gRPC

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

Формат проводной связи: хеш-код Norito во время регистрации при компиляции.
`fixtures/norito_rpc/schema_hashes.json` (linhas `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1` и т. д.).

## 5. Крючки управления и доказательства

Все, что нужно сделать, чтобы изменить состояние, предоставили достаточные доказательства для повтора:

| Акао | Требования к управлению |
|------|-------------------------------|
| Регистрация/обновление здания | Прова де референсионо референсиандо с инструкцией по урегулированию; нам не нужно голосовать за то, чтобы совет был одобрен стюардом на уровне exija. |
| Регистр премиум-класса/зарезервированный доход | `GovernanceHookV1` Идентификатор референсиандо предложения + подтверждение управляющего. |
| Трансференсия | Хэш голосования по совету + хэш сигнала DAO; Разрешение опекуна при передаче и разрешении спора. |
| Заморозить/Разморозить | Assinatura может сделать хранителя билетов более приоритетным, чем совет (разморозить). |

Torii проверено как подтверждение конференции:

1. Идентификатор предложения не существует в реестре управления (`/v1/governance/proposals/{id}`) и статусе `Approved`.
2. Хэши соответствуют зарегистрированным артефактам.
3. Управляющий/опекун Assinaturas ссылается на публичные надежды на `SuffixPolicyV1`.

Фалхас вернул имя `sns_err_governance_missing`.

## 6. Примеры потока труда

### 6.1 Регистрационный номер1. Проконсультируйтесь с клиентом `/v1/sns/policies/{suffix_id}`, чтобы получить предварительные условия, льготы и доступные уровни.
2. Клиент Монта `RegisterNameRequestV1`:
   - `selector` является производным от метки i105 (предпочтительно) или стандартно (второй вариант).
   - `term_years` в пределах границ политики.
   - `payment` ссылается на передачу разделителя tesouraria/steward.
3. Проверка Torii:
   - Нормализация метки + резервный список.
   - Срок/цена брутто по сравнению с `PriceTierV1`.
   - Сумма платежа >= предварительный расчет + сборы.
4. После успешного Torii:
   - Сохраняюсь `NameRecordV1`.
   - Эмите `RegistryEventV1::NameRegistered`.
   - Эмите `RevenueAccrualEventV1`.
   - Возврат к новой регистрации + события.

### 6.2 Ремонт на длительный срок

Ремонтные работы на период отсрочки включают в себя реквизицию, которая может быть обнаружена при обнаружении наказания:

- Torii для сравнения `now` с `grace_expires_at` и дополнительные таблицы по сбору `SuffixPolicyV1`.
- A prova de pagamento deve cobrir a sobretaxa. Фальха => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` регистрация или новая `expires_at`.

### 6.3 Заморозка стража и отмена совета

1. Guardian отправляет `FreezeNameRequestV1` ссылку на билет с идентификатором инцидента.
2. Torii переместите реестр для пункта `NameStatus::Frozen`, создайте `NameFrozen`.
3. В случае исправления ситуации совет может отменить решение; o оператор envia DELETE `/v1/sns/names/{namespace}/{literal}/freeze` com `GovernanceHookV1`.
4. Torii действителен или переопределен, выдайте `NameUnfrozen`.

## 7. Проверка и коды ошибок

| Кодиго | Описание | HTTP |
|--------|-----------|------|
| `sns_err_reserved` | Этикетка зарезервирована или заблокирована. | 409 |
| `sns_err_policy_violation` | Термин, уровень или сочетание контролеров в политике. | 422 |
| `sns_err_payment_mismatch` | Несоответствие доблести или актива в доказательстве оплаты. | 402 |
| `sns_err_governance_missing` | Artefatos degovanca requeridos ausentes/invalidos. | 403 |
| `sns_err_state_conflict` | Операция не разрешена в обычном режиме для циклической жизни. | 409 |

Все коды доступны через `X-Iroha-Error-Code` и конверты Norito JSON/NRPC.

## 8. Замечания о реализации

- Torii закреплены на крючках `NameRecordV1.auction` и подтверждены предварительные сведения о регистрации непосредственно в `PendingAuction`.
- Условия повторного использования рецептов бухгалтерской книги Norito; Помощник API-интерфейсов tesouraria fornecem (`/v1/finance/sns/payments`).
- SDK разрабатываются с использованием конечных точек и помощников для того, чтобы кошельки представляли причины ошибок (`ERR_SNS_RESERVED` и т. д.).

## 9. Проксимос пассос

- Подключите обработчики к Torii к реальному договору регистрации, когда используются символы SN-3.
- Публикуются конкретные описания SDK (Rust/JS/Swift), ссылающиеся на этот API.
- Estender [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) com ссылается на крузадос для сбора доказательств управления.