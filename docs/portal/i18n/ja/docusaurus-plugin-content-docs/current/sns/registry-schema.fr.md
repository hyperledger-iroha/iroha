---
lang: ja
direction: ltr
source: docs/portal/docs/sns/registry-schema.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note ソースカノニク
`docs/source/sns/registry_schema.md` のページを参照して、canonique du portail のコピーを確認してください。 Le fichier ソースは、1 時間の翻訳に役立ちます。
:::

# 登録スキーマ Sora ネームサービス (SN-2a)

**ステータス:** Redige 2026-03-24 -- プログラム SNS のレビュー  
**リアンロードマップ:** SN-2a「レジストリスキーマとストレージレイアウト」  
**寄稿者:** Norito 正規の構造、システムのサイクルおよびイベントの定義、Sora ネーム サービス (SNS) の実装、およびレジストラとレジストラの保持の決定、およびコントラクト、SDK、ゲートウェイを定義します。

SN-2a のスキーマを作成するための完全なドキュメント:

1. 識別子とハッシュに関する規則 (`SuffixId`、`NameHash`、選択の導出)。
2. 構造体/列挙型 Norito は、登録名、政略、接尾辞、順位、収入の再分配、および登録上のイベントを表します。
3. ストックのレイアウトとインデックスのプレフィックスは、再生時に決定されます。
4. 登録、再構築、猶予/救済、凍結と墓石を要求する機械はありません。
5. 自動化 DNS/ゲートウェイのコンソメを確立します。

## 1. 識別子とハッシュ化

|識別子 |説明 |導出 |
|-----------|---------------|-----------|
| `SuffixId` (`u16`) |プレミア ニボーの接尾辞を登録する識別子 (`.sora`、`.nexus`、`.dao`)。 [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) のサフィックス カタログを調整します。 |統治権の投票属性。在庫は`SuffixPolicyV1`です。 |
| `SuffixSelector` |接尾辞の形式 (ASCII、小文字)。 |例: `.sora` -> `sora`。 |
| `NameSelectorV1` | Selecteur binaire pour le label enregistre。 | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`。 Le label est NFC + 小文字 selon Norm v1。 |
| `NameHash` (`[u8;32]`) |コントラット、イベント、キャッシュなどを利用する最初の記録です。 | `blake3(NameSelectorV1_bytes)`。 |

決定性の緊急性:

- ラベルは Norm v1 (UTS-46 strict、STD3 ASCII、NFC) 経由で正規化されません。 DOIVENT は、ハッシュを正常に利用するために使用します。
- ラベルの予約 (`SuffixPolicyV1.reserved_labels`) は登録されません。これは、一意のガバナンス管理機能 `ReservedNameAssigned` をオーバーライドします。

## 2. 構造体 Norito

### 2.1 名前レコード V1

|チャンピオン |タイプ |メモ |
|------|------|------|
| `suffix_id` | `u16` |参照 `SuffixPolicyV1`。 |
| `selector` | `NameSelectorV1` |監査/デバッグを行うオクテット デュ セレクター ブリュット。 |
| `name_hash` | `[u8; 32]` |クレは地図やイベントを流してくれます。 |
| `normalized_label` | `AsciiString` | lisible pour l'humain (Norm v1 以降) というラベルを付けます。 |
| `display_label` | `AsciiString` |スチュワード用のケーシング。コスメティック・オプションネル。 |
| `owner` | `AccountId` | Controle les renouvellements/transfers。 |
| `controllers` | `Vec<NameControllerV1>` |アプリケーションの参照とアドレス、リゾルバー、メタデータ。 |
| `status` | `NameStatus` |サイクル開発の指標 (セクション 4)。 |
| `pricing_class` | `u8` |接尾辞の階層のインデックス (標準、プレミアム、予約)。 |
| `registered_at` | `Timestamp` |アクティベーションの初期ブロックのタイムスタンプ。 |
| `expires_at` | `Timestamp` |フィン・デュ・テルメ・ペイ。 |
| `grace_expires_at` | `Timestamp` |自動再生の終わり (デフォルト +30 時間)。 |
| `redemption_expires_at` | `Timestamp` |償還期限 (デフォルト +60 時間)。 |
| `auction` | `Option<NameAuctionStateV1>` |現在のQuand desオランダは、プレミアムソンアクティブを再開します。 |
| `last_tx_hash` | `Hash` |製品バージョンのトランザクションに対する決定点を示します。 |
| `metadata` | `Metadata` |レジストラによるメタデータの裁定 (テキスト記録、証明)。 |

サポートされる構造体:

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

### 2.2 SuffixPolicyV1|チャンピオン |タイプ |メモ |
|------|------|------|
| `suffix_id` | `u16` |クレ・プリメール。安定した政治的バージョン。 |
| `suffix` | `AsciiString` |たとえば、`sora`。 |
| `steward` | `AccountId` |統治権憲章の管理者を定義します。 |
| `status` | `SuffixStatus` | `Active`、`Paused`、`Revoked`。 |
| `payment_asset_id` | `AsciiString` |デフォルトによる和解の識別子 (例: `xor#sora`)。 |
| `pricing` | `Vec<PriceTierV1>` |優先順位と義務期間の係数。 |
| `min_term_years` | `u8` |計画を立てて、最終的な問題を解決します。 |
| `grace_period_days` | `u16` |デフォルトは 30。
| `redemption_period_days` | `u16` |デフォルトは 60。
| `max_term_years` | `u8` |最高の復興への期待。 |
| `referral_cap_bps` | `u16` | <=1000 (10%) セロン・ル・チャーター。 |
| `reserved_labels` | `Vec<ReservedNameV1>` | Liste fournie par la gouvernance avec指示 d'affectation。 |
| `fee_split` | `SuffixFeeSplitV1` |トレゾレリー / スチュワード / 紹介 (基本ポイント) の一部。 |
| `fund_splitter_account` | `AccountId` |秘密鍵を保管し、フォンを配布します。 |
| `policy_version` | `u16` |チャックの変更を増やします。 |
| `metadata` | `Metadata` |メモのエテンデュ (KPI 規約、ドキュメントのコンプライアンスのハッシュ)。 |

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

