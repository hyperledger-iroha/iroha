---
lang: ru
direction: ltr
source: docs/portal/docs/sns/registry-schema.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание
Установите `docs/source/sns/registry_schema.md` для получения дополнительной информации. Он был отправлен в Лондон.
:::

# مخطط سجل Служба имен Сора (SN-2a)

**Обзор:** Дата: 24 марта 2026 г. -- Новое сообщение в SNS  
**Обзор:** SN-2a «Схема реестра и схема хранения»  
**Уведомления:** Зарегистрируйтесь в Norito для получения дополнительной информации о состоянии системы. Служба имен Sora (SNS) обеспечивает доступ к локальному регистратору, а также к SDK и шлюзам.

В сообщении, опубликованном на сайте SN-2a, говорится:

1. Завершить хеширование (`SuffixId`, `NameHash`, селектор اشتقاق).
2. Norito structs/enums لسجلات الاسماء, سياسات suffix, tiers التسعير, توزيعات الايراد، واحداث السجل.
3. Воспроизведение нового фильма «Старый мир» повторит.
4. الة حالات تغطي التسجيل, التجديد, благодать/искупление, التجميد и надгробие.
5. Установите флажок DNS/шлюз.

## 1. Процесс и хеширование

| المعرف | الوصف | الاشتقاق |
|------------|-------------|------------|
| И18НИ00000018Х (И18НИ00000019Х) | Установите флажок (`.sora`, `.nexus`, `.dao`). Создан в формате [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | американский писатель Код: `SuffixPolicyV1`. |
| `SuffixSelector` | Введите кодовое слово (ASCII, строчные буквы). | Код: `.sora` -> `sora`. |
| `NameSelectorV1` | селектор | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. NFC + строчные буквы Norm v1. |
| И18НИ00000030X (`[u8;32]`) | Он был убит в 2007 году в Нью-Йорке. | `blake3(NameSelectorV1_bytes)`. |

Комментарии к статье:

- Поддержка стандарта Norm v1 (строгий UTS-46, STD3 ASCII, NFC). Выполняется хеширование.
- Установите флажок (номер `SuffixPolicyV1.reserved_labels`) в режиме ожидания; Установите флажок `ReservedNameAssigned`.

## 2. Введите Norito

### 2.1 ИмяЗаписиV1| حقل | النوع | الملاحظات |
|-------|------|-----------|
| `suffix_id` | `u16` | Код `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Селектор выбора параметра لتدقيق/debug. |
| `name_hash` | `[u8; 32]` | مفتاح للخرائط/الاحداث. |
| `normalized_label` | `AsciiString` | Это стандартная версия (стандартная версия Norm v1). |
| `display_label` | `AsciiString` | Стюард احرف يقدمه; تجميلي. |
| `owner` | `AccountId` | Написано в журнале / التحويلات. |
| `controllers` | `Vec<NameControllerV1>` | Используйте функции разрешения, резольверы и метаданные. |
| `status` | `NameStatus` | حالة دورة الحياة (Вечеринка 4). |
| `pricing_class` | `u8` | Доступные уровни доступа (стандартный, премиум, зарезервированный). |
| `registered_at` | `Timestamp` | Он сказал, что это не так. |
| `expires_at` | `Timestamp` | Наслаждайтесь просмотром. |
| `grace_expires_at` | `Timestamp` | نهاية Grace للتجديد التلقائي (по умолчанию +30 дней). |
| `redemption_expires_at` | `Timestamp` | Новое искупление (по умолчанию +60 дней). |
| `auction` | `Option<NameAuctionStateV1>` | В Нидерландах вновь открываются магазины премиум-класса. |
| `last_tx_hash` | `Hash` | Он сказал, что это не так. |
| `metadata` | `Metadata` | метаданные اختيارية للـ регистратора (текстовые записи, доказательства). |

Автор:

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

| حقل | النوع | الملاحظات |
|-------|------|-----------|
| `suffix_id` | `u16` | Он был убит в 2007 году. |
| `suffix` | `AsciiString` | Код `sora`. |
| `steward` | `AccountId` | стюард в чартере الحوكمة. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Это соглашение о расчете (код `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `pricing` | `Vec<PriceTierV1>` | Для каждого уровня доступны уровни. |
| `min_term_years` | `u8` | حد ادنى لمدة الشراء بغض النظر عن переопределяет. |
| `grace_period_days` | `u16` | По умолчанию 30. |
| `redemption_period_days` | `u16` | По умолчанию 60. |
| `max_term_years` | `u8` | Сделайте это в ближайшее время. |
| `referral_cap_bps` | `u16` | <=1000 (10%) чартер. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Это произошло в 2007 году. |
| `fee_split` | `SuffixFeeSplitV1` | حصص казначейство/распорядитель/направление (базисные баллы). |
| `fund_splitter_account` | `AccountId` | Вы можете использовать условное депонирование в любой момент. |
| `policy_version` | `u16` | Он был в Стиве. |
| `metadata` | `Metadata` | ملاحظات موسعة (соглашение KPI, соответствие хеш-кодам). |

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

### 2.3 Обновление и поселение| Структура | حقول | غرض |
|--------|--------|-------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Поселение Сьерра-Луи-Миссиси (Англия). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Он был Стивом Дэйвом (Турнир, Миссисипи). |

جميع حقول `TokenValue`. Это относится к `SuffixPolicyV1`.

### 2.4 Обновление

Откройте журнал повторов, проверьте DNS/шлюз.

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

يجب اضافة الاحداث الى log قابل للاعادة (مثل نطاق `RegistryEvents`) وعكسها الى каналы الخاصة В случае необходимости кэширование DNS в соответствии с SLA.

## 3. تخطيط التخزين والفهارس

| المفتاح | الوصف |
|-----|-------------|
| `Names::<name_hash>` | Установите флажок `name_hash` или `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Кошелёк Дэнни Лэйна (удобная нумерация страниц). |
| `NamesByLabel::<suffix_id, normalized_label>` | Он был убит в 2008 году. |
| `SuffixPolicies::<suffix_id>` | Сообщение `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Код `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | log только для добавления. |

Для проверки кортежей Norito используется хеширование данных. Он был убит в 2017 году в Колумбии.

## 4. Нажмите на кнопку «Получить»

| حالة | شروط الدخول | Справочная информация | الملاحظات |
|-------|----------------|---------------------|------------|
| Доступно | Написано `NameRecord`. | `PendingAuction` (премиум), `Active` (стандартная регистрация). | Это было сделано в 2007 году. |
| Ожидается аукцион | تنشأ عندما `PriceTierV1.auction_kind` != нет. | `Active` (открытый), `Tombstoned` (открытый). | Установите `AuctionOpened` и `AuctionSettled`. |
| Активный | Он сказал ему: "Нет". | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` на сайте. |
| Грейспериод | تلقائي عند `now > expires_at`. | `Active` (отсутствует в России), `Redemption`, `Tombstoned`. | По умолчанию +30 дней; Он сказал: "Лохан Хейн". |
| Искупление | `now > grace_expires_at` или `< redemption_expires_at`. | `Active` (отсутствует), `Tombstoned`. | الاوامر تتطلب رسوم عقوبة. |
| Замороженный | Он был опекуном. | `Active` (по умолчанию), `Tombstoned`. | Он находится в центре внимания диспетчеров. |
| Надгробие | Он был убит, чтобы совершить искупление. | `PendingAuction` (возобновление открытия на голландском языке) и надгробие. | Создан `NameTombstoned` на сайте. |

При вызове `RegistryEventKind` происходит кэширование нисходящего потока. Голландцы вновь открывают полезную нагрузку `AuctionKind::DutchReopen`.

## 5. Изменение настроек и синхронизация

Зарегистрируйтесь для `RegistryEventV1` и DNS/SoraFS:

1. Установите `NameRecordV1` в защитном чехле.
2. Создание шаблонов сопоставителей (в формате i105 + сжатые (`sora`) текстовые записи).
3. Установите флажок SoraDNS в [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).Автор сообщения:

- В качестве примера `NameRecordV1` *يجب* на сайте `version`. بصرامة.
- Установите `RevenueSharePosted` и установите флажок `RevenueShareRecordV1`.
- Заморозить/разморозить/захоронить хэши можно с помощью `metadata`.

## 6. Установите полезные нагрузки Norito.

### 6.1 Информация NameRecord

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "<katakana-i105-account-id>",
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

### 6.2. SuffixPolicy

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "<katakana-i105-account-id>",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("<katakana-i105-account-id>"), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "<katakana-i105-account-id>",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. Дополнительная информация

- **SN-2b (API регистратора и перехватчики управления):** Загрузка структур Torii (Norito и привязки JSON) и проверка допуска.
- **SN-3 (система аукционов и регистрации):** اعادة استخدام `NameAuctionStateV1` لتنفيذ منطق commit/review и Dutch Reopen.
- **SN-5 (Оплата и расчет):** استخخدام `RevenueShareRecordV1` для получения дополнительной информации.

Вы можете связаться с нами по телефону SNS на `roadmap.md`. Он был установлен на `status.md`.