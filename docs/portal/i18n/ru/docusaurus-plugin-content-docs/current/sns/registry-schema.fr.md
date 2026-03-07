---
lang: ru
direction: ltr
source: docs/portal/docs/sns/registry-schema.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Источник канонический
Эта страница отображает `docs/source/sns/registry_schema.md` и является дезормированной канонической копией порта. Le fichier source reste pour les mises a jour de traduction.
:::

# Схема регистрации службы имен Sora (SN-2a)

**Статут:** Redige от 24 марта 2026 г. – это обзор программы SNS.  
**План залогового права:** SN-2a «Схема реестра и схема хранения»  
**Portee:** Определите канонические структуры Norito, этапы цикла жизни и события, эмис для службы имен Sora (SNS) в зависимости от реализаций регистрации и регистратора, остающихся детерминированными в договорах, SDK и шлюзах.

В этом документе подробно описана схема для SN-2a:

1. Идентификаторы и правила хеширования (`SuffixId`, `NameHash`, производные селекторов).
2. Структуры/перечисления Norito для регистрации имен, политики суффиксов, уровней призов, перераспределения доходов и событий регистрации.
3. Макет хранилища и префиксы индексов для определенного воспроизведения.
4. Государственная машина осуществляет регистрацию, обновление, благодать/искупление, замораживание и надгробия.
5. Канонические события, связанные с автоматизацией DNS/шлюза.

## 1. Идентификаторы и хеширование

