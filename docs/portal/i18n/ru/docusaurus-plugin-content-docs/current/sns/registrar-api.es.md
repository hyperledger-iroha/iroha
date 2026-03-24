---
lang: ru
direction: ltr
source: docs/portal/docs/sns/registrar-api.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::обратите внимание на Фуэнте каноника
Эта страница отражается `docs/source/sns/registrar_api.md` и сейчас сирве как ля
копия каноника дель портала. Архив, который можно использовать для плавания
перевод.
:::

# API регистратора SNS и хуков управления (SN-2b)

**Эстадо:** Borrador 24 марта 2026 г. – последняя версия Nexus Core.  
**Дополнительная информация:** SN-2b «API регистратора и механизмы управления».  
**Предварительные требования:** Определения языка в [`registry-schema.md`](./registry-schema.md)

Это особое примечание по конечным точкам Torii, службам gRPC, запросам DTO.
ответы и артефакты правительства, необходимые для работы регистратора
Служба имен Сора (SNS). Это авторитарный договор для SDK, кошельков и
автоматизация, необходимая для регистрации, обновления или регистрации номеров SNS.

## 1. Транспортировка и аутентификация

| Реквизито | Подробности |
|-----------|---------|
| Протоколы | REST-баджо `/v1/sns/*` и сервис gRPC `sns.v1.Registrar`. Принимает Norito-JSON (`application/json`) и бинарный файл Norito-RPC (`application/x-norito`). |
| Авторизация | Токены `Authorization: Bearer` или сертификаты mTLS, выданные для суффикса-стюарда. Конечные точки, чувствительные к управлению (замораживание/размораживание, резервирование резервных копий), требуют `scope=sns.admin`. |
| Ограничения по работе | Регистраторы разделяют сегменты `torii.preauth_scheme_limits` с вызывающими объектами JSON с ограничениями по суффиксу: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Телеметрия | Torii экспонирует `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` для обработчиков регистратора (фильтр `scheme="norito_rpc"`); API также увеличивается `sns_registrar_status_total{result, suffix_id}`. |

## 2. Резюме DTO

Ссылка на канонические структуры, определенные в [`registry-schema.md`](./registry-schema.md). Все грузы включают `NameSelectorV1` + `SuffixId`, чтобы избежать неоднозначной ситуации.

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
|----------|--------|---------|-------------|
| `/v1/sns/names` | ПОСТ | `RegisterNameRequestV1` | Регистратор или восстановите номер. Подтвердите уровень драгоценных камней, проверите платежи по паго/гобернансе, создайте события регистрации. |
| `/v1/sns/names/{namespace}/{literal}/renew` | ПОСТ | `RenewNameRequestV1` | Расширенный терминал. Aplica ventanas de gracia/redencion segun la politica. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | ПОСТ | `TransferNameRequestV1` | Transfiere propiedad una vez adjuntas las aprobaciones de gobernanza. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | ПУТЬ | `UpdateControllersRequestV1` | Замените набор контроллеров; действительные направления движения фирм. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | ПОСТ | `FreezeNameRequestV1` | Заморозка опекуна/совета. Потребуйте билет опекуна и справку по досье губернатора. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | УДАЛИТЬ | `GovernanceHookV1` | Разморозить tras remediation; asegura отменяет регистрацию совета. |
| `/v1/sns/reserved/{selector}` | ПОСТ | `ReservedAssignmentRequestV1` | Назначение резервированных имен управляющего/совета. |
| `/v1/sns/policies/{suffix_id}` | ПОЛУЧИТЬ | -- | Получить актуальный `SuffixPolicyV1` (кэшируемый). |
| `/v1/sns/names/{namespace}/{literal}` | ПОЛУЧИТЬ | -- | Devuelve `NameRecordV1` актуально + estado efectivo (Active, Grace и т. д.). |**Кодификация селектора:** сегмент `{selector}` принимает I105, включая шестнадцатеричный канонический второй ADDR-5; Torii для нормализации через `NameSelectorV1`.

