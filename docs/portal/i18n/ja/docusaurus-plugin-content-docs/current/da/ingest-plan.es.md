---
lang: ja
direction: ltr
source: docs/portal/docs/da/ingest-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ノート フエンテ カノニカ
リフレジャ`docs/source/da/ingest_plan.md`。 Mantenga ambas バージョン en
:::

# Sora Nexus のデータ可用性の取り込み計画

_編集者: 2026-02-20 - 担当: コア プロトコル WG / ストレージ チーム / DA WG_

El ワークストリーム DA-2 拡張 Torii API の取り込みと BLOB の発行
Norito と SoraFS の複製のメタデータ。エステ ドキュメント キャプチャー エル
API の有効性を確認するための保護措置
シミュレーションによるブロックの実装 (セギミエントス)
DA-1)。ペイロードのファイル形式 DEBEN 使用コーデック Norito;許可されません
serde/JSON のフォールバック。

## オブジェクト

- アセプターブロブグランデス（セグメントタイカイ、サイドカーデレーン、アーティファクトデ）
  gobernanza) Torii 経由の形式決定。
- プロデューサー マニフェスト Norito canonicos que describan el blob、parametros de
  コーデック、消去とポリシーの保持。
- SoraFS および encolar ジョブで、大量のメタデータを保持します。
  レプリカ。
- ピンの意図と政治的レジストリのタグの公開 SoraFS y observadores
  デ・ゴベルナンザ。
- 告発者は、クライアントの回復を決定するために承認を与えます
  出版。

## 上位 API (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

ペイロードは `DaIngestRequest` と Norito のコードです。ラス・レスプエスタス・ウーサン
`application/norito+v1` は `DaIngestReceipt` を開発しました。

|レスペスタ |意味 |
| --- | --- |
| 202 承認済み |ブロブ エン コーラ パラ チャンキング/レプリカ。 SE devuelve el recibo。 |
| 400 不正なリクエスト | Violacion de esquema/tamano (ver validaciones)。 |
| 401 不正 |トークン API の有効/無効。 |
| 409 紛争 |メタデータの重複 `client_blob_id` は偶然ではありません。 |
| 413 ペイロードが大きすぎます | BLOB の経度設定の制限を超えています。 |
| 429 リクエストが多すぎます |レート制限を設定します。 |
| 500 内部エラー | Fallo inesperado (ログ + アラータ)。 |

## エスケマ Norito プロプエスト

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

> 実装ノート: Rust パラエストスの正規表現
> ペイロード アホラ ビブン バジョ `iroha_data_model::da::types`、ラッパー デ
> マニフェストの構造の要求/受領書 `iroha_data_model::da::ingest`
> ja `iroha_data_model::da::manifest`。

カンポ `compression` はペイロードの準備をするために発信者を宣言します。 Torii
acepta `identity`, `gzip`, `deflate`, y `zstd`, バイト数の制限を解除
hashear、chunking、verificar はオプションを示します。

### 検証チェックリスト1. リクエストヘッダー Norito と `DaIngestRequest` が一致していることを確認します。
2. `total_size` のペイロードの相違 (デコンプリミド)
   最大限の設定を超えてください。
3. `chunk_size` のアラインアクション (機能、<= 2 MiB)。
4. Asegurar `data_shards + parity_shards` <= maximo グローバル y パリティ >= 2。
5. `retention_policy.required_replica_count` デベ・レスペタル・ラ・リネア・ベース・デ
   ゴベルナンザ。
6. コントラ ハッシュ カノニコの検証 (エンド エル カンポの署名を除く)。
7. Rechazar duplicado `client_blob_id` salvo que el hash del payload y la
   メタデータ ショーン アイデンティコス。
8. Cuando se provee `norito_manifest`、検証エスケマ + ハッシュ一致コンエル
   再計算のチャンク化をマニフェストします。デ・ロ・コントラリオ・エル・ノード・ジェネラ・エル
   アルマセナを明らかにしてください。
9. レプリカ構成の政治政策: Torii 再記述
   `RetentionPolicy` 環境保護 `torii.da_ingest.replication_policy` (バージョン
   `replication-policy.md`) y rechaza マニフェストの事前構築メタデータ
   保持期間は偶然ではありません。

### チャンク化とレプリケーションのフルホス

1. Trocear el ペイロード en `chunk_size`、チャンク + raiz Merkle による計算 BLAKE3。
2. Construir Norito `DaManifestV1` (struct nueva) 侵害のキャプチャ
   チャンク (role/group_id)、レイアウト消去 (フィラデルフィアのコンテオス)
   コラム マス `ipa_commitment`)、政治的保持メタデータ。
