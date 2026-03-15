---
lang: ru
direction: ltr
source: docs/portal/docs/sns/registrar-api.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание
یہ صفحہ `docs/source/sns/registrar_api.md` کی عکاسی کرتا ہے اور اب پورٹل
کی نونیکل کاپی ہے۔ سورس فائل ترجمہ PRs کے لئے برقرار رہتی ہے۔
:::

# API-интерфейс регистратора SNS для перехватчиков (SN-2b)

**Описание:** 24 марта 2026 г. - Nexus Core ریویو کے تحت  
**Дополнительно:** SN-2b «API регистратора и средства управления»  
**Нажмите здесь:** Установите флажок [`registry-schema.md`](./registry-schema.md) میں۔

Конечные точки Torii, службы gRPC, запросы/ответы DTO и управление
артефакты или артефакты, например, регистратор службы имен Sora (SNS).
درکار ہیں۔ SDK, кошельки и автоматизация, а также поддержка SNS и SNS.
رجسٹر، продлить یا управлять کرنا چاہتے ہیں۔

## 1. ٹرانسپورٹ اور توثیق

| شرط | تفصیل |
|-----|-------|
| پروٹوکولز | REST `/v1/sns/*` Доступ к службе gRPC `sns.v1.Registrar`۔ Используйте Norito-JSON (`application/json`) и Norito-RPC (`application/x-norito`). |
| Авторизация | `Authorization: Bearer` токены یا mTLS сертификаты ہر суффикс стюарда کی طرف سے جاری۔ конечные точки, чувствительные к управлению (замораживание/разблокирование, зарезервированные назначения) |
| ریٹ حدود | Регистраторы `torii.preauth_scheme_limits` сегментируют вызывающие абоненты JSON, а также суффикс اور ہر и взрывные ограничения: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`۔ |
| ٹیلیمیٹری | Torii обработчики регистратора کے لئے `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` ظاہر کرتا ہے (`scheme="norito_rpc"` фильтр); API-интерфейс `sns_registrar_status_total{result, suffix_id}` |

## 2. DTO خلاصہ

Например, [`registry-schema.md`](./registry-schema.md) можно найти канонические структуры, которые можно найти здесь. Полезные нагрузки `NameSelectorV1` + `SuffixId` могут использоваться для маршрутизации или маршрутизации.

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

| Конечная точка | طریقہ | Полезная нагрузка | تفصیل |
|----------|-------|---------|-------|
| `/v1/sns/registrations` | ПОСТ | `RegisterNameRequestV1` | Если вы хотите, чтобы это произошло, ценовой уровень حل کرتا ہے، доказательства оплаты/управления کی توثیق کرتا ہے، события реестра излучают کرتا ہے۔ |
| `/v1/sns/registrations/{selector}/renew` | ПОСТ | `RenewNameRequestV1` | مدت بڑھاتا ہے۔ Окна благодати/искупления |
| `/v1/sns/registrations/{selector}/transfer` | ПОСТ | `TransferNameRequestV1` | حکمرانی одобрения لگنے کے بعد منتقل کرتا ہے۔ |
| `/v1/sns/registrations/{selector}/controllers` | ПУТЬ | `UpdateControllersRequestV1` | контроллеры подписанные адреса учетных записей کی توثیق کرتا ہے۔ |
| `/v1/sns/registrations/{selector}/freeze` | ПОСТ | `FreezeNameRequestV1` | заморозка опекуна/совета۔ Билет опекуна и квитанция об управлении کا حوالہ درکار۔ |
| `/v1/sns/registrations/{selector}/freeze` | УДАЛИТЬ | `GovernanceHookV1` | исправление کے بعد разморозить; совет отвергает ریکارڈ ہونے کو یقینی بناتا ہے۔ |
| `/v1/sns/reserved/{selector}` | ПОСТ | `ReservedAssignmentRequestV1` | зарезервированные имена, управляющий/совет, назначение, назначение. |
| `/v1/sns/policies/{suffix_id}` | ПОЛУЧИТЬ | -- | `SuffixPolicyV1` موجودہ حاصل کرتا ہے (кэшируемый)۔ |
| `/v1/sns/registrations/{selector}` | ПОЛУЧИТЬ | -- | موجودہ `NameRecordV1` + موثر حالت (Active, Grace وغیرہ) واپس کرتا ہے۔ |

**Кодировка селектора:** `{selector}` сегмент пути I105, сжатый (`sora`) или канонический шестнадцатеричный ADDR-5, который может быть использован в качестве исходного кода. Torii `NameSelectorV1` سے нормализовать کرتا ہے۔**Модель ошибки:** Конечные точки Norito JSON `code`, `message`, `details` и многое другое. Коды `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Помощники CLI (N0 для регистратора)

Стюарды закрытого бета-тестирования, CLI и регистратор, а также файлы JSON и JSON:

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

- `--owner` Добавлена учетная запись конфигурации CLI; Для учетных записей контроллера используется `--controller` или `[owner]`.
- Флаги встроенных платежей براہ راست `PaymentProofV1` سے карта ہوتے ہیں؛ Структурированная квитанция ہو تو `--payment-json PATH` دیں۔ Метаданные (`--metadata-json`) и перехватчики управления (`--governance-json`)

Помощники, доступные только для чтения:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Реализация `crates/iroha_cli/src/commands/sns.rs` دیکھیں؛ Команды и команды Norito DTOs для вывода данных CLI Torii ответы побайтно.

