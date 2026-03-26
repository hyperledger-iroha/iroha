---
lang: ru
direction: ltr
source: docs/portal/docs/sns/registry-schema.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Эта страница отражает `docs/source/sns/registry_schema.md` и служителям портала стандартной копии. Исходный файл сохраняется для обновленных переводов.
:::

# Схема реестра Служба имен Sora (SN-2a)

**Статус:** Черновик 24.03.2026 -- отправлено на ревью программы SNS  
**Ссылка на дорожную карту:** SN-2a "Схема реестра и схема хранения"  
**Область:** Определить канонические структуры Norito, состояние жизненного цикла и событий для службы имен Sora (SNS), чтобы реализовать реестр и регистратор, частично определенные в контрактах, SDK и шлюзах.

Этот документ завершает поставку схемы для СН-2а, определяя:

1. Идентификаторы и правила хеширования (`SuffixId`, `NameHash`, селекторы деривации).
2. Norito структуры/перечисления для имен записей, суффиксов политик, ценовых уровней, распределенных доходов и событий реестра.
3. Схема хранения и префиксов индексов для детерминированного воспроизведения.
4. Машину охлаждения, охватывающую регистрацию, продление, благодать/искупление, замораживание и надгробие.
5. Канонические события, потребляемые автоматизация DNS/шлюза.

## 1. Идентификаторы и хеширование

