---
lang: ja
direction: ltr
source: docs/portal/docs/sns/registry-schema.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note フォンテ カノニカ
Esta pagina espelha `docs/source/sns/registry_schema.md` e アゴラは、ポータルを共有して提供します。永久に取引を続けてください。
:::

# Sora ネームサービス (SN-2a) を登録します。

**ステータス:** Redigido 2026-03-24 -- SNS プログラムのサブメティド  
**ロードマップへのリンク:** SN-2a「レジストリ スキーマとストレージ レイアウト」  
**エスコポ:** 定義は、Norito canonicas、システム デ ビダ、OS イベント、エミッション パラオ、Sora ネーム サービス (SNS) の実装として、レジストリおよびレジストラの決定性、SDK、ゲートウェイとして定義されています。

SN-2a の詳細に関する文書の完全性:

1. ハッシュの記録の識別 (`SuffixId`、`NameHash`、デリバチャオ デ セレトレ)。
2. 構造体/列挙型 Norito は、名前、政治、政治、プレコ、受信、イベントの登録に使用されます。
3. 決定的な再生時のインデックスのプレフィックスとプレフィックスのレイアウト。
4. Uma maquina de estados cobrindo registro、renovacao、grace/redemption、frees e tombstone。
5. イベント キャノニコス コンスミドス ペラ オートマカオ DNS/ゲートウェイ。

## 1. ID のハッシュ化

|識別子 |説明 |デリバカオ |
|-----------|---------------|-----------|
| `SuffixId` (`u16`) |優れた接尾辞を登録する識別子 (`.sora`、`.nexus`、`.dao`)。 Alinhado 社のサフィックス カタログ [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md)。 |統治者による投票;アルマゼナド em `SuffixPolicyV1`。 |
| `SuffixSelector` | canonica em string do sufixo (ASCII、小文字) を形式化します。 |例: `.sora` -> `sora`。 |
| `NameSelectorV1` | Seletor binario para o rotulo registrado. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`。 NFC + 小文字のセグンド ノルム v1 を使用します。 |
| `NameHash` (`[u8;32]`) |コントラトス、イベント、キャッシュなどの主要なコンテンツを保存します。 | `blake3(NameSelectorV1_bytes)`。 |

決定性の要件:

- Norm v1 (UTS-46 strict、STD3 ASCII、NFC) 経由で正常に動作します。文字列として DEVEM を正規化し、ハッシュを実行します。
- Rotulos reservados (`SuffixPolicyV1.reserved_labels`) 登録なし。統治機構の発行イベントイベント `ReservedNameAssigned` をオーバーライドします。

## 2. 発情期 Norito

### 2.1 名前レコード V1

|カンポ |ティポ |メモ |
|------|------|------|
| `suffix_id` | `u16` |参照番号 `SuffixPolicyV1`。 |
| `selector` | `NameSelectorV1` |バイトは、オーディオ/デバッグ用のセレクター ブルートを実行します。 |
| `name_hash` | `[u8; 32]` |パラマップ/イベントをチャベします。 |
| `normalized_label` | `AsciiString` | Rotulo Legivel por humanos (ポスト Norm v1)。 |
| `display_label` | `AsciiString` |フォルネシド・ペロ・スチュワードのケーシング。コスメティアオプション。 |
| `owner` | `AccountId` | Controla renovacoes/transferencias。 |
| `controllers` | `Vec<NameControllerV1>` |参照はすべての内容を参照し、リゾルバーまたはメタデータを適用します。 |
| `status` | `NameStatus` | Indicador de ciclo de vida (ver Secao 4)。 |
| `pricing_class` | `u8` |プレコの階層のインデックス (標準、プレミアム、予約済み)。 |
| `registered_at` | `Timestamp` |初期のブロコのタイムスタンプ。 |
| `expires_at` | `Timestamp` |フィム・ド・テルモ・パゴ。 |
| `grace_expires_at` | `Timestamp` |自動改修を行います (デフォルトは +30 ディアス)。 |
| `redemption_expires_at` | `Timestamp` |償還のフィム・ダ・ジャネラ（デフォルトは+60ディアス）。 |
| `auction` | `Option<NameAuctionStateV1>` | Presente quando ha オランダは、プレミアム アクティビティを再開します。 |
| `last_tx_hash` | `Hash` |ポンテイロは、取引先の決定を決定します。 |
| `metadata` | `Metadata` |メタデータはレジストラによって任意に管理されます (テキスト記録、証明)。 |

サポートする構造体:

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
| `suffix_id` | `u16` |チャーベ・プライマリア。政治の中心。 |
| `suffix` | `AsciiString` |たとえば、`sora`。 |
| `steward` | `AccountId` |スチュワードは統治憲章を定めていない。 |
| `status` | `SuffixStatus` | `Active`、`Paused`、`Revoked`。 |
| `payment_asset_id` | `AsciiString` | Padrao による決済の ID (例 `61CtjvNd9T3THAR65GsMVHr82Bjc`)。 |
| `pricing` | `Vec<PriceTierV1>` |デュラソーの事前管理係数。 |
| `min_term_years` | `u8` |層をオーバーライドするための独立した要素を相互に比較します。 |
| `grace_period_days` | `u16` |デフォルトは 30。
| `redemption_period_days` | `u16` |デフォルトは 60。
| `max_term_years` | `u8` | Maximo de renovacao antecipada。 |
| `referral_cap_bps` | `u16` | <=1000 (10%) セグンド・オ・チャーター。 |
| `reserved_labels` | `Vec<ReservedNameV1>` | Lista fornecida pela Governmenta com instrucoes de atribuicao。 |
| `fee_split` | `SuffixFeeSplitV1` | Partes tesouraria / スチュワード / 紹介 (基本ポイント)。 |
| `fund_splitter_account` | `AccountId` | Conta que mantem エスクロー + 配布資金。 |
| `policy_version` | `u16` |ムダンカを増やしていきます。 |
| `metadata` | `Metadata` |重要事項 (KPI 規約、ドキュメントのコンプライアンスのハッシュ)。 |

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

### 2.3 和解の受領記録

|構造体 |カンポス |プロポジト |
|----------|----------|----------|
| `RevenueShareRecordV1` | `suffix_id`、`epoch_id`、`treasury_amount`、`steward_amount`、`referral_amount`、`escrow_amount`、`settled_at`、`tx_hash`。 |和解の時期を決定する登録 (セマナル)。 |
| `RevenueAccrualEventV1` | `name_hash`、`suffix_id`、`event`、`gross_amount`、`net_amount`、`referral_account`。 | Emitido cada vez que um pagamento e postado (登録、レノバカオ、レイラオ)。 |

`TokenValue` は、Norito のコードを修正し、`SuffixPolicyV1` 関連付けを宣言しません。

### 2.4 登録イベント

DNS/ゲートウェイと分析を自動化するためのログ再生イベント。

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

OS イベント開発サーバーおよびログ再現 (例: ドミニオ `RegistryEvents`) は、ゲートウェイ パラメータのフィードを参照し、キャッシュ DNS を無効にし、SLA を実行します。

## 3. インデックスの配置

|チャベ |説明 |
|-----|---------------|
| `Names::<name_hash>` | `name_hash` と `NameRecordV1` を比較します。 |
| `NamesByOwner::<AccountId, suffix_id>` | UI de ウォレットのインデックス (paginacao amigavel)。 |
| `NamesByLabel::<suffix_id, normalized_label>` | Detecta conflitos、habilita Busca deterministica。 |
| `SuffixPolicies::<suffix_id>` |ウルティモ `SuffixPolicyV1`。 |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` の歴史。 |
| `RegistryEvents::<u64>` |ログ追加専用の com Chave de sequencia monotona。 |

