---
lang: ru
direction: ltr
source: docs/portal/docs/sns/registry-schema.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::обратите внимание на Фуэнте каноника
Эта страница отображает `docs/source/sns/registry_schema.md` и теперь является канонической копией портала. Архив можно использовать для актуализации перевода.
:::

# Регистрационный номер службы имен Сора (SN-2a)

**Эстадо:** Отредактировано 24 марта 2026 г. – отправлена редакция программы SNS.  
**Дополнительная карта:** SN-2a «Схема реестра и схема хранения».  
**Alcance:** Определите канонические структуры Norito, статусы циклической жизни и события, излучаемые для службы имен Sora (SNS), в том виде, в котором реализации регистрации и регистратора определяются в договорах, SDK и шлюзах.

Это полный документ, содержащий описание задачи для SN-2a, в частности:

1. Идентификаторы и правила хеширования (`SuffixId`, `NameHash`, производные селекторов).
2. Структуры/перечисления Norito для регистров номеров, политики суфиев, уровней драгоценностей, отчетов по входам и событий реестра.
3. Макет хранилища и префиксы индексов для определенного воспроизведения.
4. Una maquina de estados que cubre registro, renovacion, gracia/redencion, замораживание и надгробия.
5. Канонические события используются для автоматизации DNS/шлюза.

## 1. Идентификаторы и хеширование

