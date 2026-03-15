---
lang: ru
direction: ltr
source: docs/portal/docs/sns/registrar-api.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Эта страница отражает `docs/source/sns/registrar_api.md` и теперь служит
стандартной копии портала. Исходный файл остается для PR-переводов.
:::

# API регистратора SNS и хуки управления (SN-2b)

**Статус:** Подготовлен 24.03.2026 — на рассмотрении Nexus Core  
**Ссылка:** SN-2b «API регистратора и механизмы управления»  
**Предпосылки:** Схема определения в [`registry-schema.md`](./registry-schema.md)

Эта заметка описывает эндпоинты Torii, сервисы gRPC, запросы/ответы DTO и
Инструменты управления, необходимые для работы регистратора Sora Name Service
(СНС). Это важный контракт для SDK, кошельков и автоматизации, которым
нужно регистрировать, продлевать или управлять SNS-именами.

## 1. Транспортировка и аутентификация

| Требование | Детали |
|------------|--------|
| Протоколы | REST под `/v1/sns/*` и gRPC сервис `sns.v1.Registrar`. Оба принимают Norito-JSON (`application/json`) и двойной Norito-RPC (`application/x-norito`). |
| Авторизация | `Authorization: Bearer` токены или сертификаты mTLS, выданные по суффиксу steward. Чувствительные к управлению эндпоинты (замораживание/разблокирование, зарезервированные назначения) требуют `scope=sns.admin`. |
| Ограничения | регистратор делит сегменты `torii.preauth_scheme_limits` с JSON, вызывающими плюс лимиты всплеска по суффиксу: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Телеметрия | Torii публикует `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` для обработчиков регистратора (фильтр по `scheme="norito_rpc"`); API также увеличивает `sns_registrar_status_total{result, suffix_id}`. |

## 2. Обзор DTO

Поля ссылаются на стандартные структуры, установленные в [`registry-schema.md`](./registry-schema.md). Все полезные нагрузки встраивают `NameSelectorV1` + `SuffixId`, чтобы избежать неоднозначной маршрутизации.

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

## 3. REST-эндпоинты

| Конечная точка | Метод | Полезная нагрузка | Описание |
|----------|-------|---------|----------|
| `/v1/sns/registrations` | ПОСТ | `RegisterNameRequestV1` | Регистрирует или повторно открывает имя. Разрешает ценовой уровень, впоследствии доказательства платежа/управления, испускает события реестра. |
| `/v1/sns/registrations/{selector}/renew` | ПОСТ | `RenewNameRequestV1` | Продлевает срок. Применяет окно благодати/искупления из политики. |
| `/v1/sns/registrations/{selector}/transfer` | ПОСТ | `TransferNameRequestV1` | Передает воздействие после прикрепления согласования управления. |
| `/v1/sns/registrations/{selector}/controllers` | ПУТЬ | `UpdateControllersRequestV1` | Заменяет набор контроллеров; впоследствии подписанные адреса аккаунтов. |
| `/v1/sns/registrations/{selector}/freeze` | ПОСТ | `FreezeNameRequestV1` | Заморозить опекуна/совета. Требует билет опекуна и ссылку на список управления. |
| `/v1/sns/registrations/{selector}/freeze` | УДАЛИТЬ | `GovernanceHookV1` | Разморозить после ограничения; Убежден, что отмена совета зафиксирована. |
| `/v1/sns/reserved/{selector}` | ПОСТ | `ReservedAssignmentRequestV1` | Назначение зарезервированных имен стюарда/совета. |
| `/v1/sns/policies/{suffix_id}` | ПОЛУЧИТЬ | -- | Получает текущую `SuffixPolicyV1` (кэшируемо). |
| `/v1/sns/registrations/{selector}` | ПОЛУЧИТЬ | -- | Возвращает текущий `NameRecordV1` + эффективное состояние (Active, Grace и т. д.). |**Селектор кодирования:** сегмент `{selector}` принимает I105, сжатый (`sora`) или канонический шестнадцатеричный формат по ADDR-5; Torii нормализуется через `NameSelectorV1`.