Todas はホストのシリアル化を使用して Norito パラメータのハッシュ決定を行います。初期の登録情報を基に、正しい形式のインデックスが作成されます。

## 4. マキナ・デ・エスタドス・ド・シクロ・デ・ヴィーダ

|エスタード |コンディコス デ エントラーダ |トランシコエス許可 |メモ |
|------|----------------------|-----------|------|
|利用可能 |デリバド クアンド `NameRecord` エスタ アウセンテ。 | `PendingAuction` (プレミアム)、`Active` (レジストロ標準)。 |ディスポニビリダーデ ル アペナスのインデックスを参照。 |
|保留中のオークション | Criado quando `PriceTierV1.auction_kind` != なし。 | `Active` (レイラオ リキッドダ)、`Tombstoned` (セミランス)。 |レイロは `AuctionOpened` と `AuctionSettled` を発します。 |
|アクティブ |レジストリを更新してください。 | `GracePeriod`、`Frozen`、`Tombstoned`。 | `expires_at` ギア ア トランジソン。 |
|猶予期間 |オートマチックカンド `now > expires_at`。 | `Active` (レノバカオエムディア)、`Redemption`、`Tombstoned`。 |デフォルト +30 dia;私はマス・シナリザドを解決します。 |
|償還 | `now > grace_expires_at` マス `< redemption_expires_at`。 | `Active` (レノバカオ タルディア)、`Tombstoned`。 |刑罰分類群のコマンド。 |
|冷凍 |フリーズ・ド・ガバナンカ・オ・ガーディアン。 | `Active` (アポス レメディアカオ)、`Tombstoned`。 | Nao はコントローラを転送します。 |
|墓石 |自発的な放棄、永久的な紛争の結果、償還の期限切れ。 | `PendingAuction` (オランダの再開) 永久に墓石にされています。 | O イベント `NameTombstoned` はラザオを含みます。 |DEVEM エミッターの転送として、`RegistryEventKind` はダウンストリーム コエンテスをキャッシュします。 Nomes tombstoned que entram em leiloes オランダの再開試験 um ペイロード `AuctionKind::DutchReopen`。

## 5. ゲートウェイのイベントと同期

ゲートウェイ assinam `RegistryEventV1` および同期 DNS/SoraFS ao:

1. 究極のイベント `NameRecordV1` 参照。
2. Regenerar テンプレート デ リゾルバー (enderecos i105 優先 + 圧縮 (`sora`) como segunda opcao、テキスト レコード)。
3. Fluxo SoraDNS の説明 [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md) を介して、真の状態を示します。

イベントの開催日:

- `NameRecordV1` *deve* は、イベント com `version` estritamente での取引を実行します。
- Eventos `RevenueSharePosted` は、`RevenueShareRecordV1` の液体放出を参照します。
- 凍結/凍結解除/トゥームストーンのイベントには、`metadata` パラリプレイ デ オーディオの政府機関の芸術作品のハッシュが含まれます。

## 6. ペイロードの例 Norito

### 6.1 NameRecord の例

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

### 6.2 SuffixPolicy の例

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

## 7. プロキシモス・パソス

- **SN-2b (レジストラー API およびガバナンス フック):** Torii (バインディング Norito e JSON) 経由で構造体をエクスポートし、ガバナンスの技術的な入場許可をチェックします。
- **SN-3 (オークションおよび登録エンジン):** コミット/公開ロジックの再利用 `NameAuctionStateV1` およびダッチ オープン。
- **SN-5 (支払いと決済):** `RevenueShareRecordV1` は金融と自動関係の調整に使用されます。

ロードマップ SNS em `roadmap.md` と refletidas em `status.md` を統合するための準備として、ムダンカの開発者登録申請を行ってください。