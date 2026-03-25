---
lang: ru
direction: ltr
source: docs/portal/docs/sns/registry-schema.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание
Если у вас есть `docs/source/sns/registry_schema.md`, вы можете сделать это, когда хотите کام کرتا ہے۔ سورس فائل ترجمہ اپ ڈیٹس کے لیے برقرار رہے گی۔
:::

# Служба имен Сора رجسٹری اسکیمہ (SN-2a)

**حیثیت:** от 24 марта 2026 г. -- SNS پروگرام ریویو کے لیے کیا گیا  
**Обзор:** SN-2a «Схема реестра и схема хранения»  
**دائرہ:** Служба имен Sora (SNS) کے لیے Norito Наличие реестра, регистратора, реализаций, SDK и шлюзов, детерминированный подход.

В комплект поставки входит SN-2a, который может быть использован в качестве оружия. Ответ:

1. Процесс хеширования (`SuffixId`, `NameHash`, деривация селектора).
2. نام ریکارڈز, суффикс پالیسیز, قیمت tiers, ریونیو سپلٹس اور رجسٹری ایونٹس کے لیے Norito структуры/перечисления۔
3. детерминированное воспроизведение, структура хранилища и индексные префиксы.
4. رجسٹریشن، تجدید، благодать/искупление, замораживание и надгробие и государственная машина۔
5. Автоматизация DNS/шлюзов и канонические события.

## 1. Процесс хеширования

| شناخت | وضاحت | خذ |
|------------|-------------|------------|
| И18НИ00000019X (`u16`) | С суффиксами لیول (`.sora`, `.nexus`, `.dao`) или لیے رجسٹری شناخت۔ [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) Каталог суффиксов کے مطابق۔ | голосование за управление سے مختص; `SuffixPolicyV1` میں محفوظ۔ |
| `SuffixSelector` | суффикс کی каноническая строка شکل (ASCII, строчные буквы)۔ | Код: `.sora` -> `sora`. |
| `NameSelectorV1` | Нажмите кнопку селектора. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. لیبل Norm v1 کے مطابق NFC + строчные буквы۔ |
| И18НИ00000031X (`[u8;32]`) | Доступ к кэшам и выбор ключа | `blake3(NameSelectorV1_bytes)`. |

Детерминизм:

- Стандарт Norm v1 (строгий UTS-46, STD3 ASCII, NFC) или нормализация нормализации Строки и хеширование, нормализация и нормализация
- Зарезервированные метки (`SuffixPolicyV1.reserved_labels`). Переопределения только для управления `ReservedNameAssigned`

## 2. Norito ساختیں

### 2.1 ИмяЗаписиV1| فیلڈ | قسم | نوٹس |
|-------|------|-------|
| `suffix_id` | `u16` | `SuffixPolicyV1` کی ریفرنس۔ |
| `selector` | `NameSelectorV1` | аудит/отладка или необработанные байты селектора. |
| `name_hash` | `[u8; 32]` | карты/события کے لیے key۔ |
| `normalized_label` | `AsciiString` | انسانی قابلِ پڑھائی لیبل (Norm v1 کے بعد)۔ |
| `display_label` | `AsciiString` | стюард کی корпус; اختیاری косметика۔ |
| `owner` | `AccountId` | продление/перенос |
| `controllers` | `Vec<NameControllerV1>` | Использование резольверов и метаданных, а также метаданных |
| `status` | `NameStatus` | لائف سائیکل فلیگ (سیکشن 4 دیکھیں)۔ |
| `pricing_class` | `u8` | суффикс کے ценовые уровни کا индекс (стандартный, премиум, зарезервированный)۔ |
| `registered_at` | `Timestamp` | Активация ابتدائی کا بلاک ٹائم۔ |
| `expires_at` | `Timestamp` | ادا شدہ مدت کا اختتام۔ |
| `grace_expires_at` | `Timestamp` | автоматическое продление льготного периода (по умолчанию +30 дней)۔ |
| `redemption_expires_at` | `Timestamp` | Окно погашения کا اختتام (по умолчанию +60 дней)۔ |
| `auction` | `Option<NameAuctionStateV1>` | Голландия вновь открывает аукционы премиум-класса. |
| `last_tx_hash` | `Hash` | В качестве детерминированного указателя можно использовать детерминированный указатель. |
| `metadata` | `Metadata` | регистратор کی произвольные метаданные (текстовые записи, доказательства) ۔ |

