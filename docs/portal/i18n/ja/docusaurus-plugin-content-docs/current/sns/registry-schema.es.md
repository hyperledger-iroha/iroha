---
lang: ja
direction: ltr
source: docs/portal/docs/sns/registry-schema.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ノート フエンテ カノニカ
Esta pagina refleja `docs/source/sns/registry_schema.md` y ahora sirve como la copia canonica del portal。取引を実際に行うためのアーカイブ。
:::

# Esquema del registro del Sora ネーム サービス (SN-2a)

**Estado:** Redactado 2026-03-24 -- SNS プログラムの改訂版を参照  
**ロードマップの詳細:** SN-2a「レジストリ スキーマとストレージ レイアウト」  
**Alcance:** 構造 Norito を定義し、コントラトス、SDK、ゲートウェイの管理を決定し、Sora ネーム サービス (SNS) の実装方法を決定します。

SN-2a の詳細に関する完全な文書:

1. ハッシュの規則を識別します (`SuffixId`、`NameHash`、セレクターの派生)。
2. 構造体/列挙型 Norito 登録番号、政治、政治、政治、イベントの記録。
3. リプレイ決定時のインデックスの優先レイアウト。
4. Una maquina de estados que cubre registro、renovacion、gracia/redencion、y の墓石を凍結します。
5. DNS/ゲートウェイの自動化に関するイベントを実行します。

## 1. 識別子とハッシュ

