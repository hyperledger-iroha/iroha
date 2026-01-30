---
lang: fr
direction: ltr
source: docs/portal/docs/sns/registry-schema.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Канонический источник
Эта страница отражает `docs/source/sns/registry_schema.md` и служит канонической копией портала. Исходный файл сохраняется для обновлений переводов.
:::

# Схема реестра Sora Name Service (SN-2a)

**Статус:** Черновик 2026-03-24 -- отправлено на ревью программы SNS  
**Ссылка на roadmap:** SN-2a "Registry schema & storage layout"  
**Область:** Определить канонические структуры Norito, состояния жизненного цикла и события для Sora Name Service (SNS), чтобы реализации registry и registrar оставались детерминированными в контрактах, SDK и gateways.

Этот документ завершает поставку схемы для SN-2a, определяя:

1. Идентификаторы и правила хеширования (`SuffixId`, `NameHash`, derivation селекторов).
2. Norito structs/enums для записей имен, политик суффиксов, ценовых tiers, распределений доходов и событий реестра.
3. Layout хранения и префиксы индексов для детерминированного replay.
4. Машину состояний, охватывающую регистрацию, продление, grace/redemption, freeze и tombstone.
5. Канонические события, потребляемые DNS/gateway автоматизацией.

## 1. Идентификаторы и хеширование