### 2.3 収益および和解の登録

|構造体 |チャンピオン |しかし |
|--------|--------|-----|
| `RevenueShareRecordV1` | `suffix_id`、`epoch_id`、`treasury_amount`、`steward_amount`、`referral_amount`、`escrow_amount`、`settled_at`、`tx_hash`。 |登録は、エポック・デ・セトルメント（hebdomadaire）によるルートの決定を決定します。 |
| `RevenueAccrualEventV1` | `name_hash`、`suffix_id`、`event`、`gross_amount`、`net_amount`、`referral_account`。 | Chaque paiement poste (登録、再構築、enchere) を受け取ります。 |

Tous les Champs `TokenValue` utilisent l'encodage fixe canonique de Norito avec le code detects dans le `SuffixPolicyV1` associe.

### 2.4 登録上の行事

DNS/ゲートウェイと分析の自動化を行うため、ログ デ リプレイを正常に実行できます。

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

ログの再取得 (例: ドメイン `RegistryEvents`) とフィード ゲートウェイ プール クエリ キャッシュ DNS 無効と SLA を参照します。

## 3. 在庫とインデックスのレイアウト

|クレ |説明 |
|-----|---------------|
| `Names::<name_hash>` | `name_hash` と `NameRecordV1` のプライマリをマップします。 |
| `NamesByOwner::<AccountId, suffix_id>` |インデックス セカンダリ プール UI ウォレット (ページネーションに優しい)。 |
| `NamesByLabel::<suffix_id, normalized_label>` |衝突を検出し、決定的な研究を行います。 |
| `SuffixPolicies::<suffix_id>` |デルニエ `SuffixPolicyV1`。 |
| `RevenueShare::<suffix_id, epoch_id>` |ヒストリック`RevenueShareRecordV1`。 |
| `RegistryEvents::<u64>` |シーケンス単調のログ追加専用クリア。 |

