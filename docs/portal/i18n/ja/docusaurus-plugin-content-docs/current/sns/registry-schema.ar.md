---
lang: ja
direction: ltr
source: docs/portal/docs/sns/registry-schema.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::メモ
`docs/source/sns/registry_schema.md` を確認してください。 بقى ملف المصدر لتحديثات الترجمة.
:::

# مخطط سجل Sora ネームサービス (SN-2a)

**الحالة:** مسودة 2026-03-24 -- مقدمة لمراجعة برنامج SNS  
** SN-2a "レジストリ スキーマとストレージ レイアウト"  
**النطاق:** تعريف هياكل Norito القياسية وحالات دورة الحياة والاحداث المنبعثة لخدمة Sora ネームサービス (SNS)レジストラと SDK およびゲートウェイを管理します。

SN-2a の評価:

1. ハッシュ (`SuffixId`、`NameHash`、セレクター)。
2. Norito 構造体/列挙型。サフィックス層。
3. 再生を確認してください。
4. 墓石。
5. DNS/ゲートウェイ。

## 1. ハッシュ化

|ああ |ああ |ああ |
|-----------|---------------|-----------|
| `SuffixId` (`u16`) | (`.sora`、`.nexus`、`.dao`)。 [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md)。 | يعين بتصويت الحوكمة; `SuffixPolicyV1`。 |
| `SuffixSelector` | الشكل النصي القياسي للسوفكس (ASCII、小文字)。 |意味: `.sora` -> `sora`。 |
| `NameSelectorV1` |セレクターを選択します。 | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`。 NFC + 小文字の Norm v1。 |
| `NameHash` (`[u8;32]`) |重要な情報を確認してください。 | `blake3(NameSelectorV1_bytes)`。 |

重要:

- 標準 v1 (UTS-46 厳密、STD3 ASCII、NFC)。ハッシュ化。
- الوسوم المحجوزة (من `SuffixPolicyV1.reserved_labels`) لا تدخل السجل ابدا; `ReservedNameAssigned` を参照してください。

## 2. هياكل Norito

### 2.1 名前レコード V1

|ああ |ああ | और देखें
|------|------|-----------|
| `suffix_id` | `u16` | `SuffixPolicyV1`。 |
| `selector` | `NameSelectorV1` |セレクター / デバッグ。 |
| `name_hash` | `[u8; 32]` |重要な意味。 |
| `normalized_label` | `AsciiString` | قابل للقراءة (Norm v1)。 |
| `display_label` | `AsciiString` |管理人。そうです。 |
| `owner` | `AccountId` | حكمفي التجديدات/التحويلات。 |
| `controllers` | `Vec<NameControllerV1>` |重要なのは、リゾルバーとメタデータです。 |
| `status` | `NameStatus` | حالة دورة الحياة (انظر القسم 4)。 |
| `pricing_class` | `u8` |レベル (標準、プレミアム、予約)。 |
| `registered_at` | `Timestamp` |ありがとうございます。 |
| `expires_at` | `Timestamp` | और देखें |
| `grace_expires_at` | `Timestamp` |グレース للتجديد التلقائي (デフォルト +30 يوما)。 |
| `redemption_expires_at` | `Timestamp` |引き換え (デフォルト +60)。 |
| `auction` | `Option<NameAuctionStateV1>` |オランダはプレミアムを再開します。 |
| `last_tx_hash` | `Hash` | ؤشر حتمي على المعاملة التي انتجت هذه النسخة。 |
| `metadata` | `Metadata` |メタデータ レジストラ (テキスト レコード、プルーフ)。 |

重要:

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

|ああ |ああ | और देखें
|------|------|-----------|
| `suffix_id` | `u16` |重要な意味を持っています。 |
| `suffix` | `AsciiString` | `sora`。 |
| `steward` | `AccountId` |スチュワードは、憲章を取得します。 |
| `status` | `SuffixStatus` | `Active`、`Paused`、`Revoked`。 |
| `payment_asset_id` | `AsciiString` |決済完了 (مثلا `xor#sora`)。 |
| `pricing` | `Vec<PriceTierV1>` |レベルを上げます。 |
| `min_term_years` | `u8` |はオーバーライドされます。 |
| `grace_period_days` | `u16` |デフォルトは 30。
| `redemption_period_days` | `u16` |デフォルトは 60。
| `max_term_years` | `u8` |ありがとうございます。 |
| `referral_cap_bps` | `u16` | <=1000 (10%) € チャーター。 |
| `reserved_labels` | `Vec<ReservedNameV1>` | قائمة من الحوكمة مع تعليمات التخصيص. |
| `fee_split` | `SuffixFeeSplitV1` |財務/スチュワード/紹介(基本ポイント)。 |
| `fund_splitter_account` | `AccountId` |エスクロー エスクロー。 |
| `policy_version` | `u16` | زيد مع كل تغيير。 |
| `metadata` | `Metadata` | ملاحظات موسعة (KPI 規約、ハッシュ لوثائق コンプライアンス)。 |

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

