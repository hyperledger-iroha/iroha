---
lang: he
direction: rtl
source: docs/portal/docs/sns/registrar-api.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Эта страница отражает `docs/source/sns/registrar_api.md` ו теперь служит
канонической копией портала. Исходный файл остается для PR переводов.
:::

# API регистратора SNS и хуки управления (SN-2b)

** סטאטוס:** תאריך 2026-03-24 — על Nexus Core  
**מפת הדרכים של Ссылка:** SN-2b "Registrar API & Governance Hooks"  
**פריטים:** Определения схемы в [`registry-schema.md`](./registry-schema.md)

Эта заметка описывает эндпоинты Torii, SERвисы gRPC, DTO запросов/ответов ו
артефакты управления, необходимые для работы регистратора שירות שמות של סורה
(SNS). Это авторитетный контракт для SDK, кошельков автоматизации, которым
нужно регистрировать, продлевать או управлять SNS-именами.

## 1. ТрансPORT и аутентификация

| Требование | Детали |
|------------|--------|
| Протоколы | REST под `/v2/sns/*` ו-gRPC сервис `sns.v1.Registrar`. Оба принимают Norito-JSON (`application/json`) ו-двоичный Norito-RPC (`application/x-norito`). |
| Auth | `Authorization: Bearer` טוקונים או mTLS сертификаты, выданные по סיומת דייל. Чувствительные к управлению эндпоинты (הקפאה/ביטול הקפאה, מטלות שמורות) требуют `scope=sns.admin`. |
| Ограничения | הרשם делят buckets `torii.preauth_scheme_limits` עם JSON вызывающими плюс лимиты всплеска по סיומת: `sns.register`, Norito, Norito, Norito `sns.freeze`. |
| Телеметрия | Torii публикует `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` לרשם обработчиков (фильтр по `scheme="norito_rpc"`); API также увеличивает `sns_registrar_status_total{result, suffix_id}`. |

## 2. Обзор DTO

Поля ссылаются на канонические структуры, определенные в [`registry-schema.md`](./registry-schema.md). Все מטענים встраивают `NameSelectorV1` + `SuffixId`, чтобы избежать неоднозначной маршрутизации.

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

## 3. מנוחה эндпоинты

| Эндпоинт | Метод | מטען | Описание |
|--------|-------|--------|--------|
| `/v2/sns/registrations` | פוסט | `RegisterNameRequestV1` | Регистрирует или повторно открывает имя. Разрешает ценовой tier, проверяет доказательства платежа/управления, испускает события реестра. |
| `/v2/sns/registrations/{selector}/renew` | פוסט | `RenewNameRequestV1` | Продлевает срок. Применяет окна חסד/גאולה из политики. |
| `/v2/sns/registrations/{selector}/transfer` | פוסט | `TransferNameRequestV1` | Передает владение после прикрепления согласований управления. |
| `/v2/sns/registrations/{selector}/controllers` | PUT | `UpdateControllersRequestV1` | בקרי Заменяет набор; проверяет подписанные адреса аккаунтов. |
| `/v2/sns/registrations/{selector}/freeze` | פוסט | `FreezeNameRequestV1` | הקפאת אפוטרופוס/מועצה. Требует כרטיס אפוטרופוס и ссылку на מסמך ממשל. |
| `/v2/sns/registrations/{selector}/freeze` | מחק | `GovernanceHookV1` | Unfreeze после устранения; убеждается, что המועצה לעקוף את зафиксирован. |
| `/v2/sns/reserved/{selector}` | פוסט | `ReservedAssignmentRequestV1` | Назначение שמור имен דייל/מועצה. |
| `/v2/sns/policies/{suffix_id}` | קבל | -- | Получает текущую `SuffixPolicyV1` (кэшируемо). |
| `/v2/sns/registrations/{selector}` | קבל | -- | Возвращает текущий `NameRecordV1` + эффективное состояние (Active, Grace, и т. д.). |**בורר כונן:** סמן `{selector}` принимает I105, דחוס (`sora`) או קנונית משומשת ל-ADDR-5; Torii נורמאלי של `NameSelectorV1`.

**Модель ошибок:** все эндпоинты возвращают Norito JSON с `code`, `message`, I100NI730. Коды включают `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 CLI помощники (требование ручного регистратора N0)

סטורד закрытой беты теперь могут использовать רשם через CLI без ручной сборки JSON:

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

- `--owner` по умолчанию использует аккаунт конфигурации CLI; התקן את `--controller`, התקן את אביזרי הבקר של `[owner]`.
- Inline флаги платежа напрямую сопоставляются с `PaymentProofV1`; передайте `--payment-json PATH`, если уже есть структурированная квитанция. מטא נתונים (`--metadata-json`) ו-Governance Hooks (`--governance-json`) следуют тому же шаблону.

מידע לקריאה בלבד:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

См. `crates/iroha_cli/src/commands/sns.rs` לרכישה; команды повторно используют Norito DTO из этого документа, поэтому вывод CLI совпадает с от108NTа10X байт-в-байт.

Дополнительные помощники покрывают продления, передачи и действия אפוטרופוס:

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

`--governance-json` должен содержать корректную запись `GovernanceHookV1` (מזהה הצעה, גיבוב הצבעה, דייל/אפוטרופוס). Каждая команда просто отражает соответствующий эндпоинт `/v2/sns/registrations/{selector}/...` чтобы операторы беты могтьтри поверхности Torii, которые будут вызывать SDK.

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

פורמט חוט: хеш схемы Norito на этапе компиляции записан в
`fixtures/norito_rpc/schema_hashes.json` (строки `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, וכן т. д.).

