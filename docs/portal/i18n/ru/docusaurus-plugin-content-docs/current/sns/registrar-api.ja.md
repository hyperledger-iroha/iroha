---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sns/registrar-api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9121c34636d66019f7244eef7f192fe62cf785ac463ffc35c5c7dd3ad7f0365
source_last_modified: "2026-01-28T17:58:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: registrar-api
lang: ru
direction: ltr
source: docs/portal/docs/sns/registrar-api.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Канонический источник
Эта страница отражает `docs/source/sns/registrar_api.md` и теперь служит
канонической копией портала. Исходный файл остается для PR переводов.
:::

# API регистратора SNS и хуки управления (SN-2b)

**Статус:** Подготовлен 2026-03-24 — на рассмотрении Nexus Core  
**Ссылка roadmap:** SN-2b "Registrar API & governance hooks"  
**Предпосылки:** Определения схемы в [`registry-schema.md`](./registry-schema.md)

Эта заметка описывает эндпоинты Torii, сервисы gRPC, DTO запросов/ответов и
артефакты управления, необходимые для работы регистратора Sora Name Service
(SNS). Это авторитетный контракт для SDK, кошельков и автоматизации, которым
нужно регистрировать, продлевать или управлять SNS-именами.

## 1. Транспорт и аутентификация

| Требование | Детали |
|------------|--------|
| Протоколы | REST под `/v2/sns/*` и gRPC сервис `sns.v1.Registrar`. Оба принимают Norito-JSON (`application/json`) и двоичный Norito-RPC (`application/x-norito`). |
| Auth | `Authorization: Bearer` токены или mTLS сертификаты, выданные по suffix steward. Чувствительные к управлению эндпоинты (freeze/unfreeze, reserved assignments) требуют `scope=sns.admin`. |
| Ограничения | registrar делят buckets `torii.preauth_scheme_limits` с JSON вызывающими плюс лимиты всплеска по suffix: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Телеметрия | Torii публикует `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` для обработчиков registrar (фильтр по `scheme="norito_rpc"`); API также увеличивает `sns_registrar_status_total{result, suffix_id}`. |

## 2. Обзор DTO

Поля ссылаются на канонические структуры, определенные в [`registry-schema.md`](./registry-schema.md). Все payloads встраивают `NameSelectorV1` + `SuffixId`, чтобы избежать неоднозначной маршрутизации.

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

| Эндпоинт | Метод | Payload | Описание |
|----------|-------|---------|----------|
| `/v2/sns/registrations` | POST | `RegisterNameRequestV1` | Регистрирует или повторно открывает имя. Разрешает ценовой tier, проверяет доказательства платежа/управления, испускает события реестра. |
| `/v2/sns/registrations/{selector}/renew` | POST | `RenewNameRequestV1` | Продлевает срок. Применяет окна grace/redemption из политики. |
| `/v2/sns/registrations/{selector}/transfer` | POST | `TransferNameRequestV1` | Передает владение после прикрепления согласований управления. |
| `/v2/sns/registrations/{selector}/controllers` | PUT | `UpdateControllersRequestV1` | Заменяет набор controllers; проверяет подписанные адреса аккаунтов. |
| `/v2/sns/registrations/{selector}/freeze` | POST | `FreezeNameRequestV1` | Freeze guardian/council. Требует guardian ticket и ссылку на governance docket. |
| `/v2/sns/registrations/{selector}/freeze` | DELETE | `GovernanceHookV1` | Unfreeze после устранения; убеждается, что council override зафиксирован. |
| `/v2/sns/reserved/{selector}` | POST | `ReservedAssignmentRequestV1` | Назначение reserved имен steward/council. |
| `/v2/sns/policies/{suffix_id}` | GET | -- | Получает текущую `SuffixPolicyV1` (кэшируемо). |
| `/v2/sns/registrations/{selector}` | GET | -- | Возвращает текущий `NameRecordV1` + эффективное состояние (Active, Grace, и т. д.). |

**Кодирование selector:** сегмент `{selector}` принимает I105, I105 или канонический hex по ADDR-5; Torii нормализует через `NameSelectorV1`.

**Модель ошибок:** все эндпоинты возвращают Norito JSON с `code`, `message`, `details`. Коды включают `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 CLI помощники (требование ручного регистратора N0)

Steward закрытой беты теперь могут использовать registrar через CLI без ручной сборки JSON:

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

- `--owner` по умолчанию использует аккаунт конфигурации CLI; повторяйте `--controller`, чтобы добавить дополнительные controller аккаунты (по умолчанию `[owner]`).
- Inline флаги платежа напрямую сопоставляются с `PaymentProofV1`; передайте `--payment-json PATH`, если уже есть структурированная квитанция. Metadata (`--metadata-json`) и governance hooks (`--governance-json`) следуют тому же шаблону.

Read-only помощники дополняют репетиции:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

См. `crates/iroha_cli/src/commands/sns.rs` для реализации; команды повторно используют Norito DTO из этого документа, поэтому вывод CLI совпадает с ответами Torii байт-в-байт.

Дополнительные помощники покрывают продления, передачи и действия guardian:

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

`--governance-json` должен содержать корректную запись `GovernanceHookV1` (proposal id, vote hashes, подписи steward/guardian). Каждая команда просто отражает соответствующий эндпоинт `/v2/sns/registrations/{selector}/...` чтобы операторы беты могли репетировать точные поверхности Torii, которые будут вызывать SDK.

## 4. gRPC сервис

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

Wire-format: хеш схемы Norito на этапе компиляции записан в
`fixtures/norito_rpc/schema_hashes.json` (строки `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, и т. д.).

