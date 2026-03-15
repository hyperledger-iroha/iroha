---
lang: ja
direction: ltr
source: docs/portal/docs/da/ingest-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note ソースカノニク
リフレテ`docs/source/da/ingest_plan.md`。 Gardez les deux バージョンと同期
:::

# データの取り込みを計画する 可用性 Sora Nexus

_Redige: 2026-02-20 - 担当: コア プロトコル WG / ストレージ チーム / DA WG_

ワークストリーム DA-2 は、Torii の API を使用して BLOB を取り込み、停止します
メタドンニー Norito およびアモルセ ラ レプリケーション SoraFS。 Ceドキュメントキャプチャファイル
スキーマの提案、表面 API および検証のフラックス
ブロックなしで前もってシミュレーションを実行できる実装 (スイビ)
DA-1)。ペイロードのフォーマット DOIVENT ユーティリティ コーデック Norito。オークン
フォールバック Serde/JSON は許可されません。

## 目的

- ブロブボリュームのアクセプター (セグメント Taikai、サイドカーのレーン、アーティファクトのセグメント)
  gouvernance) de maniere determinist (Torii 経由)。
- マニフェストの作成 Norito canoniques derivant le blob、les parametres de
  コーデック、消去プロファイルと保存ポリティクス。
- SoraFS などのストックファイルのチャンクのメタデータを保存します。
  レプリケーションのファイル ファイル ジョブ。
- 出版社のピンの意図 + 政治的レジストリのタグ SoraFS et les
  統治監視員。
- 暴露者は、preuve を遡ってクライアントを入場させる受領書を提出します
  出版の決定。

## サーフェス API (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

ペイロードは `DaIngestRequest` でエンコードされ、Norito でエンコードされます。応答の有用性
`application/norito+v1` および renvoient `DaIngestReceipt`。

|応答 |意味 |
| --- | --- |
| 202 承認済み | BLOB はファイルを流し込みチャンキング/レプリケーションを行います。領収書の使者。 |
| 400 不正なリクエスト |スキーマ/タイユ違反 (検証のチェック)。 |
| 401 不正 |トークン API マンクォント/無効。 |
| 409 紛争 | Doublon `client_blob_id` avec メタデータが同一で​​はありません。 |
| 413 ペイロードが大きすぎます |長いブロブの構成を制限します。 |
| 429 リクエストが多すぎます |レート制限に注意してください。 |
| 500 内部エラー | Echec inattendu (ログ + アラート)。 |

## スキーマ Norito を提案します

```rust
/// Top-level ingest request.
pub struct DaIngestRequest {
    pub client_blob_id: BlobDigest,      // submitter-chosen identifier
    pub lane_id: LaneId,                 // target Nexus lane
    pub epoch: u64,                      // epoch blob belongs to
    pub sequence: u64,                   // monotonic sequence per (lane, epoch)
    pub blob_class: BlobClass,           // TaikaiSegment, GovernanceArtifact, etc.
    pub codec: BlobCodec,                // e.g. "cmaf", "pdf", "norito-batch"
    pub erasure_profile: ErasureProfile, // parity configuration
    pub retention_policy: RetentionPolicy,
    pub chunk_size: u32,                 // bytes (must align with profile)
    pub total_size: u64,
    pub compression: Compression,        // Identity, gzip, deflate, or zstd
    pub norito_manifest: Option<Vec<u8>>, // optional pre-built manifest
    pub payload: Vec<u8>,                 // raw blob data (<= configured limit)
    pub metadata: ExtraMetadata,          // optional key/value metadata map
    pub submitter: PublicKey,             // signing key of caller
    pub signature: Signature,             // canonical signature over request
}

pub enum BlobClass {
    TaikaiSegment,
    NexusLaneSidecar,
    GovernanceArtifact,
    Custom(u16),
}

pub struct ErasureProfile {
    pub data_shards: u16,
    pub parity_shards: u16,
    pub chunk_alignment: u16, // chunks per availability slice
    pub fec_scheme: FecScheme,
}

pub struct RetentionPolicy {
    pub hot_retention_secs: u64,
    pub cold_retention_secs: u64,
    pub required_replicas: u16,
    pub storage_class: StorageClass,
    pub governance_tag: GovernanceTag,
}

pub struct ExtraMetadata {
    pub items: Vec<MetadataEntry>,
}

pub struct MetadataEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub visibility: MetadataVisibility, // public vs governance-only
}

pub enum MetadataVisibility {
    Public,
    GovernanceOnly,
}

pub struct DaIngestReceipt {
    pub client_blob_id: BlobDigest,
    pub lane_id: LaneId,
    pub epoch: u64,
    pub blob_hash: BlobDigest,          // BLAKE3 of raw payload
    pub chunk_root: BlobDigest,         // Merkle root after chunking
    pub manifest_hash: BlobDigest,      // Norito manifest hash
    pub storage_ticket: StorageTicketId,
    pub pdp_commitment: Option<Vec<u8>>,     // Norito-encoded PDP bytes
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,   // total_stripes, shards_per_stripe, row_parity_stripes
    pub queued_at_unix: u64,
    #[norito(default)]
    pub rent_quote: DaRentQuote,        // XOR rent + incentives derived from policy
    pub operator_signature: Signature,
}
```

