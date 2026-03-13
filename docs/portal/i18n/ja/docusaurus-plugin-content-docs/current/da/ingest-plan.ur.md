---
lang: ja
direction: ltr
source: docs/portal/docs/da/ingest-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note メモ
ہونے تک دونوں ورژنز کو 同期 رکھیں۔
:::

# Sora Nexus データ可用性取り込み計画

_重要: 2026-02-20 -- 重要: コア プロトコル WG / ストレージ チーム / DA WG_

DA-2 処理 Torii 処理 BLOB 取り込み API 処理 Norito 処理
جاری کرتی ہے اور SoraFS ریپلیکیشن کو シード کرتی ہے۔スキーマ
API サーフェスの検証フローと実装のプロセス
シミュレーション (DA-1 フォローアップ) پر رکیر آگے بڑھے۔ペイロード形式の説明
Norito コーデックSerde/JSON フォールバック ٩ی اجازت نہیں۔

## ああ

- BLOB (Taikai セグメント、レーン サイドカー、ガバナンス アーティファクト) Torii
  決定的に قبول کرنا۔
- BLOB、コーデック パラメーター、消去プロファイル、保存ポリシー、保存ポリシー
  標準的な Norito マニフェスト
- チャンク メタデータ SoraFS ホット ストレージのレプリケーション ジョブ
  エンキュー
- インテント + ポリシー タグを固定する SoraFS レジストリ ガバナンス オブザーバー
  出版する
- 入場の領収書、クライアントの決定的な証拠
  出版物

## API サーフェス (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

ペイロード Norito でエンコードされた `DaIngestRequest`応答 `application/norito+v1`
ستعمال کرتی ہیں اور `DaIngestReceipt` واپس کرتی ہیں۔

|応答 |意味 |
| --- | --- |
| 202 承認済み | BLOB チャンク/レプリケーション キュー キュー領収書|
| 400 不正なリクエスト |スキーマ/サイズ違反 (検証チェック)۔ |
| 401 不正 | API トークン موجود نہیں/غلط۔ |
| 409 紛争 | `client_blob_id` ڈپلیکیٹ ہے اور メタデータ مختلف ہے۔ |
| 413 ペイロードが大きすぎます |構成された BLOB の長さの制限|
| 429 リクエストが多すぎます |レート制限に達しました|
| 500 内部エラー |失敗 (ログ + アラート) |

## 提案された Norito スキーマ

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

> 実装メモ: ペイロードと標準的な Rust 表現
> `iroha_data_model::da::types` ٩ے تحت ہیں، リクエスト/領収書のラッパー
> `iroha_data_model::da::ingest` マニフェスト構造
> `iroha_data_model::da::manifest` میں ہے۔

`compression` フィールド بتاتا ہے کہ callers نے payload کی​​سے تیار کیا۔ Torii
`identity`、`gzip`、`deflate`、`zstd` ハッシュ、チャンキング
オプションのマニフェストはバイトを検証し、解凍します。

### 検証チェックリスト1. リクエスト Norito ヘッダー `DaIngestRequest` が一致することを確認します。
2. `total_size` 正規 (解凍された) ペイロード長
   構成された最大数は失敗しました
3. `chunk_size` アライメントは (2 のべき乗、<= 2 MiB) を強制します。
4. `data_shards + parity_shards` <= グローバル最大パリティ >= 2
5. `retention_policy.required_replica_count` ガバナンスベースラインの尊重
6. 正規ハッシュ署名検証 (署名フィールド)
7. `client_blob_id` 重複は拒否されます ペイロード ハッシュ + メタデータが同一で​​す
8. 説明 `norito_manifest` 説明 スキーマ + ハッシュ チャンク化
   再計算されたマニフェストとの一致ノードマニフェストを生成し、保存します。
9. 構成されたレプリケーション ポリシーは次のように適用します: Torii `RetentionPolicy`
   `torii.da_ingest.replication_policy` 書き換え ( `replication-policy.md`
   ) 事前構築済みマニフェスト、拒否、保持メタデータ
   強制プロファイル سے match نہ کرے۔

### チャンク化とレプリケーションのフロー