Продление дополнительных помощников, переводы и действия опекуна или другие действия:

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

`--governance-json` میں درست `GovernanceHookV1` ریکارڈ ہونا چاہیے (идентификатор предложения, хеши голосования, подписи стюарда/опекуна)۔ ہر کمانڈ متعلقہ `/v1/sns/registrations/{selector}/...` endpoint کی عکاسی کرتی ہے تاکہ beta операторы بالکل وہی Torii поверхности репетируют Использование SDK или SDK

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

Wire-формат: хеш схемы Norito во время компиляции.
`fixtures/norito_rpc/schema_hashes.json` میں درج ہے (строки `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, وغیرہ).

## 5. Зацепки в управлении и доказательства

ہر мутирующий вызов и воспроизведение, а также доказательства, которые можно использовать в следующих случаях:

| Действие | Требуемые данные управления |
|--------|--------------------------|
| Стандартная регистрация/продление | Инструкция по расчету, см. подтверждение платежа. Голосование в совете или одобрение стюарда или одобрение стюарда |
| Регистрация премиум-уровня / зарезервированное назначение | `GovernanceHookV1` — идентификатор предложения + подтверждение от управляющего. |
| Трансфер | Хеш голосования совета + хеш сигнала DAO; разрешение опекуна جب разрешение спора о передаче سے триггер ہو۔ |
| Заморозить/Разморозить | Подпись билета опекуна کے ساتھ совет отменить (разморозить)۔ |

Torii доказательства того, что вам нужно знать:

1. Идентификатор предложения в реестре управления (`/v1/governance/proposals/{id}`) Код статуса `Approved` ہے۔
2. хеши ریکارڈ شدہ голосование артефакты سے совпадение کرتے ہیں۔
3. Подписи управляющего/опекуна `SuffixPolicyV1` سے متوقع открытые ключи, пожалуйста, обратитесь к کرتے ہیں۔

Неудачные проверки `sns_err_governance_missing` واپس کرتے ہیں۔

## 6. Примеры рабочих процессов

### 6.1 Стандартная регистрация1. Клиент `/v1/sns/policies/{suffix_id}` для запроса, определения цен, благодати и уровней доступа.
2. Клиент `RegisterNameRequestV1` بناتا ہے:
   - `selector` ترجیحی I105 یا вторая по качеству сжатая (`sora`) метка سے, производная ہے۔
   - `term_years` پالیسی حدود میں۔
   - `payment` казначейский/распределительный перевод управляющего, обратитесь к нам ہے۔
3. Torii подтвердите правильность:
   - Нормализация меток + зарезервированный список.
   - Срок/цена брутто по сравнению с `PriceTierV1`.
   - Сумма подтверждения платежа >= расчетная цена + комиссия.
4. Код Torii:
   - `NameRecordV1` محفوظ کرتا ہے۔
   - `RegistryEventV1::NameRegistered` излучает کرتا ہے۔
   - `RevenueAccrualEventV1` излучает کرتا ہے۔
   - نیا запись + события واپس کرتا ہے۔

### 6.2 Продление во время льготного периода

Льготное продление и стандартный запрос, а также обнаружение штрафов شامل ہے:

- Torii `now` vs `grace_expires_at` Таблица дополнительных сборов для `SuffixPolicyV1`.
- Подтверждение оплаты и покрытие дополнительной платы в случае необходимости. Сбой => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` и `expires_at` ریکارڈ کرتا ہے۔

### 6.3 Заморозка Стража и блокировка Совета

1. Guardian `FreezeNameRequestV1` отправьте сообщение с идентификатором инцидента или билетом на следующий день.
2. Torii записывает `NameStatus::Frozen`, выдает сигнал `NameFrozen`, выдает сигнал.
3. Исправление может быть отменено советом جاری کرتا ہے؛ Оператор DELETE `/v1/sns/registrations/{selector}/freeze` или `GovernanceHookV1` کے ساتھ بھیجتا ہے۔
4. Torii переопределить проверку проверки, `NameUnfrozen` выдать команду

## 7. Коды проверки и ошибок

| Код | Описание | HTTP |
|------|-------------|------|
| `sns_err_reserved` | Ярлык зарезервирован یا заблокирован ہے۔ | 409 |
| `sns_err_policy_violation` | Термин, контроллеры уровня или контроллеры уровня | 422 |
| `sns_err_payment_mismatch` | Доказательство оплаты میں стоимость یا несоответствие актива۔ | 402 |
| `sns_err_governance_missing` | Требуемые артефакты управления غائب/invalid ہیں۔ | 403 |
| `sns_err_state_conflict` | Состояние жизненного цикла Разрешенная операция نہیں۔ | 409 |

Коды `X-Iroha-Error-Code` и структурированные Norito Конверты JSON/NRPC для поверхностной обработки

## 8. Замечания по реализации

- Torii ожидающие аукционы `NameRecordV1.auction` Отклоните попытку прямой регистрации `PendingAuction`.
- Подтверждения оплаты Norito квитанции из бухгалтерской книги. Вспомогательные API казначейских служб (`/v1/finance/sns/payments`)
- SDK, конечные точки, строго типизированные помощники, обертка, кошельки и причины ошибок (`ERR_SNS_RESERVED`, нет).

## 9. Следующие шаги

- Аукционы SN-3, обработчики Torii, контракт на реестр и проводной обмен.
- Руководства по SDK (Rust/JS/Swift).
- [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) Поля доказательств управления, перекрестные ссылки и расширение.