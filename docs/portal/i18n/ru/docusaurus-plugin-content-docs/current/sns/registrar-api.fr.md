---
lang: ru
direction: ltr
source: docs/portal/docs/sns/registrar-api.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Источник канонический
Эта страница отражает `docs/source/sns/registrar_api.md` и выглядит ужасно
копия канонического порта. Le fichier source est conserve pour les flux de
перевод.
:::

# API регистратора SNS и механизмов управления (SN-2b)

**Статус:** Redige от 24 марта 2026 г. – обзор для Nexus Core  
**Дорожная карта залога:** SN-2b «API регистратора и механизмы управления»  
**Предварительные требования:** Определения схемы в [`registry-schema.md`](./registry-schema.md)

В этом примечании указаны конечные точки Torii, службы gRPC, DTO запроса/ответа.
и артефакты управления, необходимые для работы регистратора Соры Имя
Сервис (СНС). Это справочный контракт для SDK, кошельков и т. д.
автоматизация, позволяющая регистрировать, обновлять или добавлять имена SNS.

## 1. Транспорт и аутентификация

| Необходимость | Деталь |
|----------|--------|
| Протоколы | REST sous `/v1/sns/*` и сервис gRPC `sns.v1.Registrar`. Два принимающих Norito-JSON (`application/json`) и бинарный Norito-RPC (`application/x-norito`). |
| Авторизация | Jetons `Authorization: Bearer` или сертификаты mTLS с суффиксом стюарда. Конечные точки, чувствительные к управлению (замораживание/размораживание, резервирование аффектов), требуют `scope=sns.admin`. |
| Дебетовые лимиты | Регистраторы разделяют сегменты `torii.preauth_scheme_limits` с заявителями JSON плюс ограничения по rafale по суффиксу: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Телеметрия | Torii предоставляет `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` для обработчиков регистратора (фильтр `scheme="norito_rpc"`); Увеличение API aussi `sns_registrar_status_total{result, suffix_id}`. |

## 2. Знакомство с DTO

Ссылки на канонические структуры, определенные в [`registry-schema.md`](./registry-schema.md). Используйте встроенные утилиты `NameSelectorV1` + `SuffixId` для устранения неоднозначной маршрутизации.

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
|----------|---------|---------|-------------|
| `/v1/sns/names` | ПОСТ | `RegisterNameRequestV1` | Зарегистрируйтесь или укажите номер. Resout le tier de prix, valide les preuves de payement/goovernance, emet des Evenements de Registration. |
| `/v1/sns/names/{namespace}/{literal}/renew` | ПОСТ | `RenewNameRequestV1` | Продлите срок. Аппликация «Les Fenetres de Grace/Redemption depuis la Politique». |
| `/v1/sns/names/{namespace}/{literal}/transfer` | ПОСТ | `TransferNameRequestV1` | Передайте право собственности на одобрения совместного управления. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | ПУТЬ | `UpdateControllersRequestV1` | Заменить ансамбль контроллеров; действительны адреса счетов подписантов. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | ПОСТ | `FreezeNameRequestV1` | Заморозить опекуна/совета. Найдите хранителя билетов и справку по государственному досье. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | УДАЛИТЬ | `GovernanceHookV1` | Разморозить после восстановления; Убедитесь, что отметка совета зарегистрирована. |
| `/v1/sns/reserved/{selector}` | ПОСТ | `ReservedAssignmentRequestV1` | Влияние на имя оставляет за собой номинал управляющего/совета. |
| `/v1/sns/policies/{suffix_id}` | ПОЛУЧИТЬ | -- | Восстановите текущий файл `SuffixPolicyV1` (кэшируемый). |
| `/v1/sns/names/{namespace}/{literal}` | ПОЛУЧИТЬ | -- | Возврат le `NameRecordV1` courant + etat effectif (Active, Grace и т. д.). |**Кодировка селектора:** сегмент `{selector}` принимает i105, сжимает или шестнадцатеричный канонический набор ADDR-5; Torii нормализуется через `NameSelectorV1`.