> 実装に注意してください: ペイロードを注ぐ Rust の表現
> vivent メンテナンス スー `iroha_data_model::da::types`、ラッパーの平均
> `iroha_data_model::da::ingest` などの構造の要求/受領
> マニフェストは `iroha_data_model::da::manifest` です。

ル チャンピオン `compression` は、呼び出し元がファイル ペイロードを準備していないことをコメントで通知します。 Torii
`identity`、`gzip`、`deflate`、および `zstd`、前衛的非準拠ファイルを受け入れます
ハッシュ、チャンキング、マニフェストの検証などのオプション。

### 検証のチェックリスト1. 検証者リクエストのヘッダー Norito は、`DaIngestRequest` に対応します。
2. Echouer si `total_size` ペイロードの長さの違い
   (解凍) 最大設定を解除します。
3. `chunk_size` の位置合わせを強制します (二重の出力、<= 2 MiB)。
4. 保証者 `data_shards + parity_shards` <= 最大グローバル等価性 >= 2。
5. `retention_policy.required_replica_count` ベースラインを尊重します
   統治。
6. ハッシュ規範に対する署名の検証 (en exclluant le Champ
   署名）。
7. `client_blob_id` ペイロードとメタデータのハッシュを二重化した拒否者
   息子のアイデンティティ。
8. Quand `norito_manifest` est fourni、検証者クエリスキーマ + ハッシュ対応者
   マニフェストはチャンク化の前に再計算します。シノン le noeud ジェネレ le マニフェスト et le
   ストック。
9. アップリケのレプリケーション構成対象者: Torii 記録ファイル
   `RetentionPolicy` 平均的な `torii.da_ingest.replication_policy` (ヴォワール
   `replication-policy.md`) および拒否されたマニフェストは、事前に解釈されません。
   メタデータの保持は、プロファイルの適用に対応する必要があります。

### フラックスのチャンキングとレプリケーション

1. Decouper ファイル ペイロード en `chunk_size`、計算機 BLAKE3 パー チャンク + racine Merkle。
2. Norito `DaManifestV1` (nouvelle struct) 捕捉者レエンゲージメントを構築する
   チャンク (role/group_id)、レイアウトの消去 (リージョンのパリティの計算)
   Colonnes plus `ipa_commitment`)、保持ポリシーとメタデータ。
3. マニフェストのファイルのバイト数を確認する
   `config.da_ingest.manifest_store_dir` (Torii エクリット デ フィシエ)
   `manifest.encoded` レーン/エポック/シーケンス/チケット/フィンガープリントごとのインデックス) afin
   que l'orchestration SoraFS les ingere et relie le storage ticket aux donnees
   持続者。
4. `sorafs_car::PinIntent` による統治上の意図の発行者
   そして政治。
5. Emettre l'evenement Norito `DaIngestPublished` 監視者による通知
   (クライアントの台帳、ガバナンス、分析)。