| Идентификатор | Описание | Вывод |
|------------|-------------|------------|
| И18НИ00000019X (`u16`) | Идентификатор регистрации суффиксов первого уровня (`.sora`, `.nexus`, `.dao`). Выровняйте каталог суффиксов в [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Атрибут номинального голосования по управлению; Stocke dans `SuffixPolicyV1`. |
| `SuffixSelector` | Каноническая форма в цепочке суффиксов (ASCII, строчные буквы). | Пример: `.sora` -> `sora`. |
| `NameSelectorV1` | Двоичный выбор для регистрации метки. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Метка есть NFC + строчные буквы Norm v1. |
| И18НИ00000031X (`[u8;32]`) | Cle primaire de recherche используется по контрактам, событиям и кэшам. | `blake3(NameSelectorV1_bytes)`. |

Требования детерминизма:

- Метки нормализуются через Norm v1 (UTS-46 strict, STD3 ASCII, NFC). Цепи, использующие DOIVENT, и нормализуются перед хешем.
- Резервные метки (de `SuffixPolicyV1.reserved_labels`) не входят в регистр; les переопределяет уникальное управление событиями `ReservedNameAssigned`.

##2. Структуры Norito

### 2.1 ИмяЗаписиV1| Чемпион | Тип | Заметки |
|-------|------|-------|
| `suffix_id` | `u16` | Ссылка `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Octets du selecteur brut для аудита/отладки. |
| `name_hash` | `[u8; 32]` | Кле залить карты/события. |
| `normalized_label` | `AsciiString` | Этикетка lisible pour l'humain (сообщение Norm v1). |
| `display_label` | `AsciiString` | Корпус фурни для стюарда; косметический вариант. |
| `owner` | `AccountId` | Контролируйте обновления/переезды. |
| `controllers` | `Vec<NameControllerV1>` | Ссылки на адреса компьютеров, преобразователи или метаданные приложения. |
| `status` | `NameStatus` | Индикатор цикла жизни (см. раздел 4). |
| `pricing_class` | `u8` | Index dans les tiers de prix du suffixe (стандартный, премиум, зарезервированный). |
| `registered_at` | `Timestamp` | Временная метка блока начальной активации. |
| `expires_at` | `Timestamp` | Fin du terme paye. |
| `grace_expires_at` | `Timestamp` | Fin de gracy d'auto-renouvellement (по умолчанию +30 дней). |
| `redemption_expires_at` | `Timestamp` | Финал окончания искупления (по умолчанию +60 дней). |
| `auction` | `Option<NameAuctionStateV1>` | Настоящее время, когда голландцы вновь открываются или расширяют активы премиум-класса. |
| `last_tx_hash` | `Hash` | Pointeur определяет транзакцию, которая является продуктом этой версии. |
| `metadata` | `Metadata` | Метаданные арбитражного регистратора (текстовые записи, доказательства). |

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

| Чемпион | Тип | Заметки |
|-------|------|-------|
| `suffix_id` | `u16` | Кле первый; стабильные между политическими версиями. |
| `suffix` | `AsciiString` | например, `sora`. |
| `steward` | `AccountId` | Стюард определяет Хартию управления. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Идентификатор действия по расчету по умолчанию (например, `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | Коэффициенты призов по номинальной стоимости и правила долгосрочного страхования. |
| `min_term_years` | `u8` | Plancher pour le terme achete quel que soit l'override de tier. |
| `grace_period_days` | `u16` | По умолчанию 30. |
| `redemption_period_days` | `u16` | По умолчанию 60. |
| `max_term_years` | `u8` | Максимальное ожидание обновления. |
| `referral_cap_bps` | `u16` | <=1000 (10%) селон ле чартер. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Liste Fournie Par la Governance с инструкциями по влиянию. |
| `fee_split` | `SuffixFeeSplitV1` | Части тресорери/стюарда/направления (базисные баллы). |
| `fund_splitter_account` | `AccountId` | Compte qui escrow + распределить фонды. |
| `policy_version` | `u16` | Увеличьте изменение чака. |
| `metadata` | `Metadata` | Примечания (соглашение о KPI, хеши документов о соответствии). |

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

### 2.3 Регистрация доходов и расчетов| Структура | Чемпионы | Но |
|--------|--------|-----|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Регистрация определенных маршрутов оплаты в эпоху урегулирования (hebdomadaire). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Emis a chaque paiement poste (регистрация, продление, enchere). |

Все поля `TokenValue` используют исправленную каноническую кодировку Norito с объявленным кодом в ассоциации `SuffixPolicyV1`.

### 2.4 События регистрации

Канонические события содержат журнал повторов для автоматизации DNS/шлюза и аналитики.

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

Эти события включают обновляемый журнал (например, домен `RegistryEvents`) и отражают шлюзы каналов для того, чтобы кэши DNS были недействительны в соответствии с SLA.

## 3. Макет складских запасов и индексов

| Кле | Описание |
|-----|-------------|
| `Names::<name_hash>` | Первая карта `name_hash` против `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Index Secondaire для кошелька пользовательского интерфейса (удобное разбиение на страницы). |
| `NamesByLabel::<suffix_id, normalized_label>` | Обнаружив конфликты, обеспечьте детерминированные исследования. |
| `SuffixPolicies::<suffix_id>` | Дернье `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Исторический `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Регистрировать монотонную последовательность номиналов cle только для добавления. |

Все файлы сериализуются через кортежи Norito для обеспечения определенного хеширования между объектами. Неправильно использовать индексный шрифт с атомным шрифтом и основной регистрацией.

## 4. Машины государственного цикла жизни

| Этат | Условия входа | Разрешения на переходы | Заметки |
|-------|--------------------|---------------------|-------|
| Доступно | Вывод quand `NameRecord` отсутствует. | `PendingAuction` (премиум), `Active` (стандарт регистрации). | La recherche de disponabilite освещает отдельные индексы. |
| Ожидается аукцион | Cree quand `PriceTierV1.auction_kind` != нет. | `Active` (enchere reglee), `Tombstoned` (aucune enchere). | Les encheres emettent `AuctionOpened` и `AuctionSettled`. |
| Активный | Регистрация или продление регистрации. | И18НИ00000143Х, И18НИ00000144Х, И18НИ00000145Х. | `expires_at` пилотный переход. |
| Грейспериод | Автоматика quand `now > expires_at`. | `Active` (временное обновление), `Redemption`, `Tombstoned`. | По умолчанию +30 дней; resolu mais marque. |
| Искупление | `now > grace_expires_at` больше `< redemption_expires_at`. | `Active` (позднее обновление), `Tombstoned`. | Les Commandes Exigent des Frais de Penalite. |
| Замороженный | Заморозка управления или опекуна. | `Active` (после исправления), `Tombstoned`. | Невозможно передать данные контроллерам в течение дня. |
| Надгробие | Откажитесь добровольно, результат будет постоянным, или срок искупления истек. | `PendingAuction` (возобновление открытия на голландском языке) или оставшееся надгробие. | Вечер `NameTombstoned` содержит причину. |Государственные переходы DOIVENT соответствуют `RegistryEventKind` для того, чтобы кеши ниже по течению остались когерентными. Les noms надгробный участник encheres Dutch вновь открывает приложение к полезной нагрузке `AuctionKind::DutchReopen`.

## 5. Канонические события и шлюз синхронизации

Шлюзы подключены к `RegistryEventV1` и синхронизируют DNS/SoraFS через:

1. Ссылка на Recuperer le dernier `NameRecordV1` для последовательности событий.
2. Восстановите шаблоны преобразователя (адреса предпочтительных IH58 + сжатые (`sora`) и второй выбор, текстовые записи).
3. Закрепите пользователей зоны в течение дня с помощью рабочего процесса SoraDNS, зарегистрированного в [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Гарантии вечерней оплаты:

- Сделайте транзакцию, которая повлияет на `NameRecordV1` *doit* в дополнение к вечеру со строгим круассаном `version`.
- События `RevenueSharePosted` ссылаются на расчеты по тарифам `RevenueShareRecordV1`.
- События замораживания/размораживания/захоронения включают хеши артефактов управления в `metadata` для повтора аудита.

## 6. Примеры полезных данных Norito

### 6.1 Пример записи имени

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

## 7. Прохейновые этапы

- **SN-2b (API регистратора и перехватчики управления):** предоставляет структуры через Torii (привязки Norito и JSON) и соединяет проверки доступа к артефактам управления.
- **SN-3 (система аукционов и регистрации):** повторное использование `NameAuctionStateV1` для реализации логики фиксации/раскрытия и повторного открытия голландского языка.
- **SN-5 (Платежи и расчеты):** эксплуататор `RevenueShareRecordV1` для финансовой выверки и автоматизации отношений.

Вопросы или требования к изменениям должны быть заданы с учетом вопросов, связанных с дорожной картой SNS в `roadmap.md` и отражены в `status.md` в рамках fusion.