| Идентификатор | Описание | Вывод |
|------------|-------------|------------|
| И18НИ00000019X (`u16`) | Идентификатор реестра для более высокого уровня (`.sora`, `.nexus`, `.dao`). Подключите каталог суфиев в [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Назначено голосование за губернаторство; almacenado в `SuffixPolicyV1`. |
| `SuffixSelector` | Каноническая форма в строке суфийо (ASCII, строчные буквы). | Например: `.sora` -> `sora`. |
| `NameSelectorV1` | Селектор бинарного файла для регистрации по этикету. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Этикетка — это NFC + строчные буквы Norm v1. |
| И18НИ00000031X (`[u8;32]`) | Clave Primaria de Busqueda используется для контрактов, событий и кэшей. | `blake3(NameSelectorV1_bytes)`. |

Реквизиты детерминизма:

- Этикетки нормализуются с помощью Norm v1 (строгий UTS-46, STD3 ASCII, NFC). Каденаты пользователя DEBEN нормализуются до хэша.
- Las etiquetas reservadas (de `SuffixPolicyV1.reserved_labels`) не внесен в реестр; los overrides Solo de Gobernanza Emiten Eventos `ReservedNameAssigned`.

## 2. Структура Norito

### 2.1 ИмяЗаписиV1| Кампо | Типо | Заметки |
|-------|------|-------|
| `suffix_id` | `u16` | Ссылка `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Байты селектора для процесса аудита/отладки. |
| `name_hash` | `[u8; 32]` | Клавиша для карт/событий. |
| `normalized_label` | `AsciiString` | Этикет разборчивый для людей (пост Норма v1). |
| `display_label` | `AsciiString` | Корпус провисто для стюарда; косметика по желанию. |
| `owner` | `AccountId` | Контроль обновлений/переносов. |
| `controllers` | `Vec<NameControllerV1>` | Ссылки на направления, связанные с объектом, преобразователи или метаданные приложения. |
| `status` | `NameStatus` | Bandera de ciclo de vida (версия, раздел 4). |
| `pricing_class` | `u8` | Индекс цен на товары (стандартный, премиум, зарезервированный). |
| `registered_at` | `Timestamp` | Временная метка блокировки начальной активации. |
| `expires_at` | `Timestamp` | Конец конечной остановки. |
| `grace_expires_at` | `Timestamp` | Fin de gracia de auto-renovacion (по умолчанию +30 дней). |
| `redemption_expires_at` | `Timestamp` | Fin de ventana de redencion (по умолчанию +60 дней). |
| `auction` | `Option<NameAuctionStateV1>` | Представляем вам, когда вы обретете голландский или субастас премиум-класс, который активируется. |
| `last_tx_hash` | `Hash` | Выбор определяется транзакцией, производящей эту версию. |
| `metadata` | `Metadata` | Метаданные произвольного регистратора (текстовые записи, доказательства). |

Структуры совместимости:

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

| Кампо | Типо | Заметки |
|-------|------|-------|
| `suffix_id` | `u16` | Клаве примария; существуют между политическими версиями. |
| `suffix` | `AsciiString` | Например, `sora`. |
| `steward` | `AccountId` | Стюард определил хартию губернатора. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Идентификатор активности урегулирования по дефекту (например, `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | Коэффициенты драгоценных камней по уровням и правилам долговечности. |
| `min_term_years` | `u8` | Это означает, что термин импорта не имеет приоритета над уровнем. |
| `grace_period_days` | `u16` | По умолчанию 30. |
| `redemption_period_days` | `u16` | По умолчанию 60. |
| `max_term_years` | `u8` | Maximo de renovacion por adelantado. |
| `referral_cap_bps` | `u16` | <=1000 (10%) второго чартера. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Список высшего руководства с инструкциями по назначению. |
| `fee_split` | `SuffixFeeSplitV1` | Porciones de tesoreria / стюард / направление (базисные баллы). |
| `fund_splitter_account` | `AccountId` | Когда вы сохраняете условное депонирование + распределяете фонды. |
| `policy_version` | `u16` | Incrementa en cada cambio. |
| `metadata` | `Metadata` | Расширенные уведомления (соглашение KPI, хеши документов о накоплениях). |

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

### 2.3 Регистры проникновения и урегулирования| Структура | Кампос | Предложение |
|--------|--------|---------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Определенный регистр поселений на период поселения (семанальный). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Emitido cada vez que un pago se registra (регистрация, обновление, субаста). |

Все кампусы `TokenValue` используют каноническую кодификацию Norito с денежным кодом, объявленным в ассоциированном `SuffixPolicyV1`.

### 2.4 События регистрации

Канонические события подтверждены журналом повторов для автоматизации DNS/шлюза и аналитики.

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

События должны быть объединены в воспроизводимый журнал (например, домен `RegistryEvents`) и отражены в каналах шлюза, чтобы кэши DNS были недействительны в соответствии с SLA.

## 3. Макет хранилища и индексов

| клава | Описание |
|-----|-------------|
| `Names::<name_hash>` | Первая карта `name_hash` и `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Второй индекс пользовательского интерфейса кошелька (любимая страница). |
| `NamesByLabel::<suffix_id, normalized_label>` | Обнаружение конфликтов, умение действовать определенным образом. |
| `SuffixPolicies::<suffix_id>` | Ультимо `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | История `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Журнал доступен только для добавления с помощью clave de secuencia monotonica. |

Все клавиши будут сериализованы с использованием номеров Norito для управления хешированием, определенным между хостами. Актуализация индексов происходит в форме атомного объединения с первичным регистром.

## 4. Машина для циклической жизни

| Эстадо | Условия входа | Разрешение на переходы | Заметки |
|-------|------------------------|-------------------------|-------|
| Доступно | Произведено, когда `NameRecord` уже давно. | `PendingAuction` (премиум), `Active` (регистрационный стандарт). | La busqueda de disponibilidad lee соло индексы. |
| Ожидается аукцион | Creado cuando `PriceTierV1.auction_kind` != нет. | `Active` (la subasta se Liquida), `Tombstoned` (без пуджи). | Las subastas ementien `AuctionOpened` y `AuctionSettled`. |
| Активный | Регистрация или выход на ремонт. | И18НИ00000143Х, И18НИ00000144Х, И18НИ00000145Х. | `expires_at` импульс перехода. |
| Грейспериод | Автоматически cuando `now > expires_at`. | `Active` (ремонт на время), `Redemption`, `Tombstoned`. | По умолчанию +30 диаметров; Aun Resuelve Pero Marcado. |
| Искупление | `now > grace_expires_at` или `< redemption_expires_at`. | `Active` (позднее обновление), `Tombstoned`. | Командиры требуют штрафа. |
| Замороженный | Заморозить губернатора или опекуна. | `Active` (исправление), `Tombstoned`. | Невозможно передать или актуализировать контроллеры. |
| Надгробие | Добровольное освобождение, результат постоянного спора или истечение срока действия. | `PendingAuction` (возобновление открытия на голландском языке) или навсегда надгробие. | В событие `NameTombstoned` необходимо включить разум. |Переходы из состояния DEBEN излучают корреспондента `RegistryEventKind`, чтобы кэши ниже по течению были согласованы. Los nombres tombstones que que entran en subastas Dutch, вновь открыл адъюнкт к полезной нагрузке `AuctionKind::DutchReopen`.

## 5. Канонические события и синхронизация шлюзов

Шлюзы подключаются к `RegistryEventV1` и синхронизируются через DNS/SoraFS:

1. Получите последнюю ссылку `NameRecordV1` для отслеживания событий.
2. Обновите шаблоны преобразователя (предпочтительные направления i105 + сжатый (`sora`) как второй вариант, текстовые записи).
3. Укажите актуализированные данные зоны с помощью описания канала SoraDNS в [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Гарантии проведения мероприятий:

- Каждая транзакция, которая влияет на `NameRecordV1` *должна* быть точно объединена с событием с ограниченным доступом к `version`.
- События `RevenueSharePosted` ссылаются на ликвидацию, выданную `RevenueShareRecordV1`.
- События замораживания/размораживания/надгробия включают хэши артефактов правительства от `metadata` для воспроизведения в аудитории.

## 6. Примеры полезных нагрузок Norito

### 6.1 Пример записи имени

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "<i105-account-id>",
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

### 6.2 Пример суффиксной политики

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "<i105-account-id>",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("<i105-account-id>"), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "<i105-account-id>",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. Проксимос Пасос

- **SN-2b (API регистратора и перехватчики управления):** экспонер использует структуры через Torii (привязки Norito и JSON) и при подключении к входу проверяет артефакты управления.
- **SN-3 (система аукционов и регистрации):** повторно используйте `NameAuctionStateV1` для реализации логики фиксации/раскрытия и повторного открытия голландского языка.
- **SN-5 (Платежи и расчеты):** подтверждение `RevenueShareRecordV1` для финансовой выверки и автоматизации отчетов.

Вопросы о необходимости регистрации вместе с актуализацией дорожной карты SNS в `roadmap.md` и отражения в `status.md`, когда они интегрированы.