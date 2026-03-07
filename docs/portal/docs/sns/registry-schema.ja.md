---
lang: ja
direction: ltr
source: docs/portal/docs/sns/registry-schema.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f8df534d156b6a3e516cdd71b4c2f8ea2c6473e45afc643000e450d6d331190
source_last_modified: "2025-11-15T16:28:04.299911+00:00"
translation_last_reviewed: 2026-01-01
---

:::note 公式ソース
このページは `docs/source/sns/registry_schema.md` を反映しており、ポータルの公式コピーとして扱います。ソースファイルは翻訳更新のために残します。
:::

# Sora Name Service レジストリ schema (SN-2a)

**ステータス:** 2026-03-24 ドラフト -- SNS プログラムレビューに提出済み  
**ロードマップ:** SN-2a "Registry schema & storage layout"  
**スコープ:** Sora Name Service (SNS) の Norito 構造体、ライフサイクル状態、発行イベントを定義し、registry と registrar の実装が契約・SDK・gateway を横断して決定的になるようにする。

本ドキュメントは SN-2a の schema 成果物として、次を定義する:

1. 識別子と hashing ルール (`SuffixId`, `NameHash`, selector 派生)。
2. 名前レコード、suffix ポリシー、価格 tier、収益配分、registry イベントの Norito structs/enums。
3. 決定的 replay のための保存 layout と index プレフィックス。
4. 登録、更新、grace/redemption、freeze、tombstone を含む状態機械。
5. DNS/gateway 自動化が消費する正準イベント。

## 1. 識別子と hashing

