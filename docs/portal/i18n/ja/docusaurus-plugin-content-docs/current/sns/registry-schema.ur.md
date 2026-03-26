---
lang: ja
direction: ltr
source: docs/portal/docs/sns/registry-schema.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note メモ
یہ صفحہ `docs/source/sns/registry_schema.md` کی عکاسی کرتا ہے اور پورٹل کی کینونیکل کاپی کے طور پر کام کرتا ❁❁❁❁ سورس فائل ترجمہ اپ ڈیٹس کے لیے برقرار رہے گی۔
:::

# ソラネームサービス رجسٹری اسکیمہ (SN-2a)

**حیثیت:** مسودہ 2026-03-24 -- SNS پروگرام ریویو کے لیے جمع کیا گیا  
** 説明:** SN-2a 「レジストリ スキーマとストレージ レイアウト」  
**دائرہ:** Sora Name Service (SNS) کے لیے Norito اسٹرکچرز، لائف سائیکل اسٹیٹس اور اخراجی ایونٹسレジストリ レジストラ 実装 SDK ゲートウェイ 決定論的

SN-2a سکیمہ ڈیلیورایبل کو مکمل کرتی ہے، جس میں درج شامل ہیں:

1. ハッシュハッシュ (`SuffixId`、`NameHash`、セレクター派生)۔
2. نام ریکارڈز، 接尾語 پالیسیز، قیمت tiers، ریونیو سپلٹس اور رجسٹری ایونٹس کے لیے Norito 構造体/列挙型
3. 決定論的再生とストレージ レイアウトとインデックス プレフィックス
4. 猶予/償還、凍結、墓石、ステートマシン
5. DNS/ゲートウェイの自動化と正規のイベント

## 1. ハッシュ化

|認証済み | |ああ |
|-----------|---------------|-----------|
| `SuffixId` (`u16`) | ٹاپ لیول サフィックス (`.sora`、`.nexus`、`.dao`) [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) サフィックス カタログ مطابق۔ |統治投票、投票。 `SuffixPolicyV1` حفوظ۔ |
| `SuffixSelector` |接尾辞 正規文字列 شکل (ASCII、小文字) |意味: `.sora` -> `sora`。 |
| `NameSelectorV1` |セレクタを選択する| `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`。 Norm v1 مطابق NFC + 小文字|
| `NameHash` (`[u8;32]`) |パスワード キャッシュ パスワード キー キー| `blake3(NameSelectorV1_bytes)`。 |

決定論:

- Norm v1 (UTS-46 strict、STD3 ASCII、NFC) の正規化 ہوتے ہیں۔文字列のハッシュ化 正規化 正規化
- 予約済みラベル (`SuffixPolicyV1.reserved_labels`) レジストリ میں داخل نہیں ہوتے؛ガバナンスのみのオーバーライド `ReservedNameAssigned`

## 2. Norito और देखें

### 2.1 名前レコード V1

|ああ | और देखेंにゅう |
|------|------|------|
| `suffix_id` | `u16` | `SuffixPolicyV1` すごい|
| `selector` | `NameSelectorV1` |監査/デバッグ、生のセレクター バイト、 |
| `name_hash` | `[u8; 32]` |マップ/イベント アイコン キー|
| `normalized_label` | `AsciiString` | انسانی قابلِ پڑھائی لیبل (Norm v1 کے بعد)۔ |
| `display_label` | `AsciiString` |スチュワード ケーシング化粧品|
| `owner` | `AccountId` |更新/移籍|
| `controllers` | `Vec<NameControllerV1>` |リゾルバー、メタデータ、およびメタデータ|
| `status` | `NameStatus` | لائف سائیکل فلیگ (سیکشن 4 دیکھیں)۔ |
| `pricing_class` | `u8` |サフィックス 価格階層 インデックス (標準、プレミアム、予約) |
| `registered_at` | `Timestamp` |アクティベーションを行う|
| `expires_at` | `Timestamp` | دا شدہ مدت کا اختتام۔ |
| `grace_expires_at` | `Timestamp` |自動更新猶予期間 (デフォルト +30 年) |
| `redemption_expires_at` | `Timestamp` |引き換え期間 (デフォルト +60 ユーロ) |
| `auction` | `Option<NameAuctionStateV1>` |オランダ、プレミアムオークションを再開|
| `last_tx_hash` | `Hash` |決定論的ポインタ|
| `metadata` | `Metadata` |レジストラが任意にメタデータ (テキスト記録、プルーフ) を作成する|