3. マニフェスト カノニコ バホのエンコラー ロス バイト
   `config.da_ingest.manifest_store_dir` (Torii アーカイブを記述)
   `manifest.encoded` レーン/エポック/シーケンス/チケット/フィンガープリント) パラケラ
   SoraFS ロス インジェラとビンクル エル ストレージ チケットの保存チケット
   パーシシドス。
4. `sorafs_car::PinIntent` コンタグを介してピンを公開する意図
   政治。
5. イベントを Norito `DaIngestPublished` 監視監視者 (クライアント) に送信します。
   リジェロス、ゴベルナンザ、アナリティカ）。
6. Devolver `DaIngestReceipt` al caller (firmado por la clave de servicio DA de)
   Torii) y 送信ヘッダー `Sora-PDP-Commitment` パラケロス SDK キャプチャ
   媒介コードのコミットメント。エル レシボ アホラ、`rent_quote` を含む
   (un Norito `DaRentQuote`) y `stripe_layout`、継続的な許可
   PDP/PoTR のボーナスのほとんどのレンタル、予約、ボーナスの期待
   レイアウトと消去の 2D ジュント アル ストレージ チケットは、事前に準備しておきます。

## Actualizaciones de almacenamiento/レジストリ

- エクステンダー `sorafs_manifest` コン `DaManifestV1`、ハビリタンド パーセオ
  決定論者。
- ペイロード バージョンに関するレジストリ `da.pin_intent` の新しいストリームの統合
  マニフェストのハッシュ + チケット ID を参照します。
- 摂取後の遅延を監視するパイプラインを実際に実行します。
  チャンク化のスループット、レプリケーションのバックログ、およびフォールスのコンテオ。

## プルエバス戦略- 安全性を検証するための単一のテスト、確実な検出のチェック
  重複者。
- Norito de `DaIngestRequest`、マニフェスト y のゴールデン検証をテストします。
  領収書。
- レバンタンド統合ハーネス SoraFS + レジストリ シミュレーション、検証
  フルホス・デ・チャンク+ピン。
- 消去と組み合わせのファイル管理テスト
  保持アリエトリア。
- 不正なメタデータに対するペイロード Norito のファジング。

