---
lang: ja
direction: ltr
source: docs/portal/docs/sns/registry-schema.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Эта страница отражает `docs/source/sns/registry_schema.md` и служит канонической копией портала. Исходный файл сохраняется для обновлений переводов.
:::

# Схема реестра Sora ネームサービス (SN-2a)

**Статус:** Черновик 2026-03-24 -- отправлено на ревью программы SNS  
**ロードマップ:** SN-2a「レジストリ スキーマとストレージ レイアウト」  
**Область:** Определить канонические структуры Norito, состояния жизненного цикла и события для Sora Name Service (SNS),レジストリとレジストラ、SDK、ゲートウェイの両方をサポートします。

SN-2a からのアクセス:

1. Идентификаторы и правила хезирования (`SuffixId`、`NameHash`、派生 селекторов)。
2. Norito 構造体/列挙型、構造体、列挙型、層、層、構造体рестра。
3. レイアウトを再実行します。
4. Мазину состояний、охватывающую регистрацию、продление、猶予/救済、凍結、墓石。
5. DNS/ゲートウェイを使用して、セキュリティを確立します。

## 1. Идентификаторы и хезирование

| Идентификатор | Описание | Производная |
|-----------|---------------|-----------|
| `SuffixId` (`u16`) | Идентификатор реестра для суффиксов верхнего уровня (`.sora`、`.nexus`、`.dao`)。 Согласован с каталогом суффиксов в [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md)。 |ガバナンスを強化します。 `SuffixPolicyV1` です。 |
| `SuffixSelector` | Каноническая строковая форма суффикса (ASCII、小文字)。 |例: `.sora` -> `sora`。 |
| `NameSelectorV1` | Бинарный селектор зарегистрированного лейбла。 | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`。 NFC + 小文字の Norm v1 です。 |
| `NameHash` (`[u8;32]`) | Основной ключ поиска、используемый контрактами、событиями и кезами。 | `blake3(NameSelectorV1_bytes)`。 |

日付:

- 標準 v1 (UTS-46 厳密、STD3 ASCII、NFC) をサポートします。 Пользовательские строки ДОЛЖНЫ быть нормализованы перед хезированием.
- Зарезервированные лейблы (из `SuffixPolicyV1.reserved_labels`) никогда не входят в реестр;ガバナンスのみは `ReservedNameAssigned` をオーバーライドします。

## 2. Структуры Norito

### 2.1 名前レコード V1

| Поле | Тип | Примечания |
|------|------|-----------|
| `suffix_id` | `u16` | `SuffixPolicyV1` です。 |
| `selector` | `NameSelectorV1` | Сырые байты селектора для аудита/debug. |
| `name_hash` | `[u8; 32]` | Ключ для карт/событий。 |
| `normalized_label` | `AsciiString` | Читаемый лейбл (Norm v1) です。 |
| `display_label` | `AsciiString` |ケーシングのスチュワード。 косметика。 |
| `owner` | `AccountId` | Управляет продлениями/трансферами. |
| `controllers` | `Vec<NameControllerV1>` | Ссылки на адреса аккаунтов、リゾルバーとメタデータ。 |
| `status` | `NameStatus` | Флаг жизненного цикла (см. Раздел 4)。 |
| `pricing_class` | `u8` | Индекс в ценовых tier суффикса (標準、プレミアム、予約)。 |
| `registered_at` | `Timestamp` | Время блока первичной активации. |
| `expires_at` | `Timestamp` | Конец оплаченного срока. |
| `grace_expires_at` | `Timestamp` |自動更新の猶予期間 (デフォルトは +30 日)。 |
| `redemption_expires_at` | `Timestamp` | Конец окна 引き換え (デフォルトは +60 日)。 |
| `auction` | `Option<NameAuctionStateV1>` |オランダの再開とプレミアム аукционах をご覧ください。 |
| `last_tx_hash` | `Hash` | Детерминированный указатель на транзакцию версии。 |
| `metadata` | `Metadata` |メタデータ レジストラ (テキスト レコード、プルーフ) です。 |

要旨:

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

### 2.2 SuffixPolicyV1| Поле | Тип | Примечания |
|------|------|-----------|
| `suffix_id` | `u16` | Первичный ключ; жабилен между версиями политики. |
| `suffix` | `AsciiString` | 、`sora`。 |
| `steward` | `AccountId` |スチュワード、統治憲章。 |
| `status` | `SuffixStatus` | `Active`、`Paused`、`Revoked`。 |
| `payment_asset_id` | `AsciiString` | Актив 和解 по умолчанию (например `61CtjvNd9T3THAR65GsMVHr82Bjc`)。 |
| `pricing` | `Vec<PriceTierV1>` |層と層が異なります。 |
| `min_term_years` | `u8` |階層をオーバーライドします。 |
| `grace_period_days` | `u16` |デフォルトは 30。
| `redemption_period_days` | `u16` |デフォルトは 60。
| `max_term_years` | `u8` | Максимальная длительность предоплаты. |
| `referral_cap_bps` | `u16` | <=1000 (10%) по チャーター。 |
| `reserved_labels` | `Vec<ReservedNameV1>` |ガバナンスを維持する必要があります。 |
| `fee_split` | `SuffixFeeSplitV1` | Доли 財務/スチュワード/紹介 (基本ポイント)。 |
| `fund_splitter_account` | `AccountId` | Аккаунт エスクロー + распределение средств。 |
| `policy_version` | `u16` | Увеличивается при каждом изменении. |
| `metadata` | `Metadata` | Расзиренные заметки (KPI 規約、ハッシュ ドキュメント、コンプライアンス)。 |

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

### 2.3 解決と和解

|構造体 | Поля | Назначение |
|----------|------|---------------|
| `RevenueShareRecordV1` | `suffix_id`、`epoch_id`、`treasury_amount`、`steward_amount`、`referral_amount`、`escrow_amount`、`settled_at`、`tx_hash`。 | Детерминированная запись распределенных выплат по эпохам 和解 (неделя) |
| `RevenueAccrualEventV1` | `name_hash`、`suffix_id`、`event`、`gross_amount`、`net_amount`、`referral_account`。 | Эмитируется при каждом платеже (登録、更新、オークション)。 |

Все поля `TokenValue` используют каноническое фиксированное кодирование Norito с кодом валюты из связанного `SuffixPolicyV1`。

### 2.4 パラメータ

DNS/ゲートウェイのリプレイ ログが表示されます。

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

再生可能なログ (`RegistryEvents`) とゲートウェイ フィード、DNS キャッシュの機能を利用できます。 SLA です。

## 3. レイアウトとレイアウト

| Ключ | Описание |
|-----|----------|
| `Names::<name_hash>` | Основная карта `name_hash` -> `NameRecordV1`。 |
| `NamesByOwner::<AccountId, suffix_id>` |ウォレット UI (ページネーションフレンドリー)。 |
| `NamesByLabel::<suffix_id, normalized_label>` | Обнаружение конфликтов、детерминированный поиск。 |
| `SuffixPolicies::<suffix_id>` | Актуальный `SuffixPolicyV1`。 |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1`。 |
| `RegistryEvents::<u64>` |追加専用のログ。 |