タプル Norito を介してシリアル化されたファイルは、ホテル内でハッシュを決定するために送信されます。登録プリンシパルを参照してください。

## 4. サイクル・デ・ヴィーの機械設計

|イータット |メインディッシュの条件 |遷移が許可されます |メモ |
|------|-------|----------|------|
|利用可能 |派生クアンド `NameRecord` est がありません。 | `PendingAuction` (プレミアム)、`Active` (登録標準)。 | La recherche de disponibilite lit seulement les Indexes。 |
|保留中のオークション | Cree quand `PriceTierV1.auction_kind` != なし。 | `Active`（アンシェール・レグ​​リー）、`Tombstoned`（オーキュヌ・アンシェール）。 | Les encheres emettent `AuctionOpened` と `AuctionSettled`。 |
|アクティブ |登録または再構築。 | `GracePeriod`、`Frozen`、`Tombstoned`。 | `expires_at` パイロット ラ トランジション。 |
|猶予期間 | Automatique Quand `now > expires_at`。 | `Active` (一時更新)、`Redemption`、`Tombstoned`。 |デフォルト +30 時間。レゾル・マイス・マルク。 |
|償還 | `now > grace_expires_at` は `< redemption_expires_at` になります。 | `Active` (再構築遅刻)、`Tombstoned`。 |刑罰の緊急命令。 |
|冷凍 |統治者を凍結せよ、守護者よ。 | `Active` (修復後)、`Tombstoned`。 | Ne peut pas transferer ni metre a jour les コントローラー。 |
|墓石 |ボランティアを放棄し、永久に訴訟を起こし、償還期限が切れます。 | `PendingAuction` (オランダ語再開) あなたは墓石にされています。 | L'evenement `NameTombstoned` には、存在意義が含まれています。 |DOIVENT emettre le `RegistryEventKind` の移行は、ダウンストリームの保持一貫性のあるキャッシュをキャッシュします。オランダのペイロード `AuctionKind::DutchReopen` の添付ファイルを再開するという名前は、エンシェルの墓石のようなエントリです。

## 5. Evenements canoniques と同期ゲートウェイ

ゲートウェイは `RegistryEventV1` を維持せず、次の経由で DNS/SoraFS を同期します。

1. 一連のイベントに関する Recuperer le dernier `NameRecordV1` 参照。
2. リゾルバー テンプレートの再生成 (I105 優先 + 圧縮 (`sora`) の 2 番目の選択、テキスト レコードのアドレス)。
3. ワークフロー SoraDNS の説明 [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) を介して、ゾーンを 1 時間逃すピナー。

保証期間の保証:

- トランザクションは `NameRecordV1` に影響を及ぼします *doit* ajouter exactement unevenement avec `version` strictement croissante。
- 和解案 `RevenueSharePosted` の和解案 `RevenueShareRecordV1` を参照。
- 凍結/凍結解除/墓石に含まれるイベントは、`metadata` で監査を再生するための芸術品のハッシュです。

## 6. ペイロードの例 Norito

### 6.1 NameRecord の例

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

### 6.2 SuffixPolicy の例

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "i105...",
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
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("i105..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "i105...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. プロカイン・エテープ

- **SN-2b (レジストラ API およびガバナンス フック):** Torii (バインディング Norito および JSON) を介したエクスポーザ CES 構造体およびコネクタ ファイルは、ガバナンス上のアドミッション アーティファクトをチェックします。
- **SN-3 (オークションおよび登録エンジン):** 再利用者 `NameAuctionStateV1` は、ロジックのコミット/公開およびオランダ語の再オープンの実装者を注ぎます。
- **SN-5 (支払いと決済):** 悪用者 `RevenueShareRecordV1` は金融調整と関係自動化を実行します。

変更を要求する質問や、政府の承認を得るために、1 日のロードマップ SNS と `roadmap.md` と `status.md` の融合を要求します。