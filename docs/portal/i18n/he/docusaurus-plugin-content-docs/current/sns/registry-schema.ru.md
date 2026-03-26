---
lang: he
direction: rtl
source: docs/portal/docs/sns/registry-schema.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Эта страница отражает `docs/source/sns/registry_schema.md` ו- служит канонической копией портала. Исходный файл сохраняется для обновлений переводов.
:::

# Схема реестра Sora Name Service (SN-2a)

**סטטוס:** שרנוביק 2026-03-24 -- אופטימיזציה של מערכת SNS  
**סכימה על מפת הדרכים:** SN-2a "סכימת רישום ופריסת אחסון"  
**Область:** Определить канонические структуры Norito, состояния жизненного цикла и событич Service для Sora), רישום רישום ורשם מבצעים פעולות בקשר, SDK ושערים.

Этот документ завершает поставку схемы для SN-2a, определяя:

1. Идентификаторы и правила хеширования (`SuffixId`, `NameHash`, גזירה селекторов).
2. Norito מבנים/סכמות עבור записей имен, политик суффиксов, ценовых שכבות, распределений доходов и событий.
3. פריסה хранения и префиксы индексов למשחק חוזר של детерминированного.
4. Машину состояний, охватывающую регистрацию, продление, חסד/גאולה, הקפאה ומצבה.
5. Канонические события, потребляемые DNS/gateway автоматизацией.

## 1. Идентификаторы и хеширование

