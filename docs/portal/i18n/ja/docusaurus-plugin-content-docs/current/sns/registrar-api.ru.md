---
lang: ja
direction: ltr
source: docs/portal/docs/sns/registrar-api.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Эта страница отражает `docs/source/sns/registrar_api.md` и теперь служит
канонической копией портала。 Исходный файл остается для PR переводов.
:::

# API регистратора SNS и хуки управления (SN-2b)

**Статус:** Подготовлен 2026-03-24 — на рассмотрении Nexus Core  
**ロードマップ:** SN-2b「レジストラー API とガバナンス フック」  
**Предпосылки:** Определения схемы в [`registry-schema.md`](./registry-schema.md)

Эта заметка описывает эндпоинты Torii、сервисы gRPC、DTO запросов/ответов и
Sora Name Service の名前サービス
（SNS）。 Это авторитетный контракт для SDK、кольков и автоматизации、которым
SNS にアクセスできます。

## 1. Транспорт и аутентификация

| Требование |ださい |
|-----------|----------|
| Протоколы | REST は `/v1/sns/*` および gRPC は `sns.v1.Registrar` です。 Norito-JSON (`application/json`) と Norito-RPC (`application/x-norito`) を参照してください。 |
|認証 | `Authorization: Bearer` токены или mTLS のサフィックス スチュワード。 Чувствительные к управлению эндпоинты (凍結/凍結解除、予約された割り当て) требуют `scope=sns.admin`。 |
| Ограничения |レジストラのバケット `torii.preauth_scheme_limits` с JSON вызывающими плюс лимиты всплеска по suffix: `sns.register`、`sns.renew`、`sns.controller`、 `sns.freeze`。 |
| Телеметрия | Torii публикует `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` レジストラ (фильтр по `scheme="norito_rpc"`); API は `sns_registrar_status_total{result, suffix_id}` です。 |

## 2. Обзор DTO

[`registry-schema.md`](./registry-schema.md) を参照してください。ペイロードは `NameSelectorV1` + `SuffixId` に相当します。

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

## 3. 休息

| Эндпоинт | Метод |ペイロード | Описание |
|----------|----------|----------|----------|
| `/v1/sns/registrations` |投稿 | `RegisterNameRequestV1` | Регистрирует или повторно открывает имя. Разрезает ценовой tier, проверяет доказательства платежа/управления, испускает события реестра. |
| `/v1/sns/registrations/{selector}/renew` |投稿 | `RenewNameRequestV1` | Продлевает срок.恵みと救いを意味します。 |
| `/v1/sns/registrations/{selector}/transfer` |投稿 | `TransferNameRequestV1` | Передает владение после прикрепления согласований управления。 |
| `/v1/sns/registrations/{selector}/controllers` |置く | `UpdateControllersRequestV1` |コントローラーを使用します。必要に応じて、必要な情報を入力してください。 |
| `/v1/sns/registrations/{selector}/freeze` |投稿 | `FreezeNameRequestV1` |ガーディアン/評議会を凍結します。ガーディアンチケットとガバナンスドケットを参照してください。 |
| `/v1/sns/registrations/{selector}/freeze` |削除 | `GovernanceHookV1` |凍結を解除してください。 убеждается、議会オーバーライド зафиксирован。 |
| `/v1/sns/reserved/{selector}` |投稿 | `ReservedAssignmentRequestV1` | Назначение 予約されたスチュワード/評議会。 |
| `/v1/sns/policies/{suffix_id}` |入手 | -- | Получает текущую `SuffixPolicyV1` (кэвируемо)。 |
| `/v1/sns/registrations/{selector}` |入手 | -- | Возвращает текущий `NameRecordV1` + эффективное состояние (アクティブ、グレース、и т. д.)。 |

**セレクター:** `{selector}` は I105、圧縮 (`sora`) または 16 進数、ADDR-5; Torii нормализует через `NameSelectorV1`。

**例:** Norito JSON と `code`、`message`、`details`。 `sns_err_reserved`、`sns_err_payment_mismatch`、`sns_err_policy_violation`、`sns_err_governance_missing`。

### 3.1 CLI バージョン (требование ручного регистратора N0)

スチュワード セッション レジストラ セッション CLI セッション JSON:

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

- `--owner` は CLI を使用します。 `--controller`、コントローラー аккаунты (`[owner]`) を使用してください。
- インライン флаги платежа напрямую сопоставляются с `PaymentProofV1`; `--payment-json PATH` を確認してください。メタデータ (`--metadata-json`) とガバナンス フック (`--governance-json`) が必要です。

読み取り専用のファイル:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

