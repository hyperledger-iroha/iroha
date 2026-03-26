---
lang: ru
direction: ltr
source: docs/portal/docs/sns/registry-schema.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание Fonte canonica
Эта страница написана `docs/source/sns/registry_schema.md` и агора служит как каноническая копия портала. Постоянный архив для перевода.
:::

# Регистрационный номер службы имен Сора (SN-2a)

**Статус:** Redigido от 24 марта 2026 г. – субметка для пересмотра программы SNS  
**Ссылка на дорожную карту:** SN-2a «Схема реестра и схема хранения».  
**Обзор:** Определите канонические версии Norito, статусы циклической жизни и события, излучаемые для службы имен Sora (SNS), которые используются в качестве реализаций регистрации и регистратора, определенных в договорах, SDK и шлюзов.

Этот полный документ или описание проекта для SN-2a или конкретно:

1. Идентификаторы и проверки хеширования (`SuffixId`, `NameHash`, производные выборки).
2. Структуры/перечисления Norito для регистров имен, политики суфиксов, предварительных уровней, повторных операций и событий регистрации.
3. Макет вооружения и префиксов индексов для детерминированного воспроизведения.
4. Uma maquina de estados cobrindo registro, renovacao, благодать/искупление, замораживание и надгробия.
5. Канонические события потребляют автоматический DNS/шлюз.

## 1. Идентификаторы и хеширование