6. Renvoyer `DaIngestReceipt` au 発信者 (Torii のサービス DA の署名)
   ヘッダー `Sora-PDP-Commitment` は SDK のキャプチャファイルを流し込みます
   コミットメントエンコードの即時。保守者を含む領収書 `rent_quote`
   (un Norito `DaRentQuote`) および `stripe_layout`、永久補助提出者
   ベース、予備、ボーナス PDP/PoTR などの付属書類
   レイアウト 消去 2D 補助コート デュ ストレージ チケット前衛的なもの。

## 1 時間の在庫/登録簿を忘れる

- Etendre `sorafs_manifest` avec `DaManifestV1`、永続的な解析
  決定的な。
- レジストリの新しいストリーム `da.pin_intent` のペイロード バージョンの平均値
  参照ファイルのマニフェストのハッシュ + チケット ID。
- 摂取後の遅延を監視するためのパイプラインの監視、
  チャンク化のスループット、レプリケーションのバックログ、およびコンピューティング
  デチェク。

## テスト戦略- スキーマの検証、署名のチェック、検出を含むテストユニット
  ダブロン。
- Norito および `DaIngestRequest` のエンコーディングのゴールデン検証者、マニフェストなどをテストします
  領収書。
- SoraFS による統合制限のハーネス + レジストリ シミュレーション、検証ファイル
  フラックスデチャンク+ピン。
- 消去と組み合わせのプロファイルに関する所有権テスト
  保持アレアトワール。
- ペイロード Norito のファジングは、メタデータの不正な形式を保護します。