構造体:

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

### 2.2 SuffixPolicyV1|ああ | और देखेंにゅう |
|------|------|------|
| `suffix_id` | `u16` | 「キー」 پالیسی ورژنز میں مستحکم۔ |
| `suffix` | `AsciiString` |重要 `sora`۔ |
| `steward` | `AccountId` |ガバナンス憲章 重要なスチュワード|
| `status` | `SuffixStatus` | `Active`、`Paused`、`Revoked`。 |
| `payment_asset_id` | `AsciiString` |デフォルト決済資産識別子 (مثلا `61CtjvNd9T3THAR65GsMVHr82Bjc`)۔ |
| `pricing` | `Vec<PriceTierV1>` |層数 係数 数 階数 係数数|
| `min_term_years` | `u8` | خریدی گئی مدت کے لیے کم از کم حد۔ |
| `grace_period_days` | `u16` |デフォルトは 30。
| `redemption_period_days` | `u16` |デフォルトは 60。
| `max_term_years` | `u8` | پیشگی تجدید کی زیادہ سے زیادہ مدت۔ |
| `referral_cap_bps` | `u16` | <=1000 (10%) チャーター|
| `reserved_labels` | `Vec<ReservedNameV1>` |ガバナンス فراہم کردہ فہرست مع 割り当て ہدایات۔ |
| `fee_split` | `SuffixFeeSplitV1` |財務 / スチュワード / 紹介 حصص (基本ポイント)۔ |
| `fund_splitter_account` | `AccountId` |エスクロー رکھنے اور فنڈز تقسیم کرنے والا اکاؤنٹ۔ |
| `policy_version` | `u16` | ہر تبدیلی پر بڑھتا ہے۔ |
| `metadata` | `Metadata` | توسیعی نوٹس (KPI 規約、コンプライアンス文書ハッシュ)۔ |

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

### 2.3 決着

|構造体 | और देखしているのすごい |
|------|------|------|
| `RevenueShareRecordV1` | `suffix_id`、`epoch_id`、`treasury_amount`、`steward_amount`、`referral_amount`、`escrow_amount`、`settled_at`、`tx_hash`。 |決済エポック (ہفتہ وار) کے حساب سے ルーティングされた ادائیگیوں کا deterministic ریکارڈ۔ |
| `RevenueAccrualEventV1` | `name_hash`、`suffix_id`、`event`、`gross_amount`、`net_amount`、`referral_account`。 | ہر ادائیگی پوسٹ ہونے پر 発行 (登録、更新、オークション)۔ |

`TokenValue` 関数 Norito 正規固定小数点エンコーディング`SuffixPolicyV1` میں 宣言 ہوتا ہے۔

### 2.4 いいえ

正規イベント DNS/ゲートウェイの自動化 分析分析 再生ログ 記録

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

再生可能なログ (`RegistryEvents` ドメイン) 追加、ゲートウェイ フィード、ミラー、DNS キャッシュ、SLA、無効にする ہوں۔

## 3. ストレージレイアウトとインデックス