См. `crates/iroha_cli/src/commands/sns.rs` 日。 команды повторно используют Norito DTO из этого документа, поэтому вывод CLI совпадает с ответами Torii байт-в-байт。

保護者、保護者、保護者:

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

`--governance-json` должен содержать корректную запись `GovernanceHookV1` (提案 ID、投票ハッシュ、スチュワード/ガーディアン)。 Каждая команда просто отражает соответствующий эндпоинт `/v1/sns/registrations/{selector}/...` чтобы операторы беты могли репетировать Torii は、SDK に含まれています。

## 4. gRPC の使い方

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
```ワイヤー形式: Norito は、次のようになります。
`fixtures/norito_rpc/schema_hashes.json` (`RegisterNameRequestV1`、
`RegisterNameResponseV1`、`NameRecordV1`、および。 д.)。

## 5. Хуки управления и доказательства

Каждый изменяющий вызов должен приложить доказательства, пригодные для воспроизведения:

| Действие | Требуемые данные управления |
|----------|----------------------------|
| Стандартная регистрация/продление |解決済み、解決済み。 голосование 評議会 не нужно、если tier не требует одобрения 管理人。 |
| Регистрация プレミアム層 / 予約済み割り当て | `GovernanceHookV1` プロポーザル ID + スチュワード承認。 |
| Передача | Хез голосования Council + хел сигнала DAO;ガーディアン・クリアランス、когда передача инициирована разрезением спора。 |
|凍結/凍結解除 |ガーディアンチケットは評議会オーバーライド（凍結解除）を行います。 |

Torii の日付:

1. 提案 ID существует в レジャー управления (`/v1/governance/proposals/{id}`) および статус `Approved`。
2. 問題を解決します。
3. スチュワード/ガーディアンは `SuffixPolicyV1` に属します。

`sns_err_governance_missing` を参照してください。

## 6. Примеры рабочих процессов

### 6.1 Стандартная регистрация

1. `/v1/sns/policies/{suffix_id}` чтобы получить цены、grace и доступные tier。
2. Клиент строит `RegisterNameRequestV1`:
   - `selector` 圧縮 (`sora`) ラベル。
   - `term_years` が表示されます。
   - `payment` スプリッター トレジャリー/スチュワード。
3. Torii の値:
   - Нормализацию метки + 予約済み。
   - 期間/総額対 `PriceTierV1`。
   - Сумма доказательства платежа >= рассчитанной цены + комиссии.
4. Torii:
   - Сохраняет `NameRecordV1`。
   - `RegistryEventV1::NameRegistered`。
   - `RevenueAccrualEventV1`。
   - Возвращает новую запись + события。

### 6.2 猶予を与える

グレースとの出会い:

- Torii と `now` と `grace_expires_at` および追加料金と `SuffixPolicyV1`。
- 追加料金がかかります。 Осибка => `sns_err_payment_mismatch`。
- `RegistryEventV1::NameRenewed` は `expires_at` です。

### 6.3 ガーディアンの凍結と評議会のオーバーライド

1. ガーディアン отправляет `FreezeNameRequestV1` с ticket, ссылающимся на id инцидента.
2. Torii は `NameStatus::Frozen`、`NameFrozen` と一致します。
3. 評議会のオーバーライド。 `/v1/sns/registrations/{selector}/freeze` から `GovernanceHookV1` を削除してください。
4. Torii はオーバーライド、`NameUnfrozen` です。

## 7. Валидация и коды осибок

| Код | Описание | HTTP |
|-----|----------|------|
| `sns_err_reserved` | Метка зарезервирована или заблокирована. | 409 |
| `sns_err_policy_violation` | Срок、tier はコントローラーのレベルを表します。 | 422 |
| `sns_err_payment_mismatch` | Несоответствие значения или актива в доказательстве платежа. | 402 |
| `sns_err_governance_missing` | Отсутствуют/некорректны требуемые артефакты управления。 | 403 |
| `sns_err_state_conflict` | Операция недопустима в текущем состоянии жизненного цикла. | 409 |

`X-Iroha-Error-Code` と Norito JSON/NRPC のファイルが表示されます。

## 8. Примечания по реализации

- Torii 保留中 аукционы в `NameRecordV1.auction` と отклоняет прямые попытки регистрации, пока `PendingAuction`。
- Доказательства платежа переиспользуют Norito 元帳領収書。財務省ヘルパー API (`/v1/finance/sns/payments`)。
- SDK のアップデート、アップデート、アップデート、アップデート、アップデートпричины озибок (`ERR_SNS_RESERVED`, и т. д.)。

## 9. Следующие заги

- SN-3 を使用して Torii を確認します。
- SDK-руководства (Rust/JS/Swift)、API を使用します。
- [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) ガバナンス フックの詳細。