## ツール CLI および SDK (DA-8)- `iroha app da submit` (新しいエントリポイント CLI) エンベロープ メンテナンス ファイル ビルダー
  ブロブを操作する作業の一部を摂取します
  arbitraires hors du flux Taikai バンドル。ラ コマンド ヴィット ダンス
  `crates/iroha_cli/src/commands/da.rs:1` とコンソメ、ペイロード、プロファイル
  メタデータ/マニフェストの事前の消去/保持およびオプションの削除
  署名者ファイル `DaIngestRequest` canonique avec la cle de config CLI。レ・ラン
  レウスシス持続性 `da_request.{norito,json}` および `da_receipt.{norito,json}` sous
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` によるオーバーライド)
  リリース登録ファイル バイト Norito の正確なデータはペンダントを使用します
  摂取してください。
- La commande はデフォルトで `client_blob_id = blake3(payload)` を利用します。
  `--client-blob-id` によるオーバーライド、メタデータの JSON マップの優先
  (`--metadata-json`) およびプレジェネレータのマニフェスト (`--manifest`) およびサポート
  `--no-submit` オフラインでの準備と `--endpoint` ホストの準備
  Torii は個人化されます。領収書のJSON est imprime sur stdout en plus d'etre
  DA-8 などの ecrit sur disque、fermant l'exigence ツール「submit_blob」
  debloquant le travail de parite SDK。
- `iroha app da get` ajoute un alias DA pour l'orchestrateur multi-source qui alimente
  デジャ `iroha app sorafs fetch`。操作者は、ポインタと成果物を操作します
  マニフェスト + チャンク プラン (`--manifest`、`--plan`、`--manifest-id`) **ou** fournir
  `--storage-ticket` 経由のストレージ チケット Torii。 Quand le chemin チケット エスト
  利用、CLI 回復ファイル マニフェスト `/v1/da/manifests/<ticket>`、
  永続ファイル バンドル sous `artifacts/da/fetch_<timestamp>/` (avec をオーバーライド)
  `--manifest-cache-dir`)、`--manifest-id`、ピュイからブロブのハッシュを導出
  l'orchestrateur avec la liste `--gateway-provider` fournie を実行します。トゥレ
  ノブ avances du fetcher SoraFS は無傷のまま保存されます (マニフェスト封筒、ラベル)
  クライアント、ガードキャッシュ、匿名トランスポートのオーバーライド、スコアボードのエクスポートなど
  パス `--output`)、その他のエンドポイント マニフェスト 追加料金の追加
  `--manifest-endpoint` ホストを提供 Torii パーソナライズ、ドングルチェック
  エンドツーエンドの可用性は、名前空間 `da` なしで有効です。
  デュプリケ・ラ・ロジック・ドルケストラトゥール。
- `iroha app da get-blob` マニフェスト canoniques の指示を回復 Torii
  `GET /v1/da/manifests/{storage_ticket}`経由。ラ コマンド エクリット
  `manifest_{ticket}.norito`、`manifest_{ticket}.json` など
  `chunk_plan_{ticket}.json` そうだ `artifacts/da/fetch_<timestamp>/` (おううん
  `--output-dir` fourni par l'utilisateur) 正確なコマンドを使用する
  `iroha app da get` (`--manifest-id` を含む) は、フェッチ オーケストレーターを必要とします。
  セラ・ガルド・レ・オペレーター・オー・デ・レパートリー・スプール・デ・マニフェストと保証
  クエリ フェッチャーは、Toujours les artefacts Signes emis par Torii を利用します。ル・クライアント
  Torii JavaScript 再現フラックス経由
  `ToriiClient.getDaManifest(storageTicketHex)`、renvoyant ファイルバイト Norito
  デコード、マニフェスト JSON および呼び出し元のチャンク プラン SDK ハイドレント
  CLI によるパサーなしのオーケストラのセッションです。 Le SDK Swift の公開
  メンテナント レ ミーム サーフェス (`ToriiClient.getDaManifestBundle(...)` プラス)`fetchDaPayloadViaGateway(...)`)、ラッパーネイティブの分岐バンドル
  オーケストラ SoraFS はクライアント iOS のテレチャージャーを提供します
  マニフェスト、実行者はマルチソースをフェッチし、キャプチャー者はプリューブを排除します
  CLI を呼び出します。
  [IrohaSwift/ソース/IrohaSwift/ToriiClient.swift:240][IrohaSwift/ソース/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` 換気の決定とレンタルの計算
  インセンティブは、ストレージと保持の 4 つの機能を注ぎ込みます。
  L'helper consomme la `DaRentPolicyV1` active (JSON ou bytes Norito) ou le
  デフォルトの統合、有効な政治的および重要な JSON の再開 (`gib`、
  `months`、政治的メタデータ、チャンピオン `DaRentQuote`) 監査に関する情報
  スクリプト広告なしで政府の料金を XOR で正確に計算
  ほら。オーストラリアのコマンドを再開し、ライン `rent_quote ...` の前に進みます
  ペイロード JSON ガーダー レ ログ コンソール ライブラリ ペンダント レ ドリル
  事件。アソシエ `--quote-out artifacts/da/rent_quotes/<stamp>.json` アベック
  `--policy-label "governance ticket #..."` 永続的な成果物を注ぐ
  正確なバンドル設定に投票してください。 la CLI tronque le label パーソナル化
  チェーンを拒否して、価値のあるものを受け取ります `policy_source` 残ります
  ダッシュボードをトレゾレリーで実行できるようになります。ヴォワール
  `crates/iroha_cli/src/commands/da.rs` 副司令官らを注ぐ
  `docs/source/da/rent_policy.md` 政治のスキーマを注ぎます。
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` チェイン tout ce qui precede: il prend un storage
  チケット、マニフェストの一括請求、オーケストラの実行
  マルチソース (`iroha app sorafs fetch`) コントロールリスト `--gateway-provider`
  fournie、persiste le payload telecharge + スコアボード スー
  `artifacts/da/prove_availability_<timestamp>/`、即時ファイルを呼び出す
  ヘルパー PoR が存在します (`iroha app da prove`) バイト数が回復します。オペレータ
  オーケストラのノブを調整する人 (`--max-peers`、`--scoreboard-out`、
  d'endpoint マニフェストをオーバーライドします) およびサンプラーの証明 (`--sample-count`、
  `--leaf-index`、`--sample-seed`) すべてのコマンド製品
  監査に出席するアーティファクト DA-5/DA-9: ペイロードのコピー、証拠
  スコアボードとpreuve JSONの再開。