## CLI および SDK のツール (DA-8)- `iroha app da submit` (CLI の新しいエントリポイント) ビルダー/パブリッシャーの ahora envuelve
  オペラドール・プエダン・インゲリル・ブロブ任意の摂取コンパルティド
  フエラ デル フルホ タイカイ バンドル。エル・コマンド・ビベ・アン
  `crates/iroha_cli/src/commands/da.rs:1` y はペイロードを消費し、実行します
  メタデータ/マニフェストの事前の消去/保持およびアーカイブ
  `DaIngestRequest` CLI での設定を行うことができます。ラス・エジェクシオネス
  exitosas 持続 `da_request.{norito,json}` y `da_receipt.{norito,json}` バジョ
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` 経由でオーバーライド) パラメータ
  リリースレジストレンのロスアーティファクト、ロスバイト Norito 正確な米国のデュランテラ
  摂取する。
- エル コマンド アメリカ ポル ディフェクト `client_blob_id = blake3(payload)` ペロ アセプタ
  `--client-blob-id` 経由でオーバーライドし、JSON のメタデータを再マッピングします
  (`--metadata-json`) y はジェネラド以前の症状を示します (`--manifest`)、y soporta
  `--no-submit` パラ準備オフラインマス `--endpoint` パラホスト Torii
  個性派。受信用の JSON は標準出力の暗号化に使用されます。
  ディスコ、DA-8 および desbloqueando のツール「submit_blob」を必要とするセルランド
  SDK の取引。
- `iroha app da get` agrega un alias enfocado en DA para el orquestador マルチソース
  ケ・ヤ・ポテンシア`iroha app sorafs fetch`。ロス オペラドーレス プエデン アプンタルロ
  マニフェストのアーティファクト + チャンク プラン (`--manifest`、`--plan`、`--manifest-id`)
  **o** `--storage-ticket` 経由で Torii のストレージ チケットを取得します。クアンド・セ・アメリカ
  チケットのパス、CLI バハのマニフェスト `/v2/da/manifests/<ticket>`、
  永続化エルバンドルバジョ `artifacts/da/fetch_<timestamp>/` (オーバーライドコン
  `--manifest-cache-dir`)、`--manifest-id` の BLOB からの派生、ルエゴ
  `--gateway-provider` suministrada を出してください。トドス
  SoraFS の無傷の無傷のノブのアヴァンザドス デル フェッチャー (マニフェスト)
  エンベロープ、クライアントのラベル、ガード キャッシュ、トランスポート アノニモのオーバーライド、
  スコアボードのエクスポート、パス `--output`)、マニフェストのエンドポイントのエクスポート
  `--manifest-endpoint` パラホスト Torii Personalizados、ASI 経由の sobrescribirse
  que los は、名前空間全体の可用性をエンドツーエンドでチェックします
  `da` 罪の二重論理的オルケスタドール。
- `iroha app da get-blob` baja マニフェスト canonicos directo desde Torii via
  `GET /v2/da/manifests/{storage_ticket}`。エルコマンドエスクライブ
  `manifest_{ticket}.norito`、`manifest_{ticket}.json` y
  `chunk_plan_{ticket}.json` バジョ `artifacts/da/fetch_<timestamp>/` (o un
  `--output-dir` 利用規約) 重要な命令を実行する
  `iroha app da get` (`--manifest-id` を含む) パラメータの取得要求
  オルケスタドール。ロス オペラドーレスの劇場でのマンティエンの演出
  マニフェスト y garantiza que el fetcher siempre use los artefactos farmados
  Emidos por Torii。 JavaScript のクライアント Torii を参照してください。
  `ToriiClient.getDaManifest(storageTicketHex)`、デボルビエンド ロス バイト Norito
  デコディフィカド、SDK 非表示の呼び出し側のマニフェスト JSON とチャンク プラン
  CLI でのセッション。 El SDK de Swift のアホラの説明
  ミスマスのスーパーフィシーズ (`ToriiClient.getDaManifestBundle(...)` マス)`fetchDaPayloadViaGateway(...)`)、canalizando はネイティブ ラッパーをバンドルします
  orquestador SoraFS para que clientes iOS puedan ダウンロードマニフェスト、エジェクター
  CLI の呼び出し元のマルチソースとキャプチャ プルエバスをフェッチします。
  [IrohaSwift/ソース/IrohaSwift/ToriiClient.swift:240][IrohaSwift/ソース/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` 決定とインセンティブの計算
  ストレージと保管を管理するためのストレージを必要とします。エルヘルパー
  消費 el `DaRentPolicyV1` activo (JSON o バイト Norito) またはデフォルトの integrado、
  政治と重要性を検証し、再開する JSON (`gib`、`months`、メタデータ
  政治と選挙 `DaRentQuote`) 視聴者と貨物 XOR の比較
  アドホックな罪スクリプトの実行。エル・コマンド・タンビエン・エミット・アン・レジューメン
  en una linea `rent_quote ...` antes del payload JSON para mantener readings los
  事故の継続的な訓練のログ。エンパレヘ
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` コン
  `--policy-label "governance ticket #..."` パラ パーシスティアー アーティファクト カツオク
  正確な設定をバンドルするための情報。 CLI レコルタ エル ラベル パーソナライズド
  y rechaza 文字列 vacios para que los valores `policy_source` se mantengan
  tesoreria のダッシュボードのアクショナブル。バージョン
  `crates/iroha_cli/src/commands/da.rs` パラメータ サブコマンド
  `docs/source/da/rent_policy.md` 政治的エスケーマ。
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` encadena todo lo anterior: toma un storage
  チケット、マニフェストの一括ダウンロード、オルケスタドールの排出
  マルチソース (`iroha app sorafs fetch`) 逆リスト `--gateway-provider`
  suministrada、persiste el ペイロード デカルガード + スコアボード バジョ
  `artifacts/da/prove_availability_<timestamp>/`、メディアヘルパーの呼び出し
  PoR 存在 (`iroha app da prove`) はバイト トレイドを失います。ロス オペラドーレス
  プエデン アジャスター ロス ノブ デル オルケスタドール (`--max-peers`、`--scoreboard-out`、
  エンドポイントおよびマニフェストをオーバーライドします) およびサンプラーの証明 (`--sample-count`、
  `--leaf-index`, `--sample-seed`) ミエントラ アン ソロ コマンド プロデュース ロス
  DA-5/DA-9 の聴覚による芸術作品: ペイロードのコピア、証拠
  スコアボードとプルエバの履歴書 JSON。