| Идентификатор | Описание | Производная |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Идентификатор реестра для суффиксов верхнего уровня (`.sora`, `.nexus`, `.dao`). Согласован с каталогом суффиксов в [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Назначается голосованием по governance; хранится в `SuffixPolicyV1`. |
| `SuffixSelector` | Каноническая строковая форма суффикса (ASCII, lower-case). | Пример: `.sora` -> `sora`. |
| `NameSelectorV1` | Бинарный селектор зарегистрированного лейбла. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Лейбл в NFC + lower-case по Norm v1. |
| `NameHash` (`[u8;32]`) | Основной ключ поиска, используемый контрактами, событиями и кешами. | `blake3(NameSelectorV1_bytes)`. |

Требования детерминизма:

- Лейблы нормализуются через Norm v1 (UTS-46 strict, STD3 ASCII, NFC). Пользовательские строки ДОЛЖНЫ быть нормализованы перед хешированием.
- Зарезервированные лейблы (из `SuffixPolicyV1.reserved_labels`) никогда не входят в реестр; governance-only overrides выпускают события `ReservedNameAssigned`.

## 2. Структуры Norito

### 2.1 NameRecordV1

| Поле | Тип | Примечания |
|-------|------|-----------|
| `suffix_id` | `u16` | Ссылка на `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Сырые байты селектора для аудита/debug. |
| `name_hash` | `[u8; 32]` | Ключ для карт/событий. |
| `normalized_label` | `AsciiString` | Читаемый лейбл (после Norm v1). |
| `display_label` | `AsciiString` | Casing от steward; косметика. |
| `owner` | `AccountId` | Управляет продлениями/трансферами. |
| `controllers` | `Vec<NameControllerV1>` | Ссылки на адреса аккаунтов, resolvers или metadata приложения. |
| `status` | `NameStatus` | Флаг жизненного цикла (см. Раздел 4). |
| `pricing_class` | `u8` | Индекс в ценовых tiers суффикса (standard, premium, reserved). |
| `registered_at` | `Timestamp` | Время блока первичной активации. |
| `expires_at` | `Timestamp` | Конец оплаченного срока. |
| `grace_expires_at` | `Timestamp` | Конец grace auto-renew (default +30 дней). |
| `redemption_expires_at` | `Timestamp` | Конец окна redemption (default +60 дней). |
| `auction` | `Option<NameAuctionStateV1>` | Присутствует при Dutch reopen или premium аукционах. |
| `last_tx_hash` | `Hash` | Детерминированный указатель на транзакцию версии. |
| `metadata` | `Metadata` | Произвольная metadata registrar (text records, proofs). |

Поддерживающие структуры:

```text
Enum NameStatus {
    Available,          // derived, not stored on-ledger
    PendingAuction,
    Active,
    GracePeriod,
    Redemption,
    Frozen(NameFrozenStateV1),
    Tombstoned(NameTombstoneStateV1)
}

Struct NameFrozenStateV1 {
    reason: String,
    until_ms: u64,
}

Struct NameTombstoneStateV1 {
    reason: String,
}

Struct NameControllerV1 {
    controller_type: ControllerType,   // Account, ResolverTemplate, ExternalLink
    account_address: Option<AccountAddress>,   // Serialized as canonical `0x...` hex in JSON
    resolver_template_id: Option<String>,
    payload: Metadata,                 // Extra selector/value pairs for wallets/gateways
}

Struct TokenValue {
    asset_id: AsciiString,
    amount: u128,
}

Enum ControllerType {
    Account,
    Multisig,
    ResolverTemplate,
    ExternalLink
}

Struct NameAuctionStateV1 {
    kind: AuctionKind,             // Vickrey, DutchReopen
    opened_at_ms: u64,
    closes_at_ms: u64,
    floor_price: TokenValue,
    highest_commitment: Option<Hash>,  // reference to sealed bid
    settlement_tx: Option<Json>,
}

Enum AuctionKind {
    VickreyCommitReveal,
    DutchReopen
}
```

### 2.2 SuffixPolicyV1

| Поле | Тип | Примечания |
|-------|------|-----------|
| `suffix_id` | `u16` | Первичный ключ; стабилен между версиями политики. |
| `suffix` | `AsciiString` | например, `sora`. |
| `steward` | `AccountId` | Steward, определенный в governance charter. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Актив settlement по умолчанию (например `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | Коэффициенты цен по tiers и правила длительности. |
| `min_term_years` | `u8` | Минимальный срок покупки вне зависимости от tier overrides. |
| `grace_period_days` | `u16` | Default 30. |
| `redemption_period_days` | `u16` | Default 60. |
| `max_term_years` | `u8` | Максимальная длительность предоплаты. |
| `referral_cap_bps` | `u16` | <=1000 (10%) по charter. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Список от governance с инструкциями назначения. |
| `fee_split` | `SuffixFeeSplitV1` | Доли treasury / steward / referral (basis points). |
| `fund_splitter_account` | `AccountId` | Аккаунт escrow + распределение средств. |
| `policy_version` | `u16` | Увеличивается при каждом изменении. |
| `metadata` | `Metadata` | Расширенные заметки (KPI covenant, hashes docs по compliance). |

```text
Struct PriceTierV1 {
    tier_id: u8,
    label_regex: String,       // RE2-syntax pattern describing eligible labels
    base_price: TokenValue,    // Price per one-year term before suffix coefficient
    auction_kind: AuctionKind, // Default auction when the tier triggers
    dutch_floor: Option<TokenValue>,
    min_duration_years: u8,
    max_duration_years: u8,
}

Struct ReservedNameV1 {
    normalized_label: AsciiString,
    assigned_to: Option<AccountId>,
    release_at_ms: Option<u64>,
    note: String,
}

Struct SuffixFeeSplitV1 {
    treasury_bps: u16,     // default 7000 (70%)
    steward_bps: u16,      // default 3000 (30%)
    referral_max_bps: u16, // optional referral carve-out (<= 1000)
    escrow_bps: u16,       // % routed to claw-back escrow
}
```

### 2.3 Записи доходов и settlement

| Struct | Поля | Назначение |
|--------|------|------------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Детерминированная запись распределенных выплат по эпохам settlement (неделя). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Эмитируется при каждом платеже (registration, renewal, auction). |

Все поля `TokenValue` используют каноническое фиксированное кодирование Norito с кодом валюты из связанного `SuffixPolicyV1`.

### 2.4 События реестра

Канонические события дают replay log для DNS/gateway автоматизации и аналитики.

```text
Struct RegistryEventV1 {
    name_hash: [u8; 32],
    suffix_id: u16,
    selector: NameSelectorV1,
    version: u64,               // increments per NameRecord update
    timestamp: Timestamp,
    tx_hash: Hash,
    actor: AccountId,
    event: RegistryEventKind,
}

Enum RegistryEventKind {
    NameRegistered { expires_at: Timestamp, pricing_class: u8 },
    NameRenewed { expires_at: Timestamp, term_years: u8 },
    NameTransferred { previous_owner: AccountId, new_owner: AccountId },
    NameControllersUpdated { controller_count: u16 },
    NameFrozen(NameFrozenStateV1),
    NameUnfrozen,
    NameTombstoned(NameTombstoneStateV1),
    AuctionOpened { kind: AuctionKind },
    AuctionSettled { winning_account: AccountId, clearing_price: TokenValue },
    RevenueSharePosted { epoch_id: u64, treasury_amount: TokenValue, steward_amount: TokenValue },
    SuffixPolicyUpdated { policy_version: u16 },
}
```

События должны добавляться в replayable log (например, домен `RegistryEvents`) и зеркалироваться в gateway feeds, чтобы DNS caches инвалидировались в пределах SLA.

## 3. Layout хранения и индексы

| Ключ | Описание |
|-----|----------|
| `Names::<name_hash>` | Основная карта `name_hash` -> `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Вторичный индекс для wallet UI (pagination friendly). |
| `NamesByLabel::<suffix_id, normalized_label>` | Обнаружение конфликтов, детерминированный поиск. |
| `SuffixPolicies::<suffix_id>` | Актуальный `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | История `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Append-only log с монотонной последовательностью. |

Все ключи сериализуются Norito tuples для детерминированного хеширования между хостами. Обновления индексов выполняются атомарно вместе с основным рекордом.

## 4. Машина состояний жизненного цикла

| Состояние | Условия входа | Допустимые переходы | Примечания |
|-------|----------------|---------------------|------------|
| Available | Производно, когда `NameRecord` отсутствует. | `PendingAuction` (premium), `Active` (standard registration). | Поиск доступности читает только индексы. |
| PendingAuction | Создается, когда `PriceTierV1.auction_kind` != none. | `Active` (auction settles), `Tombstoned` (no bids). | Аукционы эмитируют `AuctionOpened` и `AuctionSettled`. |
| Active | Регистрация или продление успешно. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` двигает переход. |
| GracePeriod | Автоматически при `now > expires_at`. | `Active` (on-time renewal), `Redemption`, `Tombstoned`. | Default +30 дней; резолвится, но помечено. |
| Redemption | `now > grace_expires_at` но `< redemption_expires_at`. | `Active` (late renewal), `Tombstoned`. | Команды требуют штрафного платежа. |
| Frozen | Freeze от governance или guardian. | `Active` (после ремедиации), `Tombstoned`. | Нельзя передавать или обновлять controllers. |
| Tombstoned | Добровольная сдача, итог спора или истекшая redemption. | `PendingAuction` (Dutch reopen) или остается tombstoned. | Событие `NameTombstoned` должно включать причину. |

Переходы состояний ДОЛЖНЫ эмитировать соответствующий `RegistryEventKind`, чтобы downstream caches оставались согласованными. Tombstoned имена, входящие в Dutch reopen аукционы, прикрепляют payload `AuctionKind::DutchReopen`.

## 5. Канонические события и синхронизация gateway

Gateways подписываются на `RegistryEventV1` и синхронизируют DNS/SoraFS, выполняя:

1. Загрузка последнего `NameRecordV1`, на который указывает последовательность событий.
2. Перегенерация resolver templates (IH58 предпочтительно + compressed (`sora`) как второй выбор, text records).
3. Pin обновленных данных зоны через SoraDNS workflow из [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Гарантии доставки событий:

- Каждая транзакция, влияющая на `NameRecordV1`, *должна* добавить ровно одно событие со строго возрастающей `version`.
- `RevenueSharePosted` события ссылаются на settlements из `RevenueShareRecordV1`.
- События freeze/unfreeze/tombstone включают хеши governance артефактов в `metadata` для audit replay.

## 6. Примеры payloads Norito

### 6.1 Пример NameRecord

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "ih58...",
    controllers: [
        NameControllerV1 {
            controller_type: Account,
            account_address: Some(AccountAddress("0x02000001...")),
            resolver_template_id: None,
            payload: {}
        }
    ],
    status: Active,
    pricing_class: 0,
    registered_at: 1_776_000_000,
    expires_at: 1_807_296_000,
    grace_expires_at: 1_809_888_000,
    redemption_expires_at: 1_815_072_000,
    auction: None,
    last_tx_hash: 0xa3d4...c001,
    metadata: { "resolver": "wallet.default", "notes": "SNS beta cohort" },
}
```

### 6.2 Пример SuffixPolicy

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "ih58...",
    status: Active,
    payment_asset_id: "xor#sora",
    pricing: [
        PriceTierV1 { tier_id:0, label_regex:"^[a-z0-9]{3,}$", base_price:"120 XOR", auction_kind:VickreyCommitReveal, dutch_floor:None, min_duration_years:1, max_duration_years:5 },
        PriceTierV1 { tier_id:1, label_regex:"^[a-z]{1,2}$", base_price:"10_000 XOR", auction_kind:DutchReopen, dutch_floor:Some("1_000 XOR"), min_duration_years:1, max_duration_years:3 }
    ],
    min_term_years: 1,
    grace_period_days: 30,
    redemption_period_days: 60,
    max_term_years: 5,
    referral_cap_bps: 500,
    reserved_labels: [
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("ih58..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "ih58...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. Следующие шаги

- **SN-2b (Registrar API & governance hooks):** открыть эти structs через Torii (Norito и JSON bindings) и привязать admission checks к governance артефактам.
- **SN-3 (Auction & registration engine):** переиспользовать `NameAuctionStateV1` для логики commit/reveal и Dutch reopen.
- **SN-5 (Payment & settlement):** использовать `RevenueShareRecordV1` для финансовой сверки и автоматизации отчетов.

Вопросы и запросы на изменения следует фиксировать вместе с обновлениями SNS roadmap в `roadmap.md` и отражать в `status.md` при слиянии.