## 5. Хуки управления и доказательства

Каждый изменяющий вызов должен приложить доказательства, пригодные для воспроизведения:

| Действие | Требуемые данные управления |
|--------|--------------------------------|
| Стандартная регистрация/продление | Доказательство платежа со ссылкой на התנחלות инструкцию; מועצת голосование не нужно, если tier не требует одобрения דייל. |
| Регистрация שכבת פרימיום / משימה שמורה | `GovernanceHookV1` со ссылкой на הצעה מזהה + אישור דייל. |
| Передача | מועצת Хеш голосования + хеш сигнала DAO; אישור אפוטרופוס, когда передача инициирована разрешением спора. |
| הקפאה/בטל הקפאה | Подпись כרטיס אפוטרופוס плюс ביטול המועצה (ביטול הקפאה). |

Torii проверяет доказательства, проверяя:

1. מזהה הצעה существует в Ledger управления (`/v2/governance/proposals/{id}`) ו статус `Approved`.
2. Хеши совпадают с записанными артефактами голосования.
3. בדיקת דייל/אפוטרופוס ссылаются на ожидаемые публичные ключи из `SuffixPolicyV1`.

Неуспешные проверки возвращают `sns_err_governance_missing`.

## 6. Примеры рабочих процессов

### 6.1 Стандартная регистрация1. Client запрашивает `/v2/sns/policies/{suffix_id}` чтобы получить цены, grace и доступные tiers.
2. Клиент строит `RegisterNameRequestV1`:
   - תווית `selector` מתוצרת I105 או תווית דחוסה (`sora`).
   - `term_years` в пределах политики.
   - `payment` ссылается на перевод מפצל אוצר/דייל.
3. Torii הצג:
   - Нормализацию метки + список שמור.
   - טווח/מחיר ברוטו לעומת `PriceTierV1`.
   - Сумма доказательства платежа >= рассчитанной цены + комиссии.
4. В случае успеха Torii:
   - Сохраняет `NameRecordV1`.
   - Испускает `RegistryEventV1::NameRegistered`.
   - Испускает `RevenueAccrualEventV1`.
   - Возвращает новую запись + события.

### 6.2 Продление в период חסד

Продления в grace включают стандартный запрос плюс детект штрафа:

- Torii сравнивает `now` עם `grace_expires_at` ותוספת תשלום עבור `SuffixPolicyV1`.
- תוספת תשלום עבור Доказательство платежа должно покрывать. Ошибка => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` записывает новый `expires_at`.

### 6.3 הקפאת אפוטרופוס ועקיפה של המועצה

1. Guardian отправляет `FreezeNameRequestV1` с כרטיס, ссылающимся на id инцидента.
2. Torii переводит запись в `NameStatus::Frozen`, испускает `NameFrozen`.
3. После устранения המועצה выпускает לעקוף; оператор отправляет DELETE `/v2/sns/registrations/{selector}/freeze` עם `GovernanceHookV1`.
4. Torii проверяет לעקוף, испускает `NameUnfrozen`.

## 7. Валидация и коды ошибок

| Код | Описание | HTTP |
|-----|----------------|------|
| `sns_err_reserved` | Метка зарезервирована или заблокирована. | 409 |
| `sns_err_policy_violation` | Срок, tier или набор בקרי нарушает политику. | 422 |
| `sns_err_payment_mismatch` | Несоответствие значения или актива в доказательстве платежа. | 402 |
| `sns_err_governance_missing` | Отсутствуют/некорректны требуемые артефакты управления. | 403 |
| `sns_err_state_conflict` | Операция недопустима в текущем состоянии жизненного цикла. | 409 |

Все коды пробрасываются через `X-Iroha-Error-Code` ו-структурированные Norito JSON/NRPC конверты.

## 8. Примечания по реализации

- Torii בהמתנה аукционы в `NameRecordV1.auction` и отклоняет прямые попытки регистрации, пока Norito.
- Доказательства платежа переиспользуют Norito קבלות פנקס חשבונות; Treasury сервисы предоставляют עוזר API (`/v2/finance/sns/payments`).
- SDK должны обернуть эти эндпоинты строго типизированными помощниками, чтобы кошельки могли покатьнич ошибок (`ERR_SNS_RESERVED`, и т. д.).

## 9. Следующие шаги

- בדוק את Torii עבור מערכת התקשרות שרת עבור аукционов SN-3.
- Опубликовать SDK-руководства (Rust/JS/Swift), сссылающиеся на это API.
- Расширить [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) перекрестными ссылками на поля доказательств משילות.