**Модель ошибок:** все эндпоинты возвращают Norito JSON с `code`, `message`, `details`. Коды включают `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 CLI-помощники (требование ручного регистратора N0)

Стюард закрытой бета-версии теперь может использовать регистратор через CLI без ручной сборки JSON:

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

- `--owner` по умолчанию использует конфигурацию аккаунта CLI; Повторите `--controller`, чтобы добавить дополнительные аккаунты контроллера (по умолчанию `[owner]`).
- Inline флаги платежа напрямую сопоставляются с `PaymentProofV1`; передайте `--payment-json PATH`, если уже есть структурированная квитанция. Метаданные (`--metadata-json`) и перехватчики управления (`--governance-json`) в следующем шаблоне.

Помощники, доступные только для чтения, до сложных повторений:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

См. `crates/iroha_cli/src/commands/sns.rs` для реализации; Команда повторно использует Norito DTO из этого документа, поэтому вывод CLI соответствует ответам Torii байт-в-байт.

Дополнительные помощники раскрывают продление, передачу и действие опекуна:

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

`--governance-json` должен поддерживать корректную запись `GovernanceHookV1` (идентификатор предложения, хэши голосования, управляющий/опекун бесплатной почты). Каждый просто отражает соответствующий эндпоинт `/v1/sns/registrations/{selector}/...`, чтобы операторы могли воспроизвести точные поверхности Torii, которые будут хранить SDK.

## 4. сервис gRPC

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

Wire-формат: хеш-схемы Norito на этапе компиляции преобразованы в
`fixtures/norito_rpc/schema_hashes.json` (строки `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1` и т. д. д.).

## 5. Хуки управления и доказательства

Каждый изменяющий вызов должен прикладывать доказательства, пригодные для ввода:

| Действие | Требуемые данные управления |
|----------|-----------------------------|
| Стандартная регистрация/продление | Доказательство платежа по ссылке на процедуру расчета; Голосование совета не нужно, если уровень не требует одобрения управляющего. |
| Регистрация премиум-уровня / зарезервированное задание | `GovernanceHookV1` со ссылкой на идентификатор предложения + подтверждение стюарда. |
| Передача | Хеш голосовой совет + хеш сигнала DAO; разрешение опекуна, когда передача инициирована развитием споры. |
| Заморозить/Разморозить | Подпись билета опекуна плюс отмена совета (разморозка). |

Torii последние доказательства, проверяя:

1. Идентификатор предложения существует в реестре управления (`/v1/governance/proposals/{id}`) и статусе `Approved`.
2. Хеши случился с нарисованными документами.
3. Подписки стюарда/опекуна ссылаются на ожидаемые публичные ключи из `SuffixPolicyV1`.

Неуспешные проверки возвращают `sns_err_governance_missing`.

## 6. Примеры рабочих процессов

### 6.1 Стандартная регистрация1. Клиент запрашивает `/v1/sns/policies/{suffix_id}`, чтобы получить цены, льготы и доступные уровни.
2. Клиент строит `RegisterNameRequestV1`:
   - `selector` получен из предпочитаемого I105 или второго по предпочтению сжатого (`sora`) ярлыка.
   - `term_years` в пределах политики.
   - `payment` ссылается на перевод сплиттера казначейства/стюарда.
3. Torii сначала:
   - Метки нормализации + зарезервировано.
   - Срок/цена брутто по сравнению с `PriceTierV1`.
   - Сумма доказательств платежа >= расчетной цены + комиссия.
4. В случае успеха Torii:
   - Сохраняет `NameRecordV1`.
   - Испускает `RegistryEventV1::NameRegistered`.
   - Испускает `RevenueAccrualEventV1`.
   - Возвращает новую запись + события.

### 6.2 Продление в период благодати

Продления в благодати включают стандартный запрос плюс обнаружение штрафа:

- Torii сравнивает `now` с `grace_expires_at` и добавляет таблицы с доплатой из `SuffixPolicyV1`.
- Доказательство платежа должно раскрывать дополнительную плату. Ошибка => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` записывает новый `expires_at`.

### 6.3 Заморозка Стража и отмена совета

1. Guardian отправляет `FreezeNameRequestV1` с билетом, ссылающимся на идентификатор происшествия.
2. Torii переводит запись в `NameStatus::Frozen`, испускает `NameFrozen`.
3. После блокировки совет выдает блокировку; оператор отправляет DELETE `/v1/sns/registrations/{selector}/freeze` с `GovernanceHookV1`.
4. Torii в последнее время переопределяет, испускает `NameUnfrozen`.

## 7. Проверка и коды ошибок

| Код | Описание | HTTP |
|-----|----------|------|
| `sns_err_reserved` | Метка зарезервирована или заблокирована. | 409 |
| `sns_err_policy_violation` | Срок, уровень или набор контроллеров обеспечения политики. | 422 |
| `sns_err_payment_mismatch` | Несоответствие значений или активов в доказательстве платежа. | 402 |
| `sns_err_governance_missing` | Отсутствуют/некорректны требуемые документы управления. | 403 |
| `sns_err_state_conflict` | Операция недопустима в текущем состоянии жизненного цикла. | 409 |

Все коды продаются через `X-Iroha-Error-Code` и структурированные Norito конверты JSON/NRPC.

## 8. Примечания по реализации

- Torii хранит ожидающие аукционы в `NameRecordV1.auction` и отклоняет прямую попытку регистрации, пока `PendingAuction`.
- Доказательства платежа переиспользуют поступления в бухгалтерскую книгу Norito; Казначейские сервисы предоставляют вспомогательный API (`/v1/finance/sns/payments`).
- SDK должен обернуть эти эндпоинты строго типизированными помощниками, чтобы кошельки могли показывать понятные причины ошибок (`ERR_SNS_RESERVED` и т. д.).

## 9. Следующие шаги

- Подключить обработчиков Torii к первому реестру контрактов после показа аукционов SN-3.
- Опубликовать SDK-руководства (Rust/JS/Swift), ссылающиеся на это API.
- Расширить [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) перекрестными ссылками на поля доказательства управления крючком.