**Модель ошибок:** все конечные точки в формате Norito JSON с `code`, `message`, `details`. В число кодов входят `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Helpers CLI (требования к руководству регистратора N0)

Стюарды бета-серрады могут работать с регистратором через CLI без брони JSON вручную:

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

- `--owner` вызван дефектом в месте настройки CLI; Повторите `--controller` для дополнительного контроллера (по умолчанию `[owner]`).
- Встроенные флаги страницы, расположенные непосредственно на `PaymentProofV1`; США `--payment-json PATH`, когда вы получите estructurado. Метаданные (`--metadata-json`) и перехватчики управления (`--governance-json`) указывают на патрона.

Помощники по завершению сольных лекций:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Версия `crates/iroha_cli/src/commands/sns.rs` для реализации; Команды повторно используют DTO Norito, описанные в этом документе, чтобы данные CLI совпадали по байтам с ответами Torii.

Дополнительные помощники по ремонту, трансферу и хранению:

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

`--governance-json` должен содержать действительный регистр `GovernanceHookV1` (идентификатор предложения, хэши голосов, фирмы-распорядители/опекуны). Эта команда просто отражает конечную точку `/v1/sns/names/{namespace}/{literal}/...`, соответствующую бета-версиям операторов, и точно указывает на поверхность Torii, которую можно использовать для SDK.

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

Формат провода: хеш-код Norito во время регистрации компиляции
`fixtures/norito_rpc/schema_hashes.json` (филас `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1` и т. д.).

## 5. Крючки для государственного управления и доказывания

Если вам потребуется дополнительное доказательство для повторного воспроизведения:

| Акцион | Требуемые государственные данные |
|--------|-------------------------------|
| Регистрация/ремонт | Нажмите на ссылку, содержащую инструкцию по урегулированию; Нет необходимости голосовать за залп совета, который требует одобрения стюарда. |
| Премиальный регистр / резервирование | `GovernanceHookV1` идентификатор предложения по ссылке + подтверждение управляющего. |
| Трансфер | Хэш голосования по совету + хеш Сенала DAO; разрешение опекуна, когда передача активируется для разрешения спора. |
| Заморозить/Разморозить | Фирма-хранитель билетов может отменить совет (разморозить). |

Torii проверка подтвержденных ошибок:

1. Идентификатор предложения существует в бухгалтерской книге правительства (`/v1/governance/proposals/{id}`) и статус `Approved`.
2. Хэши совпадают с зарегистрированными артефактами голосования.
3. Фирма-распорядитель/опекун, ссылающаяся на публичные связи, надеющиеся на `SuffixPolicyV1`.

Фаллос девюэльвен `sns_err_governance_missing`.

## 6. Эджемплос де флюхо

### 6.1 Регистрация статуса1. Консультация клиента `/v1/sns/policies/{suffix_id}` для получения ценных, льготных и доступных уровней.
2. Клиентская арматура `RegisterNameRequestV1`:
   - `selector` является производным от метки I105 (предпочтительно) или компромиссным (второй лучший вариант).
   - `term_years` в пределах границ политики.
   - `payment`, которая ссылается на передачу разделителя tesoreria/steward.
3. Проверка Torii:
   - Нормализация метки + резервный список.
   - Срок/цена брутто по сравнению с `PriceTierV1`.
   - Сумма подтверждения платежа >= расчетная цена + комиссионные.
4. При выходе Torii:
   - Сохраняюсь `NameRecordV1`.
   - Эмите `RegistryEventV1::NameRegistered`.
   - Эмите `RevenueAccrualEventV1`.
   - Развернуть новую регистрацию + события.

### 6.2 Ремонт на длительный срок

Las renovaciones en gracia включает в себя запрос на выявление нарушений:

- Torii сравнивается `now` с `grace_expires_at` и совокупными таблицами перегрузки от `SuffixPolicyV1`.
- La prueba de pago debe cubrir el recargo. Фалло => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` регистрация нового `expires_at`.

### 6.3 Заключение опекуна и отмену совета

1. Guardian отправляет `FreezeNameRequestV1` с билетом, содержащим ссылку на инцидент.
2. Torii внесите в реестр `NameStatus::Frozen`, выпустите `NameFrozen`.
3. После исправления, отказ от решения совета; оператор отправляет DELETE `/v1/sns/names/{namespace}/{literal}/freeze` с `GovernanceHookV1`.
4. Torii подтвердит переопределение, выпустите `NameUnfrozen`.

## 7. Проверка и код ошибки

| Кодиго | Описание | HTTP |
|--------|-------------|------|
| `sns_err_reserved` | Этикетка зарезервирована или заблокирована. | 409 |
| `sns_err_policy_violation` | Срок, уровень набора контролеров viola la politica. | 422 |
| `sns_err_payment_mismatch` | Несоответствие доблести или актива в пруэбе-де-паго. | 402 |
| `sns_err_governance_missing` | Artefactos de gobernanza requeridos ausentes/invalidos. | 403 |
| `sns_err_state_conflict` | Операция не разрешена на фактическом состоянии цикла жизни. | 409 |

Все коды продаются через `X-Iroha-Error-Code` и конверты Norito JSON/NRPC estructurados.

## 8. Замечания о реализации

- Torii охраняется отложенными вызовами в `NameRecordV1.auction` и повторяет намерения регистрации непосредственно в этот момент в `PendingAuction`.
- Повторное использование учетных записей бухгалтерской книги Norito; проверенный помощник API servicios de tesoreria (`/v1/finance/sns/payments`).
- SDK должны включать эти конечные точки с помощниками, которые могут вызвать ошибки кошельков (`ERR_SNS_RESERVED` и т. д.).

## 9. Проксимос Пасос

- Подключите обработчики Torii к реальному договору регистрации, в который вступают субасты SN-3.
- Публикуйте конкретные описания SDK (Rust/JS/Swift), которые ссылаются на этот API.
- Удлинитель [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) с крючком для крепления крестовин к крючку для доказательств управления.