---
lang: ru
direction: ltr
source: docs/portal/docs/sns/registrar-api.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание
تعكس هذه الصفحة `docs/source/sns/registrar_api.md` وتعمل انسخة بوابة
قياسية. Он был убит в 2007 году.
:::

# Дополнительные крючки для SNS (SN-2b)

**Обращение:** Дата: 24 марта 2026 г. — добавлено Nexus Core.  
**Обновление:** SN-2b «API регистратора и средства управления»  
**Запись:** Доступно для [`registry-schema.md`](./registry-schema.md)

Для этого необходимо выполнить Torii для gRPC и проверки/проверки DTO.
Сообщение об открытии канала SNS. Использование SDK
Вы можете создать приложение для социальных сетей и социальных сетей.

## 1. Устранение неполадок

| المتطلب | تفاصيل |
|---------|----------|
| بروتوكولات | REST включает `/v1/sns/*` и gRPC `sns.v1.Registrar`. Используйте Norito-JSON (`application/json`) и Norito-RPC (`application/x-norito`). |
| Авторизация | Добавьте `Authorization: Bearer` в mTLS с суффиксом стюарда. Чтобы выполнить функцию блокировки (заморозить/разморозить, отключить заморозку), установите `scope=sns.admin`. |
| حدود المعدل | Вызовите ведра `torii.preauth_scheme_limits` с JSON-файлом, чтобы получить взрывной сигнал: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| القياس | Torii يعرض `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` Внутренний блок (رشح `scheme="norito_rpc"`); Был установлен `sns_registrar_status_total{result, suffix_id}`. |

## 2. Добавление DTO

Используйте его для создания файла [`registry-schema.md`](./registry-schema.md). Для этого используйте `NameSelectorV1` + `SuffixId`.

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

## 3. ОТДЫХ

| نقطة النهاية | طريقة | حمولة | الوصف |
|-------------|---------|---------|-------|
| `/v1/sns/names` | ПОСТ | `RegisterNameRequestV1` | تسجيل او اعادة فتح اسم. Он был создан в 2017 году в рамках программы "Старый/Старый мир" в 1990 году. |
| `/v1/sns/names/{namespace}/{literal}/renew` | ПОСТ | `RenewNameRequestV1` | يمدد المدة. يفرض نوافذ благодать/искупление в السياسة. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | ПОСТ | `TransferNameRequestV1` | Он был убит в 2007 году. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | ПУТЬ | `UpdateControllersRequestV1` | Контроллеры управления Это произошло в Сан-Франциско. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | ПОСТ | `FreezeNameRequestV1` | تجميد опекун/совет. Его отец - опекун. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | УДАЛИТЬ | `GovernanceHookV1` | В действительности, это не так. , ,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,. |
| `/v1/sns/reserved/{selector}` | ПОСТ | `ReservedAssignmentRequestV1` | تعيين اسماء محجوزة بواسطة стюард / совет. |
| `/v1/sns/policies/{suffix_id}` | ПОЛУЧИТЬ | -- | يجلب `SuffixPolicyV1` الحالي (قابل للكاش). |
| `/v1/sns/names/{namespace}/{literal}` | ПОЛУЧИТЬ | -- | يعيد `NameRecordV1` الحالي + الحالة الفعلية (Активный, Грейс, الخ). |

**Селектор выбора:** `{selector}` для i105 или шестнадцатеричного формата ADDR-5; Torii — это `NameSelectorV1`.

**Открытие:** при использовании Norito JSON для `code`, `message`, `details`. Доступны `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Поддержка CLI (ограничение доступа N0)

В списке стюардов, представленном в журнале CLI, есть JSON с кодом:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```- `--owner` для доступа к CLI; Установите `--controller` в блок управления контроллером (`[owner]`).
- Установите флажок `PaymentProofV1`; Код `--payment-json PATH` был отправлен в США. Метаданные (`--metadata-json`) и перехватчики (`--governance-json`) можно использовать.

Сообщение от президента США:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

راجع `crates/iroha_cli/src/commands/sns.rs` للتنفيذ; Создан файл DTOات Norito, который находится в режиме ожидания. Для CLI используется Torii.

مساعدات اضافية تغطي التجديدات والتحويلات واجراءات Guardian:

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

`--governance-json` указывается в списке `GovernanceHookV1` (идентификатор предложения, хэши голосования, управляющий/опекун). Для получения дополнительной информации обратитесь к `/v1/sns/names/{namespace}/{literal}/...`. В версии Torii используются SDK.