Созданные структуры:

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

| فیلڈ | قسم | نوٹس |
|-------|------|-------|
| `suffix_id` | `u16` | Ключ بنیادی; پالیسی ورژنز میں مستحکم۔ |
| `suffix` | `AsciiString` | مثال کے طور پر `sora`۔ |
| `steward` | `AccountId` | устав управления میں متعین steward۔ |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | идентификатор расчетного актива по умолчанию (مثلا `61CtjvNd9T3THAR65GsMVHr82Bjc`)۔ |
| `pricing` | `Vec<PriceTierV1>` | Каковы уровни и коэффициенты? |
| `min_term_years` | `u8` | خریدی گئی مدت کے لیے کم از کم حد۔ |
| `grace_period_days` | `u16` | По умолчанию 30. |
| `redemption_period_days` | `u16` | По умолчанию 60. |
| `max_term_years` | `u8` | پیشگی تجدید کی زیادہ سے زیادہ مدت۔ |
| `referral_cap_bps` | `u16` | <=1000 (10%) чартерный платеж |
| `reserved_labels` | `Vec<ReservedNameV1>` | управление کی فراہم کردہ فہرست مع назначить ہدایات۔ |
| `fee_split` | `SuffixFeeSplitV1` | казначейство / управляющий / направление حصص (базисные баллы)۔ |
| `fund_splitter_account` | `AccountId` | Депозитное депонирование |
| `policy_version` | `u16` | ہر تبدیلی پر بڑھتا ہے۔ |
| `metadata` | `Metadata` | توسیعی نوٹس (соглашение KPI, хэши документов соответствия)۔ |

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

### 2.3 ریونیو اور Settlement ریکارڈز| Структура | فیلڈز | مقصد |
|--------|-------|------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | эпоха поселений (ہفتہ وار) کے حساب سے разгромленный ادائیگیوں کا детерминированный ریکارڈ۔ |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | ہر ادائیگی پوسٹ ہونے پر эмитировать (регистрация, продление, аукцион)۔ |

`TokenValue` или Norito — каноническое кодирование с фиксированной запятой. متعلقہ `SuffixPolicyV1` میں объявите ہوتا ہے۔

### 2.4 رجسٹری ایونٹس

Канонические события Автоматизация DNS/шлюза Аналитика и журнал повторов فراہم کرتے ہیں۔

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

Доступ к воспроизводимому журналу (домен `RegistryEvents`) Добавление дополнительных каналов шлюза и зеркала DNS-кэши SLA могут быть недействительными.

## 3. Структура хранилища с индексами

| Ключ | وضاحت |
|-----|-------------|
| `Names::<name_hash>` | `name_hash` سے `NameRecordV1` تک بنیادی map۔ |
| `NamesByOwner::<AccountId, suffix_id>` | Индекс пользовательского интерфейса кошелька (удобная нумерация страниц)۔ |
| `NamesByLabel::<suffix_id, normalized_label>` | обнаружение конфликтов کرتا ہے اور детерминированный поиск فعال بناتا ہے۔ |
| `SuffixPolicies::<suffix_id>` | تازہ ترین `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` ہسٹری۔ |
| `RegistryEvents::<u64>` | журнал только для добавления جس کی монотонная последовательность клавиш ہے۔ |

Ключи Norito, кортежи, сериализация, хосты, детерминированное хеширование. обновления индекса بنیادی ریکارڈ کے ساتھ атомарно