1. ペイロード `chunk_size` میں تقسیم کریں، ہر chunk پر BLAKE3 اور Merkle root حساب کریں۔
2. Norito `DaManifestV1` (構造体) チャンクコミットメント (role/group_id)
   消去レイアウト (行/列パリティ数 + `ipa_commitment`)、保持ポリシー
   メタデータとキャプチャの作成
3. 正規マニフェスト バイト `config.da_ingest.manifest_store_dir` キュー キュー
   (Torii `manifest.encoded` ファイル レーン/エポック/シーケンス/チケット/フィンガープリント)
   SoraFS オーケストレーション 取り込み ストレージ チケット 永続化データ リンク
4. `sorafs_car::PinIntent` ガバナンス タグ + ポリシー ピン インテントの公開
5. Norito イベント `DaIngestPublished` はオブザーバー (ライト クライアント、ガバナンス、分析) を発行します
   ありがとうございます
6. `DaIngestReceipt` 呼び出し元 (Torii DA サービス キー、署名済み)
   `Sora-PDP-Commitment` ヘッダーの出力 SDK のコミットメント キャプチャの実行
   領収書 `rent_quote` (Norito `DaRentQuote`) `stripe_layout` شامل ہیں،
   提出者の基本賃料、リザーブシェア、PDP/PoTR ボーナスの期待値、2D
   消去レイアウト ストレージ チケット 資金 コミット پہلے دکھا سکتے ہیں۔

## ストレージ/レジストリの更新

- `sorafs_manifest` `DaManifestV1` 拡張拡張決定論的解析
- レジストリ ストリーム `da.pin_intent` バージョン管理されたペイロード
  マニフェスト ハッシュ + チケット ID 参照番号
- 可観測性パイプライン、取り込みレイテンシ、チャンキング スループット、レプリケーション バックログ
  失敗カウント ٹریک کرنے کیلئے اپ ڈیٹ کریں۔

## テスト戦略- スキーマ検証、署名チェック、重複検出、単体テスト
- Norito エンコードのゴールデン テスト (`DaIngestRequest`、マニフェスト、レシート)
- 統合ハーネスのモック SoraFS + レジストリ チャンク + ピン フローの検証
- プロパティテスト、ランダム消去プロファイル、保持の組み合わせのカバー
- Norito ペイロード ファジング 不正な形式のメタデータ

## CLI および SDK ツール (DA-8)- `iroha app da submit` (CLI エントリポイント) 共有インジェスト ビルダー/パブリッシャー、ラップ、またはラップ
  演算子 Taikai バンドル フロー 任意の BLOB の取り込みٌہ
  `crates/iroha_cli/src/commands/da.rs:1` ペイロード、消去/保持プロファイル、オプション
  メタデータ/マニフェスト ファイル CLI 設定キー 正規 `DaIngestRequest` 記号 ہے۔
  `da_request.{norito,json}` `da_receipt.{norito,json}` を実行します。
  `artifacts/da/submission_<timestamp>/` کے تحت محفوظ کرتے ہیں (`--artifact-dir` によるオーバーライド) تاکہ
  リリース アーティファクトの取り込み میں استعمال ہونے والے 正確な Norito バイト レコード
- デフォルトの `client_blob_id = blake3(payload)` は、`--client-blob-id` をオーバーライドします。
  メタデータ JSON マップ (`--metadata-json`) 事前に生成されたマニフェスト (`--manifest`) を受け入れる
  `--no-submit` (オフライン準備) `--endpoint` (カスタム Torii ホスト)領収書のJSON
  stdout 印刷 ہوتا ہے اور ディスク پر بھی لکھا جاتا ہے، جس سے DA-8 کا "submit_blob" 要件 پورا ہوتا ہے
  SDK パリティ作業のブロック解除 ہوتا ہے۔