|キー | |
|-----|---------------|
| `Names::<name_hash>` | `name_hash` 地図 `NameRecordV1` 地図|
| `NamesByOwner::<AccountId, suffix_id>` |ウォレット UI のインデックス (ページネーションに適しています) |
| `NamesByLabel::<suffix_id, normalized_label>` |競合を検出する 決定論的検索 فعال بناتا ہے۔ |
| `SuffixPolicies::<suffix_id>` | `SuffixPolicyV1` です。 |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` فسٹری۔ |
| `RegistryEvents::<u64>` |追加専用ログ جس کی キー シーケンス 単調 ہے۔ |

キー Norito タプル シリアル化 ہوتی ہیں ホスト ٩ے ハッシュ決定論的ハッシュインデックス更新 بنیادی ریکارڈ کے ساتھ 原子的に ہوتی ہیں۔

## 4. ライフサイクル ステート マシン

|状態 |エントリー条件 |許可される遷移 |メモ |
|------|------|----------------------|------|
|利用可能 | جب `NameRecord` موجود نہ ہو۔ | `PendingAuction` (プレミアム)、`Active` (標準登録)。 |在庫状況検索インデックス پڑھتی ہے۔ |
|保留中のオークション | جب `PriceTierV1.auction_kind` != なし ہو۔ | `Active` (オークションが決済)、`Tombstoned` (入札なし)。 |オークション `AuctionOpened` اور `AuctionSettled` を放出する|
|アクティブ | جسٹریشن یا تجدید کامیاب ہو۔ | `GracePeriod`、`Frozen`、`Tombstoned`。 | `expires_at` 遷移|
|猶予期間 | جب `now > expires_at` ہو۔ | `Active` (オンタイム更新)、`Redemption`、`Tombstoned`。 |デフォルト +30 ユーロ;解決する ہوتا ہے مگر flag ہوتا ہے۔ |
|償還 | `now > grace_expires_at` は `< redemption_expires_at` です。 | `Active`(後期リニューアル)、`Tombstoned`。 |ペナルティ料金|
|冷凍 |ガバナンス ガーディアン フリーズ| `Active` (修復)、`Tombstoned`。 |転送 コントローラー アップデート 完了|
|墓石 |降伏 紛争 紛争 償還 償還| `PendingAuction` (オランダ語再開) یا 墓石 رہتا ہے۔ | `NameTombstoned` ایونٹ میں وجہ شامل ہونی چاہیے۔ |

状態遷移 `RegistryEventKind` はダウンストリーム キャッシュを発行します。墓石のようなもの オランダのオークション再開 میں داخل ہوں وہ `AuctionKind::DutchReopen` payload شامل کرتے ہیں۔

## 5. 正規イベントとゲートウェイ同期

ゲートウェイ `RegistryEventV1` と同期 DNS/SoraFS と同期:1. ایونٹ sequence میں حوالہ کردہ تازہ ترین `NameRecordV1` حاصل کریں۔
2. リゾルバー テンプレート (i105 バージョン + 圧縮 (`sora`) 2 番目に最適なアドレス、テキスト レコード)
3. [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) ゾーンデータピンのSoraDNSワークフロー

イベント配信の保証:

- ہر ٹرانزیکشن جو `NameRecordV1` پر اثر ڈالے *لازمی* طور پر `version` کے ساتھ صرف ایک ایونٹ شامل کرے جو سختی سے بڑھتی ہو۔
- `RevenueSharePosted` 番号 `RevenueShareRecordV1` 発行された決済数 参照番号
- 凍結/凍結解除/廃棄 監査リプレイ `metadata` ガバナンス アーティファクト ハッシュ

## 6. Norito ペイロード

### 6.1 名前レコードの説明

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

### 6.2 SuffixPolicy の説明

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

## 7. ああ、

- **SN-2b (レジストラー API およびガバナンス フック):** 構造体 Torii を公開する (Norito JSON バインディング) アドミッション チェック ガバナンス アーティファクトを公開するऔर देखें
- **SN-3 (オークションおよび登録エンジン):** コミット/公開 オランダ語の再開ロジック کے لیے `NameAuctionStateV1` دوبارہ استعمال کریں۔
- **SN-5 (支払いと決済):** 調整、自動化、`RevenueShareRecordV1` استعمال کریں۔

سوالات یا تبدیلی کی درخواستیں `roadmap.md` میں SNS اپ ڈیٹس کے ساتھ درج کریں اور merge کے وقت `status.md` میں شامل کریں۔