| Идентификатор | Описание | Деривакао |
|------------|-------------|------------|
| И18НИ00000019X (`u16`) | Идентификатор регистрации для суффиксов высшего уровня (`.sora`, `.nexus`, `.dao`). Alinhado входит в каталог суфиксов в [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Atribuido por voto degovanca; вооружен `SuffixPolicyV1`. |
| `SuffixSelector` | Каноническая форма em string do sufixo (ASCII, строчные буквы). | Пример: `.sora` -> `sora`. |
| `NameSelectorV1` | Выбор двоичного файла для повторной регистрации. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Поворотный и NFC + вторая строчная буква Norm v1. |
| И18НИ00000031X (`[u8;32]`) | Сначала используйте контракты, события и кэши. | `blake3(NameSelectorV1_bytes)`. |

Реквизиты детерминизма:

- Поворотные устройства нормализуются с помощью Norm v1 (строгий UTS-46, STD3 ASCII, NFC). Поскольку строки пользователя DEVEM нормализуются до хэширования.
- Зарезервировано (de `SuffixPolicyV1.reserved_labels`) без записи в реестр; переопределяет возможности управления событиями `ReservedNameAssigned`.

## 2. Эструтурас Norito

### 2.1 ИмяЗаписиV1| Кампо | Типо | Заметки |
|-------|------|-------|
| `suffix_id` | `u16` | Ссылка `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Байты делают селектор бруто для аудитории/отладки. |
| `name_hash` | `[u8; 32]` | Используйте карты/мероприятия. |
| `normalized_label` | `AsciiString` | Rotulo legivel por humanos (после нормы v1). |
| `display_label` | `AsciiString` | Кассовый форнесидо пело стюард; косметика по желанию. |
| `owner` | `AccountId` | Controla renovacoes/transferencias. |
| `controllers` | `Vec<NameControllerV1>` | Ссылки на исходные данные, преобразователи или метаданные приложения. |
| `status` | `NameStatus` | Индикатор циклической жизни (версия 4). |
| `pricing_class` | `u8` | Indice nos tiers de preco do sufixo (стандартный, премиум, зарезервированный). |
| `registered_at` | `Timestamp` | Временная метка начальной блокировки. |
| `expires_at` | `Timestamp` | Fim do termo pago. |
| `grace_expires_at` | `Timestamp` | Fim da Grace de Auto-Renovacao (по умолчанию +30 дней). |
| `redemption_expires_at` | `Timestamp` | Fim da janela de искупления (по умолчанию +60 дней). |
| `auction` | `Option<NameAuctionStateV1>` | Представляем вам, когда голландцы вновь откроют или премиальные активы. |
| `last_tx_hash` | `Hash` | Понтейро определился с транзакцией, которую вы сделаете наоборот. |
| `metadata` | `Metadata` | Метаданные произвольного регистратора (текстовые записи, доказательства). |

Структуры поддержки:

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
| `suffix_id` | `u16` | Чаве первоцветная; estavel entre versoes de politica. |
| `suffix` | `AsciiString` | Например, `sora`. |
| `steward` | `AccountId` | Стюард не определил никакой хартии управления. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Идентификатор исходного поселения по адресу (пример `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | Коэффициенты предварительной подготовки и проверки в течение длительного времени. |
| `min_term_years` | `u8` | Это для независимого режима переопределения уровня. |
| `grace_period_days` | `u16` | По умолчанию 30. |
| `redemption_period_days` | `u16` | По умолчанию 60. |
| `max_term_years` | `u8` | Максимо де реновакао антеципада. |
| `referral_cap_bps` | `u16` | <= 1000 (10%) второго или чартерного рейса. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Список правил управления с инструкциями по выплате. |
| `fee_split` | `SuffixFeeSplitV1` | Partes tesouraria / стюард / направление (базисные баллы). |
| `fund_splitter_account` | `AccountId` | Сообщите о временном депонировании + распределении средств. |
| `policy_version` | `u16` | Incrementa em Cada Mudanca. |
| `metadata` | `Metadata` | Notas estendidas (соглашение KPI, хеши документов соответствия). |

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

### 2.3 Регистры приходов и расчетов| Структура | Кампос | Предложение |
|--------|--------|-----------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Реестр детерминистических платежей за период поселения (семанальный). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Emitido cada vez que um pagamento e postado (регистрация, обновление, регистрация). |

Все кампусы `TokenValue` используют каноническую кодификацию Norito с кодом моего объявления, связанного с `SuffixPolicyV1`.

### 2.4 События регистрации

Канонические события позволяют вести журнал повторов для автоматического DNS/шлюза и аналитики.

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

Наши события должны быть добавлены к воспроизведению журнала (например, к домену `RegistryEvents`) и отражены в каналах шлюза, чтобы кэши DNS были недействительны в соответствии с SLA.

## 3. Макет вооружения и индексов

| Чаве | Описание |
|-----|-------------|
| `Names::<name_hash>` | Первоначальная карта `name_hash` для `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Второй индекс пользовательского интерфейса кошелька (любая страница). |
| `NamesByLabel::<suffix_id, normalized_label>` | Detecta conflitos, habilita busca deterministica. |
| `SuffixPolicies::<suffix_id>` | Ультимо `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Исторический `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Журнал доступен только для добавления в монотонную последовательность. |

Все это необходимо для сериализации с использованием ввода Norito для детерминированного хеширования между хостами. Атуализированный индекс исправления формы атомного объединения с помощью первичного регистра.

## 4. Машина для циклической жизни

| Эстадо | Входные билеты | Разрешения на транзит | Заметки |
|-------|----------------------|----------------------|-------|
| Доступно | Произведено, когда `NameRecord` уже давно. | `PendingAuction` (премиум), `Active` (стандарт регистрации). | Индексы отсутствия доступа к аппенам. |
| Ожидается аукцион | Criado quando `PriceTierV1.auction_kind` != нет. | `Active` (жидкая жидкость), `Tombstoned` (sem-копья). | Лейлос выпускает `AuctionOpened` и `AuctionSettled`. |
| Активный | Регистрация или обновление должны быть выполнены. | И18НИ00000143Х, И18НИ00000144Х, И18НИ00000145Х. | `expires_at` помог с переходом. |
| Грейспериод | Автоматически когда `now > expires_at`. | `Active` (обновление диаметром), `Redemption`, `Tombstoned`. | По умолчанию +30 диаметров; ainda разрешит Мас Sinalizado. |
| Искупление | `now > grace_expires_at` mas `< redemption_expires_at`. | `Active` (позднее обновление), `Tombstoned`. | Comandos exigem Taxa de Penalidade. |
| Замороженный | Заморозить правительство или опекуна. | `Active` (после исправления), `Tombstoned`. | Невозможно передать контроллеры для настройки. |
| Надгробие | Добровольный отказ, результат постоянного спора или срок искупления. | `PendingAuction` (возобновление открытия в Нидерландах) или навсегда надгробие. | На событие `NameTombstoned` пришлось включить разао. |При переходе из состояния DEVEM эмитира или `RegistryEventKind` соответствующий параметр кэширует нисходящие потоки. Номы надгробны, когда голландцы снова открывают экзамен с полезной нагрузкой `AuctionKind::DutchReopen`.

## 5. Канонические события и синхронизация шлюзов

Шлюзы с кодом `RegistryEventV1` и синхронизацией DNS/SoraFS или:

1. Введите или укажите последнюю ссылку `NameRecordV1` на последовательность событий.
2. Обновите шаблоны преобразователя (предпочтительные файлы i105 + сжатые (`sora`) в качестве второй операции, текстовые записи).
3. Установите ближайшие настройки зоны с помощью описания SoraDNS в [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Гарантии проведения мероприятий:

- Cada transacao que afeta um `NameRecordV1` *deve* anexar exatamente um evento com `version` estritamente Crescente.
- События `RevenueSharePosted` ссылаются на жидкости, излучаемые `RevenueShareRecordV1`.
- События замораживания/размораживания/надгробия включают хэши артефактов управления от `metadata` для воспроизведения в аудитории.

## 6. Примеры полезных нагрузок Norito

### 6.1 Пример записи имени

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

### 6.2 Пример суффиксной политики

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

## 7. Проксимос пассос

- **SN-2b (API регистратора и перехватчики управления):** экспортируйте эти структуры через Torii (привязки Norito и JSON) и проверяйте допуск и артефакты управления.
- **SN-3 (система аукционов и регистрации):** повторно используйте `NameAuctionStateV1` для реализации логики фиксации/раскрытия и повторного открытия голландского языка.
- **SN-5 (Платежи и расчеты):** подтвердите `RevenueShareRecordV1` для выверки финансов и автоматического расчета отношений.

Мы можем предложить или запросить информацию о совместной регистрации в качестве первоначальной дорожной карты SNS в `roadmap.md` и refletidas в `status.md`, когда они интегрированы.