## 4. Запуск gRPC

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

Формат проводного соединения: хэш Norito в исходном коде.
`fixtures/norito_rpc/schema_hashes.json` (صفوف `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, الخ).

## 5. крючки

В сообщении, опубликованном в журнале "Миниатюра", говорится:

| Новости | بيانات الحوكمة المطلوبة |
|---------|-------------------------|
| Справочник/помощник повара | اثبات دفع يشير الى تعليمات поселение; Он был назначен стюардом в качестве стюарда. |
| Купюра премиум-класса / تعيين محجوز | `GovernanceHookV1` — идентификатор предложения + управляющий. |
| نقل | хэш تصويت المجلس + хэш اشارة DAO; опекун по допуску Дэниел Хейлз Сити. |
| تجميد/فك تجميد | Свою роль опекуна отменяет действие Защитника (в случае с исчезновением). |

Torii добавлен в раздел:

1. Идентификатор предложения указывается в دفتر الحوكمة (`/v1/governance/proposals/{id}`) и `Approved`.
2. Использование хэшей может быть выполнено в режиме реального времени.
3. Управляющий/опекун تشير الى المفاتيح العامة المتوقعة من `SuffixPolicyV1`.

Это приложение `sns_err_governance_missing`.

## 6. امثلة سير العمل

### 6.1.

1. Установите флажок `/v1/sns/policies/{suffix_id}` для получения дополнительной информации о Грейс.
2. Для `RegisterNameRequestV1`:
   - `selector` находится на этикетке i105 (открыто) и (отсутствует).
   - `term_years` добавлен в программу.
   - `payment` находится в сплиттере/стюарде.
3. Torii Код:
   - Ярлык تطبيع + قائمة محجوزة.
   - Срок/цена брутто по сравнению с `PriceTierV1`.
   - مبلغ اثبات الدفع >= السعر المحسوب + الرسوم.
4. Установите флажок Torii:
   - Код `NameRecordV1`.
   - Код `RegistryEventV1::NameRegistered`.
   - Код `RevenueAccrualEventV1`.
   - يعيد السجل الجديد + الاحداث.

### 6.2 Обращение к благодати

Обратилась к Грейс Тэйлз Уилсону из фильма "Страна":

- Torii вместо `now` для `grace_expires_at` за дополнительную плату для `SuffixPolicyV1`.
- Стоимость трансфера в отеле за дополнительную плату. فشل => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` вместо `expires_at`.

### 6.3 Добавление опекуна и переопределение режима1. опекун يرسل `FreezeNameRequestV1` مع تذكرة تشير الى id حادث.
2. Torii вместо `NameStatus::Frozen`, вместо `NameFrozen`.
3. Переопределение режима блокировки; Нажмите кнопку DELETE `/v1/sns/names/{namespace}/{literal}/freeze` вместо `GovernanceHookV1`.
4. Torii используется для переопределения, а также `NameUnfrozen`.

## 7. التحقق واكواد الخطا

| الكود | الوصف | HTTP |
|-------|-------|------|
| `sns_err_reserved` | Сделайте это и сделайте это. | 409 |
| `sns_err_policy_violation` | Используйте контроллеры контроллеров, чтобы получить доступ к ним. | 422 |
| `sns_err_payment_mismatch` | Он был использован в качестве актива в рамках проекта. | 402 |
| `sns_err_governance_missing` | اثار الحوكمة المطلوبة غائبة/غير صالحة. | 403 |
| `sns_err_state_conflict` | Это было сделано в честь Дня Победы. | 409 |

Загрузите файл `X-Iroha-Error-Code` и файл Norito JSON/NRPC.

## 8. Устранение неполадок

- Torii для проверки работоспособности `NameRecordV1.auction` для проверки работоспособности Был установлен `PendingAuction`.
- اثباتات الدفع تعيد استخدام ايصالات دفتر Norito; Проверьте API-интерфейсы (`/v1/finance/sns/payments`).
- Используйте SDK для получения дополнительной информации о возможностях использования. Установите флажок (`ERR_SNS_RESERVED`, الخ).

## 9. Дополнительная информация

- Установлен Torii, установленный на борту SN-3.
- Использование SDK (Rust/JS/Swift) для создания приложений.
- Установите [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) Установите крючки на крючки.