### 2.3 決済と決済|構造体 |ああ |ああ |
|----------|----------|----------|
| `RevenueShareRecordV1` | `suffix_id`、`epoch_id`、`treasury_amount`、`steward_amount`、`referral_amount`、`escrow_amount`、`settled_at`、`tx_hash`。 |決済（اسبوعيا）。 |
| `RevenueAccrualEventV1` | `name_hash`、`suffix_id`、`event`、`gross_amount`、`net_amount`、`referral_account`。 | يصدر عند تسجيل كل دفعة (تسجيل، تجديد، مزاد)。 |

認証済み `TokenValue` 認証済み `SuffixPolicyV1`ああ。

### 2.4 説明

DNS/ゲートウェイのリプレイ ログ。

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

ログイン ログ ログ (`RegistryEvents`) フィード フィードDNS の SLA キャッシュ。

## 3. いいえ

|ああ |ああ |
|-----|---------------|
| `Names::<name_hash>` |テスト `name_hash` テスト `NameRecordV1`。 |
| `NamesByOwner::<AccountId, suffix_id>` |ウォレット (ページネーションフレンドリー)。 |
| `NamesByLabel::<suffix_id, normalized_label>` | كشف التعارضات وتمكين البحث الحتمي. |
| `SuffixPolicies::<suffix_id>` | `SuffixPolicyV1`。 |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1`。 |
| `RegistryEvents::<u64>` |ログは追加専用です。 |

Norito タプルはハッシュ化されたものです。最高のパフォーマンスを見せてください。

## 4. いいえ、いいえ。

|ああ | और देखेंニュース | ニュースऔर देखें
|----------|----------------|----------|----------|
|利用可能 | شتقة عند غياب `NameRecord`。 | `PendingAuction` (プレミアム)、`Active` (標準登録)。 |ありがとうございます。 |
|保留中のオークション | `PriceTierV1.auction_kind` != なし。 | `Active` (評価)、`Tombstoned` (評価)。 | `AuctionOpened` と `AuctionSettled` を確認してください。 |
|アクティブ |最高です。 | `GracePeriod`、`Frozen`、`Tombstoned`。 | `expires_at` は。 |
|猶予期間 | `now > expires_at` です。 | `Active` (英語)、`Redemption`、`Tombstoned`。 |デフォルト +30; زال يحل لكنه معلّم。 |
|償還 | `now > grace_expires_at` は `< redemption_expires_at` です。 | `Active` (評価)、`Tombstoned`。 |ありがとうございます。 |
|冷凍 |守護者。 | `Active` (国際)、`Tombstoned`。 |コントローラー。 |
|墓石 |償還を求めてください。 | `PendingAuction` (オランダ語再開) او يبقى 墓石。 | حدث `NameTombstoned` يجب ان يتضمن السبب. |

`RegistryEventKind` は、ダウンストリームのキャッシュをキャッシュします。オランダの再開ペイロード `AuctionKind::DutchReopen`。

## 5. 同期する

`RegistryEventV1` DNS/SoraFS 番号:

1. `NameRecordV1` は、次のとおりです。
2. 解析リゾルバー テンプレート (解析 IH58 解析 + 圧縮 (`sora`) 解析テキスト レコード)。
3. [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md)。

回答:

- كل معاملة تؤثر على `NameRecordV1` *يجب* ان تضيف حدثا واحدا فقط مع `version` متزايد بصرامة.
- `RevenueSharePosted` は `RevenueShareRecordV1` です。
- 凍結/凍結解除/廃棄のハッシュ値を指定します。`metadata` を指定します。

## 6. Norito ペイロード

### 6.1 ネームレコード

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

### 6.2 SuffixPolicy

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

## 7. いいえ

- **SN-2b (レジストラー API およびガバナンス フック):** 構造体 Torii (Norito JSON バインディング) およびアドミッション チェック。
- **SN-3 (オークションおよび登録エンジン):** اعادة استخدام `NameAuctionStateV1` لتنفيذ منطق commit/reveal و オランダの再開。
- **SN-5 (支払いおよび決済):** استخدام `RevenueShareRecordV1` للتسوية المالية واتوتمة التقارير。

يجب تقديم الاسئلة او طلبات التغيير مع تحديثات خارطة طريق SNS في `roadmap.md` وعكسها في `status.md` です。