Все ключи сериализуются Norito タプルは、детерминированного хезирования между хостами です。 Обновления индексов выполняются атомарно вместе с основным рекордом.

## 4. Мазина состояний жизненного цикла

| Состояние | Условия входа | Допустимые переходы | Примечания |
|----------|----------------|----------|----------|
|利用可能 | Производно、когда `NameRecord` отсутствует。 | `PendingAuction` (プレミアム)、`Active` (標準登録)。 | Поиск доступности читает только индексы。 |
|保留中のオークション | Создается, когда `PriceTierV1.auction_kind` != なし。 | `Active` (オークションが決済)、`Tombstoned` (入札なし)。 | Аукционы эмитируют `AuctionOpened` および `AuctionSettled`。 |
|アクティブ |必要に応じてご利用ください。 | `GracePeriod`、`Frozen`、`Tombstoned`。 | `expires_at` двигает переход。 |
|猶予期間 | Автоматически при `now > expires_at`。 | `Active` (オンタイム更新)、`Redemption`、`Tombstoned`。 |デフォルトは +30 日です。 резолвится、но помечено。 |
|償還 | `now > grace_expires_at` から `< redemption_expires_at`。 | `Active`(後期リニューアル)、`Tombstoned`。 | Команды требуют страфного платежа. |
|冷凍 |ガバナンスと保護者を凍結します。 | `Active` (標準)、`Tombstoned`。 |コントローラーを使用してください。 |
|墓石 |償還を求めてください。 | `PendingAuction` (オランダ語再開) или остается 墓石。 | `NameTombstoned` が表示されます。 |

`RegistryEventKind` では、ダウンストリーム キャッシュがキャッシュされています。廃棄された、オランダの再開 аукционы、прикрепляют ペイロード `AuctionKind::DutchReopen`。

## 5. Канонические события и синхронизация ゲートウェイゲートウェイは `RegistryEventV1` と DNS/SoraFS、および次のとおりです。

1. Загрузка последнего `NameRecordV1`, на который указывает последовательность событий.
2. リゾルバー テンプレート (i105 バージョン + 圧縮 (`sora`) как второй выбор、テキスト レコード)。
3. SoraDNS ワークフローを [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) にピン留めします。

メッセージ:

- Каждая транзакция, влияющая на `NameRecordV1`, *должна* добавить ровно одно событие со строго возрастающей `version`。
- `RevenueSharePosted` は、`RevenueShareRecordV1` の決済を完了します。
- `metadata` 日の監査リプレイによるガバナンスの凍結/凍結解除/廃棄。

## 6. ペイロード Norito

### 6.1 名前レコードの説明

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

### 6.2 サフィックスポリシーの説明

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

## 7. Следующие заги

- **SN-2b (レジストラー API およびガバナンス フック):** 構造体 Torii (Norito および JSON バインディング) およびアドミッション チェックとガバナンス クラス。
- **SN-3 (オークション & 登録エンジン):** `NameAuctionStateV1` はコミット/公開、オランダ語は再開します。
- **SN-5 (支払いと決済):** использовать `RevenueShareRecordV1` для финансовой сверки и автоматизации отчетов.

SNS ロードマップと `roadmap.md` の最新情報を確認します。 `status.md` です。