| Идентификатор | Описание | Производная |
|------------|-------------|------------|
| И18НИ00000019X (`u16`) | Идентификатор реестра для суффиксов верхних уровней (`.sora`, `.nexus`, `.dao`). Согласован с каталогом суффиксов в [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Назначается голосованием по управлению; хранилище в `SuffixPolicyV1`. |
| `SuffixSelector` | Каноническая строковая форма суффикса (ASCII, строчные буквы). | Пример: `.sora` -> `sora`. |
| `NameSelectorV1` | Бинарный селектор зарегистрированного лейбла. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Лейбл в NFC + строчные по Norm v1. |
| И18НИ00000031X (`[u8;32]`) | Основной ключевой поиск, заключение контрактов, событий и кешами. | `blake3(NameSelectorV1_bytes)`. |

Требования детерминизма:

- Лейблы нормализуются через Norm v1 (UTS-46 strict, STD3 ASCII, NFC). Пользовательские строки ДОЛЖНЫ быть нормализованы перед хешированием.
- Зарезервированные лейблы (из `SuffixPolicyV1.reserved_labels`) никогда не включаются в реестр; Только управление переопределяет события `ReservedNameAssigned`.

## 2. Структуры Norito

### 2.1 ИмяЗаписиV1| Поле | Тип | Примечания |
|-------|------|-----------|
| `suffix_id` | `u16` | Ссылка на `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Сырые байты селектора для аудита/отладки. |
| `name_hash` | `[u8; 32]` | Ключ для карт/событий. |
| `normalized_label` | `AsciiString` | Читаемый лейбл (после Norm v1). |
| `display_label` | `AsciiString` | Корпус от стюарда; косметика. |
| `owner` | `AccountId` | Управляет продлениями/трансферами. |
| `controllers` | `Vec<NameControllerV1>` | Ссылки на адреса аккаунтов, резолверов или приложений с метаданными. |
| `status` | `NameStatus` | Флаг жизненного цикла (см. Раздел 4). |
| `pricing_class` | `u8` | Индекс ценовых уровней суффикса (стандартный, премиум, зарезервированный). |
| `registered_at` | `Timestamp` | Время блока первой активации. |
| `expires_at` | `Timestamp` | Конец оплаченного срока. |
| `grace_expires_at` | `Timestamp` | Конец льготного автопродления (по умолчанию +30 дней). |
| `redemption_expires_at` | `Timestamp` | Выкуп оконного конца (по умолчанию +60 дней). |
| `auction` | `Option<NameAuctionStateV1>` | Присутствует на возобновленных или премиальных аукционах в Голландии. |
| `last_tx_hash` | `Hash` | Определенный указатель на транзакцию версии. |
| `metadata` | `Metadata` | Произвольная регистратор метаданных (текстовые записи, доказательства). |

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

### 2.2 СуффиксПолисиВ1

| Поле | Тип | Примечания |
|-------|------|-----------|
| `suffix_id` | `u16` | Первичный ключ; стабилен между версиями политики. |
| `suffix` | `AsciiString` | например, `sora`. |
| `steward` | `AccountId` | Управляющий, в настоящее время в уставе управления. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Активировать урегулирование по умолчанию (например `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | Коэффициенты цен по уровням и правилам продолжительности. |
| `min_term_years` | `u8` | Минимальный срок покупки зависит от переопределения уровня. |
| `grace_period_days` | `u16` | По умолчанию 30. |
| `redemption_period_days` | `u16` | По умолчанию 60. |
| `max_term_years` | `u8` | Максимальная продолжительность предоплаты. |
| `referral_cap_bps` | `u16` | <=1000 (10%) по чартеру. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Список управления с уважением. |
| `fee_split` | `SuffixFeeSplitV1` | Доли казны/распорядителя/реферала (базисные баллы). |
| `fund_splitter_account` | `AccountId` | Аккаунт условного депонирования + передача средств. |
| `policy_version` | `u16` | Увеличивается за каждым фасадом. |
| `metadata` | `Metadata` | Расширенные заметки (соглашение KPI, хэши документов по соответствию). |

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

### 2.3 Запись доходов и расчетов| Структура | Поля | Назначение |
|--------|------|------------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Определенная запись распределенных выплат по периодам расчетов (неделя). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Эмитируется при каждом платеже (регистрации, продлении, аукционе). |

Все поля `TokenValue` используют традиционное постоянное кодирование Norito с валютой связанного `SuffixPolicyV1`.

### 2.4 Реестр событий

Канонические события предоставляют журнал повторов для автоматизации и аналитики DNS/шлюза.

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

События должны добавляться в воспроизводимый журнал (например, домен `RegistryEvents`) и зеркалироваться в каналах шлюза, чтобы DNS-кеши отключались в пределах SLA.

## 3. Схема хранения и индексов

| Ключ | Описание |
|-----|----------|
| `Names::<name_hash>` | Основная карта `name_hash` -> `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Вторичный индекс для пользовательского интерфейса кошелька (удобная нумерация страниц). |
| `NamesByLabel::<suffix_id, normalized_label>` | Обнаружение факторов, детерминированный поиск. |
| `SuffixPolicies::<suffix_id>` | Актуальный `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | История `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Журнал только для добавления с монотонной непрерывностью. |

Все ключи сериализуются кортежами Norito для детерминированного хеширования между хостами. Обновления индексов выполняются атомарно вместе с основным рекордом.

## 4. Машина запуска жизненного цикла

| Состояние | Условия входа | Допустимые переходы | Примечания |
|-------|----------------|---------------------|------------|
| Доступно | Производно, когда `NameRecord` отсутствует. | `PendingAuction` (премиум), `Active` (стандартная регистрация). | Поиск доступности читает только индексы. |
| Ожидается аукцион | Создается, когда `PriceTierV1.auction_kind` != none. | `Active` (аукцион рассчитан), `Tombstoned` (нет ставок). | Аукционы эмитируют `AuctionOpened` и `AuctionSettled`. |
| Активный | Регистрация или продление успешно. | И18НИ00000143Х, И18НИ00000144Х, И18НИ00000145Х. | `expires_at` двигает переход. |
| Грейспериод | Автоматически при `now > expires_at`. | `Active` (своевременное продление), `Redemption`, `Tombstoned`. | По умолчанию +30 дней; резолвится, но помечено. |
| Искупление | `now > grace_expires_at` но `< redemption_expires_at`. | `Active` (позднее продление), `Tombstoned`. | Команды требуют штрафного платежа. |
| Замороженный | Заморозка от управления или опекуна. | `Active` (после исправления), `Tombstoned`. | Возможность передачи или обновления контролеров. |
| Надгробие | Добровольная сдача, итог спора или очевидная искупление. | `PendingAuction` (возобновление открытия на голландском языке) или остается надгробием. | Событие `NameTombstoned` должно включать причину. |Переходы изменения ДОЛЖНЫ эмитировать стандартный `RegistryEventKind`, чтобы последующие кэши были без согласования. Захороненные имена, входящие в голландские вновь открытые аукционы, прикрепляют полезную нагрузку `AuctionKind::DutchReopen`.

## 5. Канонические события и шлюз синхронизации

Шлюзы под результат на `RegistryEventV1` и синхронизируют DNS/SoraFS, выполняя:

1. Загрузка последней `NameRecordV1`, которая указывает на последовательность событий.
2. Пегенерация шаблонов резольвера (i105 считается + сжатый (`sora`) как второй выбор, текстовые записи).
3. Закрепите зону обновленных данных через рабочий процесс SoraDNS из [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Гарантии доставки событий:

- каждая транзакция, влияющая на `NameRecordV1`, *должна* добавить ровно одно событие со строго возрастающей `version`.
- События `RevenueSharePosted` ссылаются на расчеты из `RevenueShareRecordV1`.
- События замораживания/размораживания/захоронения включают в себя инструменты управления хеши в `metadata` для воспроизведения аудита.

## 6. Примеры полезных данных Norito

### 6.1 Пример NameRecord

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "soraカタカナ...",
    controllers: [
        NameControllerV1 {
            controller_type: Account,
            account_address: Some(AccountAddress("0x020001...")),
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
    steward: "soraカタカナ...",
    status: Active,
    payment_asset_id: "61CtjvNd9T3THAR65GsMVHr82Bjc",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("soraカタカナ..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "soraカタカナ...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. Следующие шаги

- **SN-2b (API регистратора и перехватчики управления):** откройте эти структуры через Torii (Norito и привязки JSON) и привяжите проверки допуска к артефактам управления.
- **SN-3 (система аукционов и регистрации):** переиспользовать `NameAuctionStateV1` для логики фиксации/раскрытия и повторного открытия голландского языка.
- **SN-5 (Платежи и расчеты):** используйте `RevenueShareRecordV1` для финансовых сверок и обработки отчетов.

Вопросы и пожелания по изменениям следует фиксировать вместе с обновлениями дорожной карты SNS в `roadmap.md` и отражать в `status.md` при слиянии.