| 識別子 | 説明 | 派生 |
|------------|-------------|------------|
| `SuffixId` (`u16`) | トップレベル suffix (`.sora`, `.nexus`, `.dao`) の registry 識別子。[`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) の suffix カタログと整合。 | ガバナンス投票で割当; `SuffixPolicyV1` に格納。 |
| `SuffixSelector` | suffix の正準文字列 (ASCII, lower-case)。 | 例: `.sora` -> `sora`. |
| `NameSelectorV1` | 登録ラベルのバイナリ selector。 | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. ラベルは Norm v1 に従い NFC + lower-case。 |
| `NameHash` (`[u8;32]`) | 契約、イベント、キャッシュが使う主検索キー。 | `blake3(NameSelectorV1_bytes)`. |

決定性の要件:

- ラベルは Norm v1 (UTS-46 strict, STD3 ASCII, NFC) で正規化する。ユーザー文字列はハッシュ前に必ず正規化する。
- 予約ラベル (`SuffixPolicyV1.reserved_labels`) は registry に入れない。ガバナンスのみの override は `ReservedNameAssigned` を発行する。

## 2. Norito 構造体

### 2.1 NameRecordV1

| フィールド | 型 | メモ |
|-------|------|-------|
| `suffix_id` | `u16` | `SuffixPolicyV1` を参照。 |
| `selector` | `NameSelectorV1` | 監査/debug 用の生 selector bytes。 |
| `name_hash` | `[u8; 32]` | map/event のキー。 |
| `normalized_label` | `AsciiString` | 人間向けラベル (Norm v1 後)。 |
| `display_label` | `AsciiString` | steward の casing; 見た目用。 |
| `owner` | `AccountId` | 更新/移転の制御。 |
| `controllers` | `Vec<NameControllerV1>` | アカウントアドレス、resolver、アプリ metadata への参照。 |
| `status` | `NameStatus` | ライフサイクルフラグ (Section 4)。 |
| `pricing_class` | `u8` | suffix の price tier index (standard, premium, reserved)。 |
| `registered_at` | `Timestamp` | 初期アクティベーションのブロック時刻。 |
| `expires_at` | `Timestamp` | 支払い期間の終了。 |
| `grace_expires_at` | `Timestamp` | 自動更新 grace の終了 (default +30 days)。 |
| `redemption_expires_at` | `Timestamp` | redemption ウィンドウ終了 (default +60 days)。 |
| `auction` | `Option<NameAuctionStateV1>` | Dutch reopen または premium auction の時に付与。 |
| `last_tx_hash` | `Hash` | このバージョンを作った tx への決定的ポインタ。 |
| `metadata` | `Metadata` | registrar の任意 metadata (text records, proofs)。 |

サポート構造体:

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

| フィールド | 型 | メモ |
|-------|------|-------|
| `suffix_id` | `u16` | 主キー。ポリシー版本間で安定。 |
| `suffix` | `AsciiString` | 例: `sora`. |
| `steward` | `AccountId` | governance charter に定義された steward。 |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | デフォルトの settlement asset (例 `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | price tier の係数と期間ルール。 |
| `min_term_years` | `u8` | tier override に関わらず購入期間の下限。 |
| `grace_period_days` | `u16` | Default 30. |
| `redemption_period_days` | `u16` | Default 60. |
| `max_term_years` | `u8` | 前払い更新の最大年数。 |
| `referral_cap_bps` | `u16` | <=1000 (10%) per charter. |
| `reserved_labels` | `Vec<ReservedNameV1>` | governance 提供の割当ルール付きリスト。 |
| `fee_split` | `SuffixFeeSplitV1` | treasury / steward / referral の比率 (basis points)。 |
| `fund_splitter_account` | `AccountId` | escrow を保持し資金分配するアカウント。 |
| `policy_version` | `u16` | 変更ごとに増分。 |
| `metadata` | `Metadata` | 追加メモ (KPI covenant, compliance doc hashes)。 |

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

### 2.3 収益と settlement レコード

| Struct | フィールド | 目的 |
|--------|--------|---------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | 週次 settlement エポックごとの分配支払いの決定的記録。 |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | 支払いが記録されるたびに発行 (登録、更新、auction)。 |

`TokenValue` は Norito の固定小数点エンコードを使用し、通貨コードは対応する `SuffixPolicyV1` で宣言する。

### 2.4 レジストリイベント

正準イベントは DNS/gateway 自動化と分析のための replay log を提供する。

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

イベントは replay 可能なログ (例: `RegistryEvents` ドメイン) に追加し、gateway feeds にも反映して DNS cache を SLA 内で無効化する。

## 3. 保存レイアウトとインデックス

| キー | 説明 |
|-----|-------------|
| `Names::<name_hash>` | `name_hash` から `NameRecordV1` への主マップ。 |
| `NamesByOwner::<AccountId, suffix_id>` | wallet UI 向け二次インデックス (pagination friendly)。 |
| `NamesByLabel::<suffix_id, normalized_label>` | 競合検出と決定的検索。 |
| `SuffixPolicies::<suffix_id>` | 最新の `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` の履歴。 |
| `RegistryEvents::<u64>` | 単調増加シーケンスで key される append-only log。 |

すべてのキーは Norito tuples でシリアライズし、ホスト間の hashing を決定的に保つ。インデックス更新は主レコードと原子的に行う。

## 4. ライフサイクル状態機械

| 状態 | 入口条件 | 許可される遷移 | メモ |
|-------|----------------|---------------------|------------|
| Available | `NameRecord` が存在しない場合に導出。 | `PendingAuction` (premium), `Active` (standard registration). | 可用性検索はインデックスのみ読む。 |
| PendingAuction | `PriceTierV1.auction_kind` != none の場合に生成。 | `Active` (auction settles), `Tombstoned` (no bids). | Auction は `AuctionOpened` と `AuctionSettled` を発行。 |
| Active | 登録または更新に成功。 | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` が遷移を駆動。 |
| GracePeriod | `now > expires_at` で自動遷移。 | `Active` (on-time renewal), `Redemption`, `Tombstoned`. | Default +30 days; 解決はするがフラグ付き。 |
| Redemption | `now > grace_expires_at` かつ `< redemption_expires_at`. | `Active` (late renewal), `Tombstoned`. | コマンドにペナルティ料金が必要。 |
| Frozen | governance または guardian による freeze。 | `Active` (remediation 後), `Tombstoned`. | transfer や controllers 更新不可。 |
| Tombstoned | 自主返却、恒久的な紛争結果、redemption 期限切れ。 | `PendingAuction` (Dutch reopen) か tombstoned 継続。 | `NameTombstoned` イベントは理由必須。 |

状態遷移は対応する `RegistryEventKind` を必ず発行し、下流 cache の一貫性を保つ。Dutch reopen に入る tombstoned 名は `AuctionKind::DutchReopen` の payload を付与する。

## 5. 正準イベントと gateway 同期

Gateways は `RegistryEventV1` を購読し、DNS/SoraFS を次の手順で同期する:

1. イベントシーケンスで参照される最新の `NameRecordV1` を取得。
2. Resolver templates を再生成 (IH58 推奨 + compressed (`sora`) 次善の addresses, text records)。
3. [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) の SoraDNS フローで更新ゾーンを pin。

イベント配信の保証:

- `NameRecordV1` に影響する各トランザクションは *必ず* `version` が厳密に増加するイベントを 1 件だけ追加する。
- `RevenueSharePosted` は `RevenueShareRecordV1` の settlement を参照する。
- Freeze/unfreeze/tombstone のイベントは監査 replay のため `metadata` に governance artefact hash を含める。

## 6. Norito payload 例

### 6.1 NameRecord 例

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

### 6.2 SuffixPolicy 例

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

## 7. 次のステップ

- **SN-2b (Registrar API & governance hooks):** これらの structs を Torii で公開し (Norito/JSON bindings)、admission checks を governance artefact に接続する。
- **SN-3 (Auction & registration engine):** `NameAuctionStateV1` を再利用し commit/reveal と Dutch reopen のロジックを実装。
- **SN-5 (Payment & settlement):** `RevenueShareRecordV1` を利用して財務リコンシリエーションとレポート自動化を行う。

変更要望や質問は `roadmap.md` の SNS 更新と合わせて記録し、マージ時に `status.md` に反映する。