**Модель ошибки:** все конечные точки возвращаются Norito JSON с `code`, `message`, `details`. Коды включают `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Помощник CLI (требование к регистратору вручную N0)

Фермеры бета-тестирования могут использовать регистратор через CLI без фабрики JSON a la main:

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

- `--owner` prend по умолчанию для расчета конфигурации CLI; Повторите `--controller` для настройки дополнительных счетчиков контроллера (по умолчанию `[owner]`).
- Встроенные флаги оплаты отображаются в направлении версии `PaymentProofV1`; passez `--payment-json PATH`, когда вы найдете восстановленную структуру. Метадонны (`--metadata-json`) и рычаги управления (`--governance-json`) соответствуют схеме мема.

Помощники на лекциях завершают повторения:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Voir `crates/iroha_cli/src/commands/sns.rs` для реализации; Команды повторно используют DTO Norito в этом документе, который соответствует байту CLI для байтовых дополнительных ответов Torii.

Дополнительные помощники в отношении обновлений, переводов и действий по защите:

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

`--governance-json` содержит действительную регистрацию `GovernanceHookV1` (идентификатор предложения, хэши голосов, подписи распорядителя/опекуна). Команда Reflete Simple l'endpoint `/v1/sns/names/{namespace}/{literal}/...` соответствует тем, которые операторы бета-версии могут повторять требования к поверхностям Torii, которые называются SDK.

## 4. Служба gRPC

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

Формат провода: хэш схемы Norito во время компиляции и регистрации в нем.
`fixtures/norito_rpc/schema_hashes.json` (линии `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1` и т. д.).

## 5. Приемы управления и ограничения

Выберите, что нужно изменить, чтобы объединить повторно используемые предметы для отражения:

| Действие | Реквизиты для управления |
|--------|-------------------------------|
| Стандарт регистрации/обновления | Preuve de payement Referencant ине инструкция по расчету; aucun голосование в совете необходимо для того, чтобы получить высокий уровень одобрения стюарда. |
| Регистрация премий/резервных премий | `GovernanceHookV1` Идентификатор предложения референта + подтверждение управляющего. |
| Трансферт | Хэш голосования совета + хэш сигнала DAO; разрешение опекуна, когда перевод является отклонением по разрешению дела. |
| Заморозить/Разморозить | Подпись хранителя билета плюс отмена совета (разморозка). |

Torii проверьте предварительные условия проверки:

1. Идентификатор предложения существует в бухгалтерской книге управления (`/v1/governance/proposals/{id}`) и в законе `Approved`.
2. Регистрируются хэши, соответствующие артефактам голосования.
3. Подписи стюарда/опекуна, ссылающиеся на общественных посетителей `SuffixPolicyV1`.

Элементы управления отданы `sns_err_governance_missing`.

## 6. Примеры рабочего процесса### 6.1 Стандарт регистрации

1. Опрос клиента `/v1/sns/policies/{suffix_id}` для восстановления призов, благодати и других доступных уровней.
2. Конструируем клиент `RegisterNameRequestV1`:
   - `selector` получить метку i105 (предпочтительно) или сжать (второй выбор).
   - `term_years` в пределах политики.
   - `payment` референтант le Transfert du Splitter tresorerie/steward.
3. Torii действительно:
   - Нормализация метки + резервный список.
   - Срок/цена брутто по сравнению с `PriceTierV1`.
   - Montant de preuve de paiement >= расчетная цена + фрайз.
4. Успешно Torii:
   - Сохраняюсь `NameRecordV1`.
   - Эмет `RegistryEventV1::NameRegistered`.
   - Эмет `RevenueAccrualEventV1`.
   - Вернуть новую запись + события.

### 6.2 Подвеска La Grace Renouvellement

Обновления подвески la Grace, включая стандартный запрос плюс обнаружение наказания:

- Torii сравните `now` с `grace_expires_at` и добавьте таблицы надбавок к `SuffixPolicyV1`.
- Предоплата должна быть произведена за дополнительную плату. Эчек => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` зарегистрируйте новый `expires_at`.

### 6.3 Заморозить стража и отменить совет

1. Guardian имеет номер `FreezeNameRequestV1` с указанием билета, указывающего на инцидент.
2. Torii замените запись в `NameStatus::Frozen`, добавьте `NameFrozen`.
3. После исправления совет должен отменить решение; l'operateur envoie DELETE `/v1/sns/names/{namespace}/{literal}/freeze` с `GovernanceHookV1`.
4. Torii действителен для переопределения, используйте `NameUnfrozen`.

## 7. Проверка и коды ошибок

| Код | Описание | HTTP |
|------|-------------|------|
| `sns_err_reserved` | Этикетка резервная или блокированная. | 409 |
| `sns_err_policy_violation` | Срок, уровень или ансамбль контролеров политики. | 422 |
| `sns_err_payment_mismatch` | Несоответствие стоимости или актива в преуве де платеже. | 402 |
| `sns_err_governance_missing` | Отсутствующие/недействительные артефакты управления. | 403 |
| `sns_err_state_conflict` | Операция не разрешена в текущем цикле жизни. | 409 |

Все коды передаются через структуры `X-Iroha-Error-Code` и конверты Norito JSON/NRPC.

## 8. Замечания по реализации

- Torii проводит аукционы и следит за `NameRecordV1.auction` и отклоняет предварительные заявки на регистрацию напрямую, как `PendingAuction`.
- Les preuves de paiement reutilisent les recus du Ledger Norito; вспомогательные службы API (`/v1/finance/sns/payments`).
- Пакеты SDK разрабатывают конечные точки с типами помощников для того, чтобы кошельки были мощными презентаторами причин ошибок (`ERR_SNS_RESERVED` и т. д.).

##9. Прохейновые этапы

- Доверьтесь обработчикам Torii при регистрации контракта на доступных аукционах SN-3.
- Издатель руководств по спецификациям SDK (Rust/JS/Swift), ссылающийся на этот API.
- Etendre [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) с залогами круазов и превентивными крючками управления.