## 4. Конечный автомат жизненного цикла

| Государство | Условия участия | Разрешенные переходы | Заметки |
|-------|------------------|--------------------|-------|
| Доступно | جب `NameRecord` موجود نہ ہو۔ | `PendingAuction` (премиум), `Active` (стандартная регистрация). | наличие поиск صرف индексы پڑھتی ہے۔ |
| Ожидается аукцион | جب `PriceTierV1.auction_kind` != нет ہو۔ | `Active` (аукцион рассчитан), `Tombstoned` (нет ставок). | аукционы `AuctionOpened` اور `AuctionSettled` испускают کرتی ہیں۔ |
| Активный | رجسٹریشن یا تجدید کامیاب ہو۔ | И18НИ00000143Х, И18НИ00000144Х, И18НИ00000145Х. | `expires_at` переход چلاتا ہے۔ |
| Грейспериод | جب `now > expires_at` ہو۔ | `Active` (своевременное продление), `Redemption`, `Tombstoned`. | По умолчанию +30 дней; разрешить ہوتا ہے مگر флаг ہوتا ہے۔ |
| Искупление | `now > grace_expires_at` или `< redemption_expires_at`. | `Active` (позднее продление), `Tombstoned`. | کمانڈز پر штраф درکار ہے۔ |
| Замороженный | управление یا хранитель замораживания۔ | `Active` (устранение ошибок), `Tombstoned`. | перенос обновления контроллеров نہیں کر سکتے۔ |
| Надгробие | رضاکارانہ сдача, спор نتیجہ, искупление ختم۔ | `PendingAuction` (открытие на голландском языке) یا надгробие رہتا ہے۔ | `NameTombstoned` ایونٹ میں وجہ شامل ہونی چاہیے۔ |

Переходы между состояниями `RegistryEventKind` испускают кэши нижестоящего потока Tombstoned на возобновленных голландских аукционах میں داخل ہوں وہ `AuctionKind::DutchReopen` payload شامل کرتے ہیں۔## 5. Канонические события и синхронизация шлюза

Шлюзы `RegistryEventV1` для синхронизации с DNS/SoraFS для синхронизации:

1. Последовательность действий, которая может быть использована для `NameRecordV1` کریں۔
2. Шаблоны резольверов دوبارہ بنائیں (I105 ترجیحی + сжатые (`sora`) вторые лучшие адреса, текстовые записи)۔
3. [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) Задайте рабочий процесс SoraDNS для вывода данных зоны.

Гарантии проведения мероприятия:

- ہر ٹرانزیکشن جو `NameRecordV1` в случае необходимости *لازمی* طور پر `version` کے ساتھ صرف ایک ایونٹ شامل کرے جو سختی سے بڑھتی ہو۔
- `RevenueSharePosted` ایونٹس `RevenueShareRecordV1` سے эмитированные расчеты کو reference کرتے ہیں۔
- заморозить/разморозить/захоронить повтор аудита کے لیے `metadata` میں хэши артефактов управления شامل کرتے ہیں۔

## 6. Полезные нагрузки Norito

### 6.1 Имя записи

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "i105...",
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

### 6.2 Политика суффикса

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "i105...",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("i105..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "i105...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. اگلے اقدامات

- **SN-2b (API регистратора и перехватчики управления):** структуры Torii могут предоставлять доступ (Norito привязки JSON) и проверки допуска и артефакты управления. سے جوڑیں۔
- **SN-3 (система аукционов и регистрации):** зафиксировать/показать голландскую логику повторного открытия `NameAuctionStateV1` دوبارہ استعمال کریں۔
- **SN-5 (Платежи и расчеты):** Для выверки данных используется автоматизация `RevenueShareRecordV1`.

Если вы хотите использовать SNS `roadmap.md`, вы также можете использовать SNS. Для слияния используйте `status.md`.