|識別子 |説明 |派生 |
|-----------|---------------|-----------|
| `SuffixId` (`u16`) |優れた登録情報 (`.sora`、`.nexus`、`.dao`)。 Alineado は、[`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) のカタログを参照してください。 |アシニャド・ポル・ヴォト・デ・ゴベルナンザ。アルマセナド en `SuffixPolicyV1`。 |
| `SuffixSelector` | canonica en string del sufijo 形式 (ASCII、小文字)。 |例: `.sora` -> `sora`。 |
| `NameSelectorV1` |登録されたエチケットのセレクタ。 | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`。 NFC + 小文字セグン Norm v1 のルール。 |
| `NameHash` (`[u8;32]`) |クラーベの主要なバスケダ ポート コントラトス、イベント、キャッシュ。 | `blake3(NameSelectorV1_bytes)`。 |

決定性の要件:

- Norm v1 (UTS-46 strict、STD3 ASCII、NFC) を介して正規化されます。 DEBEN はハッシュを正規化します。
- 登録情報の保存 (`SuffixPolicyV1.reserved_labels`) 登録番号。 los は、solo de gobernanza のイベント `ReservedNameAssigned` をオーバーライドします。

## 2. 構造 Norito

### 2.1 名前レコード V1

|カンポ |ティポ |メモ |
|------|------|------|
| `suffix_id` | `u16` |参照番号 `SuffixPolicyV1`。 |
| `selector` | `NameSelectorV1` |オーディオ/デバッグ用のプロセッサーのセレクターのバイト数。 |
| `name_hash` | `[u8; 32]` |クラーベパラマパス/イベント。 |
| `normalized_label` | `AsciiString` |人間にとって読みやすいエチケット (Norm v1 以降)。 |
| `display_label` | `AsciiString` |ケーシングの管轄区域管理者。コスメティアオプション。 |
| `owner` | `AccountId` |改修/移転を制御します。 |
| `controllers` | `Vec<NameControllerV1>` |オブジェクトの指示、リゾルバー、またはアプリケーションのメタデータを参照します。 |
| `status` | `NameStatus` | Bandera de ciclo de vida (セクション 4 版)。 |
| `pricing_class` | `u8` |金利の階層のインデックス (標準、プレミアム、予約済み)。 |
| `registered_at` | `Timestamp` |ブロックのアクティベーションの初期タイムスタンプ。 |
| `expires_at` | `Timestamp` |フィン・デル・テルミーノ・パガード。 |
| `grace_expires_at` | `Timestamp` |自動改修の終わり (デフォルト +30 ディアス)。 |
| `redemption_expires_at` | `Timestamp` | Fin de ventana de redencion (デフォルト +60 dias)。 |
| `auction` | `Option<NameAuctionStateV1>` | Presente cuando se reabre オランダ語のサブバス プレミアム エスタン アクティバス。 |
| `last_tx_hash` | `Hash` | Puntero determinista a la transaccion que produjo esta version. |
| `metadata` | `Metadata` |登録機関による任意のメタデータ (テキスト記録、証明)。 |

主な構造体:

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

### 2.2 SuffixPolicyV1|カンポ |ティポ |メモ |
|------|------|------|
| `suffix_id` | `u16` |クラーベ・プリマリア。政治の安定したバージョン。 |
| `suffix` | `AsciiString` |例として、`sora`。 |
| `steward` | `AccountId` |スチュワードはゴベルナンザ憲章を決定します。 |
| `status` | `SuffixStatus` | `Active`、`Paused`、`Revoked`。 |
| `payment_asset_id` | `AsciiString` |欠陥のある決済の活動識別子 (por ejemplo `61CtjvNd9T3THAR65GsMVHr82Bjc`)。 |
| `pricing` | `Vec<PriceTierV1>` |保険料と耐久性の係数。 |
| `min_term_years` | `u8` | Piso para el termino comprado sin importar は層をオーバーライドします。 |
| `grace_period_days` | `u16` |デフォルトは 30。
| `redemption_period_days` | `u16` |デフォルトは 60。
| `max_term_years` | `u8` |アデランタドによる改修の最大の利点。 |
| `referral_cap_bps` | `u16` | <=1000 (10%) セグン エル チャーター。 |
| `reserved_labels` | `Vec<ReservedNameV1>` |割り当てに関する指示をリストします。 |
| `fee_split` | `SuffixFeeSplitV1` |ポルシオネス・デ・テソレリア / スチュワード / 紹介 (基本ポイント)。 |
| `fund_splitter_account` | `AccountId` | Cuenta que mantiene エスクロー + フォンドの配布。 |
| `policy_version` | `u16` |カンビオを増やしていきます。 |
| `metadata` | `Metadata` | Notas extendidas (KPI規約、文書化されたハッシュ)。 |

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

### 2.3 和解の登録簿

|構造体 |カンポス |プロポジト |
|----------|--------|----------|
| `RevenueShareRecordV1` | `suffix_id`、`epoch_id`、`treasury_amount`、`steward_amount`、`referral_amount`、`escrow_amount`、`settled_at`、`tx_hash`。 |登録は、和解の時期を決定するパゴス (セマナル) です。 |
| `RevenueAccrualEventV1` | `name_hash`、`suffix_id`、`event`、`gross_amount`、`net_amount`、`referral_account`。 | Emitido cada vez que un pago se registra (登録、改修、サブバスタ)。 |

Todos los Campos `TokenValue` usan la codificacion fija canonica de Norito con el codigo de moneda declarado en el `SuffixPolicyV1` asociado。

### 2.4 登録イベント

ロスイベントは、DNS/ゲートウェイと分析の自動化によるログ再生ができないことを証明しています。

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

イベントログは、再現可能なログ (記録、`RegistryEvents`) を収集し、ゲートウェイのフィードを参照して、キャッシュ DNS を無効にし、SLA を無効にします。

## 3. アルマセナミエントのインデックスのレイアウト

|クラーベ |説明 |
|-----|---------------|
| `Names::<name_hash>` | `name_hash` と `NameRecordV1` の最初のマップ。 |
| `NamesByOwner::<AccountId, suffix_id>` | UI のウォレットの秒数をインデックスします (ページ分割可能)。 |
| `NamesByLabel::<suffix_id, normalized_label>` |矛盾を検出し、決定的なハビリタ ブスケーダ。 |
| `SuffixPolicies::<suffix_id>` |ウルティモ `SuffixPolicyV1`。 |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` の歴史。 |
| `RegistryEvents::<u64>` |ログ追加専用の con clave de secuencia monotonica。 |

Todas las claves se Serializan usando tuplas Norito para mantener el hashing determinista entre hosts。初期の登録に必要な形式のインデックスが実際に作成されます。

## 4. マキナ・デ・エスタドス・デル・シクロ・デ・ヴィーダ

|エスタード |コンディショネス デ エントラーダ |トランシシオネス許可 |メモ |
|----------|--------------------------|--------------------------|----------|
|利用可能 |デリバド クアンド `NameRecord` エスタ アウセンテ。 | `PendingAuction` (プレミアム)、`Active` (レジストロスタンダード)。 | La Busqueda de disponibilidad lee ソロインデックス。 |
|保留中のオークション | Creado cuando `PriceTierV1.auction_kind` != なし。 | `Active` (la subasta se liquida)、`Tombstoned` (sin pujas)。 |最後のサブバスは `AuctionOpened` と `AuctionSettled` を発します。 |
|アクティブ |レジストロ・オ・リノベーション・エキトーザ。 | `GracePeriod`、`Frozen`、`Tombstoned`。 | `expires_at` 過渡期の衝動。 |
|猶予期間 |オートマチッククアンド `now > expires_at`。 | `Active`（リノベーション・ア・ティエンポ）、`Redemption`、`Tombstoned`。 |デフォルト +30 dia;アウン・リズエルベ・ペロ・マルカド。 |
|償還 | `now > grace_expires_at` ペロ `< redemption_expires_at`。 | `Active`（リノベーションターディア）、`Tombstoned`。 |ロスコマンドスは罰金を要求します。 |
|冷凍 |フリーズ・デ・ゴベルナンザ・オ・ガーディアン。 | `Active` (トラス修復)、`Tombstoned`。 |実際のコントローラに転送する必要はありません。 |
|墓石 |任意の意思表示、永続的な紛争の結果、期限切れの通知。 | `PendingAuction` (オランダの再開) o 永続的な墓石。 |エル イベント `NameTombstoned` デベ イ​​ンクルイル ラゾン。 |ラス トランジション デ スタド DEBEN 送信者 el 通信者 `RegistryEventKind` パラ ケ ラス キャッシュ ダウンストリーム セマンテンガン コヒーレンテス。 Los nombres tombstoned que entran en subastas オランダの再開補助装置とペイロード `AuctionKind::DutchReopen`。

## 5. ゲートウェイのイベントと同期

`RegistryEventV1` を購読しているゲートウェイが失われていて、DNS/SoraFS メディアンテが発生しています:

1. イベントの最後まで `NameRecordV1` を参照してください。
2. Regenerar テンプレート デ リゾルバー (指示 i105 優先 + 圧縮 (`sora`) コモ セグンダ オプション、テキスト レコード)。
3. [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) の SoraDNS 記述を介して、実際のデータを確認します。

イベントの開催日:

- `NameRecordV1` *デベ* は、`version` 制限付きイベントに対する正確な取引を要求します。
- ロス イベント `RevenueSharePosted` 参照液体 `RevenueShareRecordV1` の放出。
- 凍結/凍結解除/墓石のイベントには、`metadata` パラリプレイ デ オーディオのハッシュが含まれます。

## 6. ペイロードの例 Norito

### 6.1 NameRecord の使用法

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

### 6.2 SuffixPolicy の使用方法

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

## 7. プロキシモスパソス

- **SN-2b (レジストラー API およびガバナンス フック):** Torii 経由の exponer estos structs (バインディング Norito y JSON) y コネクタ アドミッション チェックは、管理上の成果物をチェックします。
- **SN-3 (オークションおよび登録エンジン):** オランダ語リオープンのコミット/公開ロジックの再利用 `NameAuctionStateV1`。
- **SN-5 (支払いと決済):** 財務調整とレポートの自動化に関する `RevenueShareRecordV1` の承認。

SNS の `roadmap.md` および `status.md` を参照して、レジストラの実際のロードマップを確認してください。