| Идентификатор | Описание | Производная |
|------------|----------------|-------------|
| `SuffixId` (`u16`) | Идентификатор реестра для суффиксов верхнего уровня (`.sora`, `.nexus`, `.dao`). Согласован с каталогом суффиксов в [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Назначается голосованием по ממשל; хранится в `SuffixPolicyV1`. |
| `SuffixSelector` | Каноническая строковая форма суффикса (ASCII, אותיות קטנות). | דוגמה: `.sora` -> `sora`. |
| `NameSelectorV1` | Бинарный селектор зарегистрированного лейбла. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Лейбл в NFC + אותיות קטנות по Norm v1. |
| `NameHash` (`[u8;32]`) | Основной ключ поиска, используемый контрактами, событиями и кешами. | `blake3(NameSelectorV1_bytes)`. |

Требования детерминизма:

- Лейблы нормализуются через Norm v1 (UTS-46 strict, STD3 ASCII, NFC). Пользовательские строки ДОЛЖНЫ быть нормализованы перед хешированием.
- Зарезервированные лейблы (יз `SuffixPolicyV1.reserved_labels`) никогда не входят в реестр; עוקף ממשל בלבד выпускают события `ReservedNameAssigned`.

## 2. סטרוקטורות Norito

### 2.1 NameRecordV1| Поле | טיפ | Примечания |
|-------|------|--------|
| `suffix_id` | `u16` | סמל עבור `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Сырые байты селектора для аудита/debug. |
| `name_hash` | `[u8; 32]` | Ключ для карт/событий. |
| `normalized_label` | `AsciiString` | Читаемый лейбл (после Norm v1). |
| `display_label` | `AsciiString` | מארז של דייל; косметика. |
| `owner` | `AccountId` | Управляет продлениями/трансферами. |
| `controllers` | `Vec<NameControllerV1>` | הגדרות כתובות, פותרים או מטא נתונים приложения. |
| `status` | `NameStatus` | Флаг жизненного цикла (см. Раздел 4). |
| `pricing_class` | `u8` | Индекс в ценовых tiers суффикса (סטנדרטי, פרימיום, שמור). |
| `registered_at` | `Timestamp` | Время блока первичной активации. |
| `expires_at` | `Timestamp` | Конец оплаченного срока. |
| `grace_expires_at` | `Timestamp` | חידוש אוטומטי של Конец חסד (ברירת מחדל +30 дней). |
| `redemption_expires_at` | `Timestamp` | פדיון Конец окна (ברירת מחדל +60 дней). |
| `auction` | `Option<NameAuctionStateV1>` | Присутствует при פתיחה מחדש בהולנדית או פרימיום аукционах. |
| `last_tx_hash` | `Hash` | Детерминированный указатель на транзакцию версии. |
| `metadata` | `Metadata` | רשם מטא נתונים של Произвольная (רשומות טקסט, הוכחות). |

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

| Поле | טיפ | Примечания |
|-------|------|--------|
| `suffix_id` | `u16` | Первичный ключ; стабилен между версиями политики. |
| `suffix` | `AsciiString` | דוגמה, `sora`. |
| `steward` | `AccountId` | דייל, определенный в governance charter. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | התנחלות Актив по умолчанию (например `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | Коэффициенты цен по tiers и правила длительности. |
| `min_term_years` | `u8` | Минимальный срок покупки вне зависимости от דחיפת שכבות. |
| `grace_period_days` | `u16` | ברירת מחדל 30. |
| `redemption_period_days` | `u16` | ברירת מחדל 60. |
| `max_term_years` | `u8` | Максимальная длительность предоплаты. |
| `referral_cap_bps` | `u16` | <=1000 (10%) по צ'רטר. |
| `reserved_labels` | `Vec<ReservedNameV1>` | תיאור של ממשל с инструкциями назначения. |
| `fee_split` | `SuffixFeeSplitV1` | Доли אוצר / דייל / הפניה (נקודות בסיס). |
| `fund_splitter_account` | `AccountId` | Аккаунт נאמנות + распределение средств. |
| `policy_version` | `u16` | Увеличивается при каждом изменении. |
| `metadata` | `Metadata` | Расширенные заметки (אמנת KPI, hashes docs по ציות). |

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

### 2.3 Записи доходов и הסדר| מבנה | Поля | Назначение |
|--------|------|--------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, Norito, Norito. | Детерминированная запись распределенных выплат по эпохам הסדר (неделя). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Эмитируется при каждом платеже (רישום, חידוש, מכירה פומבית). |

Все поля `TokenValue` используют каноническое фиксированное кодирование Norito с кодом валюсты `SuffixPolicyV1`.

### 2.4 События реестра

Канонические события дают יומן הפעלה חוזר ל-DNS/gateway автоматизации аналитики.

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

События должны добавляться в replayable log (לדוגמה, домен `RegistryEvents`) ו- зеркалироваться в gateway feeds, чтобиры DNS caches пределах SLA.

## 3. פריסה хранения и индексы

| Ключ | Описание |
|-----|--------|
| `Names::<name_hash>` | Основная карта `name_hash` -> `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Вторичный индекс עבור ממשק המשתמש של ארנק (ידידותי לדפיון). |
| `NamesByLabel::<suffix_id, normalized_label>` | Обнаружение конфликтов, детерминированный поиск. |
| `SuffixPolicies::<suffix_id>` | Актуальный `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | קובץ `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | יומן הוספה בלבד с монотонной последовательностью. |

Все ключи сериализуются Norito tuples для детерминированного хеширования между хостами. Обновления индексов выполняются атомарно вместе с основным рекордом.

## 4. Машина состояний жизненного цикла

| Состояние | Условия входа | Допустимые переходы | Примечания |
|-------|----------------|---------------------|------------|
| זמין | Производно, когда `NameRecord` отсутствует. | `PendingAuction` (פרימיום), `Active` (רישום סטנדרטי). | Поиск доступности читает только индексы. |
| מכירה פומבית בהמתנה | Создается, когда `PriceTierV1.auction_kind` != אין. | `Active` (מכירה פומבית מסתיימת), `Tombstoned` (ללא הצעות). | Аукционы эмитируют `AuctionOpened` ו-`AuctionSettled`. |
| פעיל | Регистрация или продление успешно. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` двигает переход. |
| תקופת חסד | Автоматически при `now > expires_at`. | `Active` (חידוש בזמן), `Redemption`, `Tombstoned`. | ברירת מחדל +30 дней; резолвится, но помечено. |
| גאולה | `now > grace_expires_at` לא `< redemption_expires_at`. | `Active` (חידוש מאוחר), `Tombstoned`. | Команды требуют штрафного платежа. |
| קפוא | הקפאת ניהול או אפוטרופוס. | `Active` (после ремедиации), `Tombstoned`. | Нельзя передавать или обновлять בקרים. |
| מצבה | Добровольная сдача, итог спора или истекшая גאולה. | `PendingAuction` (פתיחה מחדש בהולנד) или остается מצבה. | Событие `NameTombstoned` должно включать причину. |Переходы состояний ДОЛЖНЫ эмитировать соответствующий `RegistryEventKind`, чтобы downstream caches оставанлисьсь с. Tombstoned имена, входящие в Dutch reopen аукционы, прикрепляют מטען `AuctionKind::DutchReopen`.

## 5. שער Канонические события и синхронизация

Gateways подписываются ב-`RegistryEventV1` ו- синхронизируют DNS/SoraFS, выполняя:

1. Загрузка последнего `NameRecordV1`, на который указывает последовательность событий.
2. תבניות פותר Перегенерация (i105 предпочтительно + דחוס (`sora`) как второй выбор, רשומות טקסט).
3. Pin обновленных данных зоны через זרימת העבודה של SoraDNS из [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Гарантии доставки событий:

- Каждая транзакция, влияющая на `NameRecordV1`, *должна* добавить ровно одно событие со строего вою `version`.
- `RevenueSharePosted` события ссылаются на התנחלויות из `RevenueShareRecordV1`.
- הקפאה/ביטול הקפאה/טומבסטון.

## 6. מטענים Примеры Norito

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

- **SN-2b (Registrar API & Governance Hooks):** открыть эти structs через Torii (Norito ו-JSON bindings) и привязать этифарть chez admission к governance.
- **SN-3 (מנוע מכירות פומביות ורישום):** переиспользовать `NameAuctionStateV1` ל- логики commit/reveal и הולנדית לפתוח מחדש.
- **SN-5 (תשלום וסילוק):** использовать `RevenueShareRecordV1` для финансовой сверки и автоматизации отчетов.

חיפושים ופיתוחים של מפת הדרכים של SNS ב-`roadmap.md` и отражать70X отражать70X слиянии.