- `iroha app da get` DA に焦点を当てたエイリアス فراہم کرتا ہے جو マルチソース オーケストレーターを使用する کو جو پہلے ہی
  `iroha app sorafs fetch` ٩و چلاتا ہے۔オペレーター マニフェスト + チャンク プラン アーティファクト (`--manifest`、`--plan`、`--manifest-id`)
  **یا** `--storage-ticket` 経由の Torii ストレージ チケットチケット パス CLI `/v2/da/manifests/<ticket>` 番号
  マニフェストのダウンロード کرتی ہے، バンドル کو `artifacts/da/fetch_<timestamp>/` میں محفوظ کرتی ہے (経由で上書き)
  `--manifest-cache-dir`)`--manifest-id` BLOB ハッシュの導出 `--gateway-provider` リスト
  オーケストレーターの実行 ٩رتی ہے۔ SoraFS フェッチャーの高度なノブ (マニフェスト エンベロープ、
  クライアント ラベル、ガード キャッシュ、匿名トランスポート オーバーライド、スコアボード エクスポート (`--output` パス)
  `--manifest-endpoint` マニフェスト エンドポイント オーバーライド エンドツーエンドの可用性チェック
  مکمل طور پر `da` 名前空間 میں رہتے ہیں بغیر オーケストレーター ロジックの重複 کئے۔
- `iroha app da get-blob` Torii سے `GET /v2/da/manifests/{storage_ticket}` کے ذریعے 正規マニフェスト کھینچتا ہے۔
  `manifest_{ticket}.norito`、`manifest_{ticket}.json`、`chunk_plan_{ticket}.json`
  `artifacts/da/fetch_<timestamp>/` میں لکھتی ہے (ユーザー指定 `--output-dir`) اور عین `iroha app da get`
  呼び出し (`--manifest-id`) エコー کرتی ہے جو フォローアップ オーケストレーターのフェッチ کیلئے درکار ہے۔演算子
  マニフェスト スプール ディレクトリ سے دور رہتے ہیں اور fetcher ہمیشہ Torii کے 署名済みアーティファクト استعمال کرتا ہے۔
  JavaScript Torii クライアント یہی フロー `ToriiClient.getDaManifest(storageTicketHex)` کے ذریعے دیتا ہے، اور デコード
  Norito バイトマニフェスト JSON チャンク プラン、SDK 呼び出し元 CLI、オーケストレーター セッション ハイドレート
  ありがとうございますSwift SDK の表面が公開される (`ToriiClient.getDaManifestBundle(...)` اور)
  `fetchDaPayloadViaGateway(...)`) バンドル ネイティブ SoraFS オーケストレーター ラッパー パイプ iOS クライアント
  マニフェストのダウンロード マルチソースのフェッチ 証明のキャプチャ CLI のキャプチャ[IrohaSwift/ソース/IrohaSwift/ToriiClient.swift:240][IrohaSwift/ソース/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` ストレージ サイズ、保持期間、決定的な家賃、インセンティブの内訳
  計算する有効なヘルパー `DaRentPolicyV1` (JSON Norito バイト) 組み込みのデフォルト値
  ポリシー検証 JSON 概要 (`gib`、`months`、ポリシー メタデータ、`DaRentQuote` フィールド) 印刷
  監査人のガバナンス議事録 正確な XOR 料金の引用 アドホック スクリプトの説明JSON ペイロード
  `rent_quote ...` の概要 بھی دیتی ہے تاکہ コンソール ログが読み取り可能になる
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` ٩و `--policy-label "governance ticket #..."` ساتھ
  美しいアーティファクトの保存 政策投票 設定バンドルの引用 保存CLI カスタム ラベルとトリム
  文字列拒否 `policy_source` ダッシュボード 実行可能な文字列और देखें
  `crates/iroha_cli/src/commands/da.rs` (サブコマンド) `docs/source/da/rent_policy.md` (ポリシースキーマ)
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` チェーン番号: ストレージ チケット 正規マニフェスト バンドル
  マルチソース オーケストレーター (`iroha app sorafs fetch`) のダウンロード `--gateway-provider` リスト
  ダウンロードされたペイロード + スコアボード `artifacts/da/prove_availability_<timestamp>/` میں محفوظ کرتا ہے، اور
  PoR ヘルパー (`iroha app da prove`) フェッチされたバイト数 呼び出し数オペレーター オーケストレーター ノブ
  (`--max-peers`、`--scoreboard-out`、マニフェスト エンドポイント オーバーライド) プルーフ サンプラー (`--sample-count`、`--leaf-index`、
  `--sample-seed`) コマンド DA-5/DA-9 の監査とアーティファクトの調整:
  ペイロードのコピー、スコアボードの証拠、JSON 証明の概要