## 5. Хуки управления и доказательства

Каждый изменяющий вызов должен приложить доказательства, пригодные для воспроизведения:

| Действие | Требуемые данные управления |
|----------|-----------------------------|
| Стандартная регистрация/продление | Доказательство платежа со ссылкой на settlement инструкцию; голосование council не нужно, если tier не требует одобрения steward. |
| Регистрация premium tier / reserved assignment | `GovernanceHookV1` со ссылкой на proposal id + steward acknowledgement. |
| Передача | Хеш голосования council + хеш сигнала DAO; guardian clearance, когда передача инициирована разрешением спора. |
| Freeze/Unfreeze | Подпись guardian ticket плюс council override (unfreeze). |

Torii проверяет доказательства, проверяя:

1. proposal id существует в ledger управления (`/v2/governance/proposals/{id}`) и статус `Approved`.
2. Хеши совпадают с записанными артефактами голосования.
3. Подписи steward/guardian ссылаются на ожидаемые публичные ключи из `SuffixPolicyV1`.

Неуспешные проверки возвращают `sns_err_governance_missing`.

## 6. Примеры рабочих процессов

### 6.1 Стандартная регистрация

1. Клиент запрашивает `/v2/sns/policies/{suffix_id}` чтобы получить цены, grace и доступные tiers.
2. Клиент строит `RegisterNameRequestV1`:
   - `selector` получен из предпочитаемого I105 или второго по предпочтению I105 label.
   - `term_years` в пределах политики.
   - `payment` ссылается на перевод splitter treasury/steward.
3. Torii проверяет:
   - Нормализацию метки + reserved список.
   - Term/gross price vs `PriceTierV1`.
   - Сумма доказательства платежа >= рассчитанной цены + комиссии.
4. В случае успеха Torii:
   - Сохраняет `NameRecordV1`.
   - Испускает `RegistryEventV1::NameRegistered`.
   - Испускает `RevenueAccrualEventV1`.
   - Возвращает новую запись + события.

### 6.2 Продление в период grace

Продления в grace включают стандартный запрос плюс детект штрафа:

- Torii сравнивает `now` с `grace_expires_at` и добавляет таблицы surcharge из `SuffixPolicyV1`.
- Доказательство платежа должно покрывать surcharge. Ошибка => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` записывает новый `expires_at`.

### 6.3 Guardian freeze и council override

1. Guardian отправляет `FreezeNameRequestV1` с ticket, ссылающимся на id инцидента.
2. Torii переводит запись в `NameStatus::Frozen`, испускает `NameFrozen`.
3. После устранения council выпускает override; оператор отправляет DELETE `/v2/sns/registrations/{selector}/freeze` с `GovernanceHookV1`.
4. Torii проверяет override, испускает `NameUnfrozen`.

## 7. Валидация и коды ошибок

| Код | Описание | HTTP |
|-----|----------|------|
| `sns_err_reserved` | Метка зарезервирована или заблокирована. | 409 |
| `sns_err_policy_violation` | Срок, tier или набор controllers нарушает политику. | 422 |
| `sns_err_payment_mismatch` | Несоответствие значения или актива в доказательстве платежа. | 402 |
| `sns_err_governance_missing` | Отсутствуют/некорректны требуемые артефакты управления. | 403 |
| `sns_err_state_conflict` | Операция недопустима в текущем состоянии жизненного цикла. | 409 |

Все коды пробрасываются через `X-Iroha-Error-Code` и структурированные Norito JSON/NRPC конверты.

## 8. Примечания по реализации

- Torii хранит pending аукционы в `NameRecordV1.auction` и отклоняет прямые попытки регистрации, пока `PendingAuction`.
- Доказательства платежа переиспользуют Norito ledger receipts; treasury сервисы предоставляют helper API (`/v2/finance/sns/payments`).
- SDK должны обернуть эти эндпоинты строго типизированными помощниками, чтобы кошельки могли показывать понятные причины ошибок (`ERR_SNS_RESERVED`, и т. д.).

## 9. Следующие шаги

- Подключить обработчики Torii к реальному контракту реестра после появления аукционов SN-3.
- Опубликовать SDK-руководства (Rust/JS/Swift), ссылающиеся на это API.
- Расширить [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) перекрестными ссылками на поля доказательств governance hook.
