---
lang: ja
direction: ltr
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T15:38:30.661072+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus データ可用性取り込み計画

_起草: 2026-02-20 - 所有者: コア プロトコル WG / ストレージ チーム / DA WG_

DA-2 ワークストリームは、Norito を発行する BLOB 取り込み API を使用して Torii を拡張します。
メタデータとシード SoraFS レプリケーション。この文書には、提案された内容が記載されています
スキーマ、API サーフェイス、および検証フローを必要とせずに実装を進めることができる
未処理のシミュレーションのブロック (DA-1 フォローアップ)。すべてのペイロード形式は必須です
Norito コーデックを使用します。 Serde/JSON フォールバックは許可されません。

## 目標

- 大きな BLOB (Taikai セグメント、レーン サイドカー、ガバナンス アーティファクト) を受け入れる
  決定的に Torii を超えます。
- BLOB、コーデック パラメータ、
  消去プロファイルと保持ポリシー。
- SoraFS ホット ストレージにチャンク メタデータを保持し、レプリケーション ジョブをキューに入れます。
- ピン インテント + ポリシー タグを SoraFS レジストリとガバナンスに公開します。
  観察者たち。
- クライアントが出版の決定的な証拠を取り戻すために、入場受領書を公開します。

## API サーフェス (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

ペイロードは、Norito でエンコードされた `DaIngestRequest` です。応答の使用
`application/norito+v1` と `DaIngestReceipt` を返します。

|応答 |意味 |
| --- | --- |
| 202 承認済み |チャンク/レプリケーションのためにキューに入れられた BLOB。レシートが返ってきた。 |
| 400 不正なリクエスト |スキーマ/サイズ違反 (検証チェックを参照)。 |
| 401 不正 | API トークンが欠落しているか無効です。 |
| 409 紛争 |メタデータが一致しない `client_blob_id` が重複しています。 |
| 413 ペイロードが大きすぎます |構成された BLOB の長さ制限を超えています。 |
| 429 リクエストが多すぎます |レート制限に達しました。 |
| 500 内部エラー |予期しない障害 (ログに記録 + アラート)。 |

```
GET /v2/da/proof_policies
Accept: application/json | application/x-norito
```

現在のレーン カタログから派生したバージョン付き `DaProofPolicyBundle` を返します。
バンドルは、`version` (現在は `1`)、`policy_hash` (
順序付けされたポリシー リスト)、および `lane_id`、`dataspace_id`、を含む `policies` エントリ
`alias`、および強制された `proof_scheme` (現在は `merkle_sha256`、KZG レーンは
KZG コミットメントが利用可能になるまで、取り込みによって拒否されます)。現在のブロックヘッダー
`da_proof_policies_hash` 経由でバンドルにコミットするため、クライアントは
DA コミットメントまたはプルーフを検証するときに設定されるアクティブなポリシー。このエンドポイントを取得します
プルーフを作成する前に、レーンのポリシーと現在のポリシーが一致していることを確認します。
バンドルハッシュ。コミットメント リスト/証明エンドポイントは同じバンドルを運ぶため、SDK
プルーフをアクティブなポリシー セットにバインドするために追加のラウンドトリップを行う必要はありません。

```
GET /v2/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

順序付けられたポリシー リストと、
`policy_hash` なので、SDK はブロックの生成時に使用されたバージョンを固定できます。の
ハッシュは、Norito でエンコードされたポリシー配列に対して計算され、変更されるたびに変更されます。
レーンの `proof_scheme` が更新され、クライアントがレーン間のドリフトを検出できるようになりました。
キャッシュされたプルーフとチェーン構成。

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
```> 実装メモ: これらのペイロードの正規の Rust 表現は現在、以下に存在します。
> `iroha_data_model::da::types`、`iroha_data_model::da::ingest` のリクエスト/受信ラッパーあり
> および `iroha_data_model::da::manifest` のマニフェスト構造。

`compression` フィールドは、呼び出し側がペイロードをどのように準備したかをアドバタイズします。 Torii は受け入れます
`identity`、`gzip`、`deflate`、および `zstd`、前のバイトを透過的に解凍します。
オプションのマニフェストのハッシュ化、チャンク化、および検証。

### 検証チェックリスト

1. リクエスト Norito ヘッダーが `DaIngestRequest` と一致することを確認します。
2. `total_size` が標準 (解凍された) ペイロード長と異なる場合、または設定された最大ペイロード長を超えている場合は失敗します。
3. `chunk_size` アライメントを強制します (2 のべき乗、= 2 であることを確認します。
5. `retention_policy.required_replica_count` はガバナンスのベースラインを尊重しなければなりません。
6. 正規ハッシュに対する署名検証 (署名フィールドを除く)。
7. ペイロード ハッシュとメタデータが同一で​​ない限り、重複する `client_blob_id` を拒否します。
8. `norito_manifest` が指定された場合、スキーマ + ハッシュの一致が再計算されていることを確認します。
   チャンク化後にマニフェストします。それ以外の場合、ノードはマニフェストを生成して保存します。
9. 構成されたレプリケーション ポリシーを適用します。Torii は、送信されたポリシーを書き換えます。
   `RetentionPolicy` と `torii.da_ingest.replication_policy` (を参照)
   `replication_policy.md`) を保持し、事前に構築されたマニフェストを拒否します。
   メタデータが適用されたプロファイルと一致しません。

### チャンク化とレプリケーションのフロー1. ペイロードを `chunk_size` にチャンクし、チャンクごとの BLAKE3 + マークル ルートを計算します。
2. チャンク コミットメント (role/group_id) をキャプチャする Norito `DaManifestV1` (新しい構造体) をビルドします。
   消去レイアウト (行と列のパリティ数と `ipa_commitment`)、保持ポリシー、
   そしてメタデータ。
3. 正規マニフェスト バイトを `config.da_ingest.manifest_store_dir` の下にキューに入れます。
   (Torii は、レーン/エポック/シーケンス/チケット/フィンガープリントをキーとする `manifest.encoded` ファイルを書き込みます) したがって、SoraFS
   オーケストレーションはそれらを取り込み、ストレージ チケットを永続化データにリンクできます。
4. ガバナンス タグ + ポリシーを使用して、`sorafs_car::PinIntent` 経由でピン インテントを公開します。
5. Norito イベント `DaIngestPublished` を発行して、オブザーバー (ライト クライアント、
   ガバナンス、分析）。
6. `DaIngestReceipt` (Torii DA サービス キーで署名) を返し、
   Base64 Norito エンコードを含む `Sora-PDP-Commitment` 応答ヘッダー
   SDK がサンプリング シードをすぐに隠しておくことができるように、派生コミットメントの。
   レシートには `rent_quote` (`DaRentQuote`) と `stripe_layout` が埋め込まれます。
   そのため、提出者は XOR 義務、リザーブシェア、PDP/PoTR ボーナスの期待、
   資金をコミットする前に、ストレージ チケットのメタデータとともに 2D 消去マトリックスのディメンションを確認します。
7. オプションのレジストリ メタデータ:
   - `da.registry.alias` — PIN レジストリ エントリをシードするための公開の暗号化されていない UTF-8 エイリアス文字列。
   - `da.registry.owner` — レジストリの所有権を記録する公開の暗号化されていない `AccountId` 文字列。
   Torii はこれらを生成された `DaPinIntent` にコピーするため、ダウンストリームのピン処理でエイリアスをバインドできるようになります。
   生のメタデータ マップを再解析することなく所有者に提供します。不正な形式または空の値は、実行中に拒否されます。
   取り込み検証。

## ストレージ/レジストリの更新

- `sorafs_manifest` を `DaManifestV1` で拡張し、確定的な解析を有効にします。
- バージョン管理されたペイロード参照を含む新しいレジストリ ストリーム `da.pin_intent` を追加します。
  マニフェスト ハッシュ + チケット ID。
- 可観測性パイプラインを更新して、取り込みレイテンシ、チャンキング スループット、
  レプリケーションのバックログと失敗数。
- Torii `/status` 応答には、最新の応答を表示する `taikai_ingest` 配列が含まれるようになりました。
  エンコーダから取り込みまでのレイテンシー、ライブエッジ ドリフト、および (クラスター、ストリーム) ごとのエラー カウンター、DA-9 の有効化
  Prometheus をスクレイピングせずに、ノードからヘルス スナップショットを直接取り込むためのダッシュボード。

## テスト戦略- スキーマ検証、署名チェック、重複検出のための単体テスト。
- `DaIngestRequest` の Norito エンコード、マニフェスト、およびレシートを検証するゴールデン テスト。
- 統合ハーネスがモック SoraFS + レジストリをスピンアップし、チャンク + ピン フローをアサートします。
- ランダム消去プロファイルと保持の組み合わせをカバーする特性テスト。
- 不正なメタデータを防ぐための Norito ペイロードのファジング。
- すべての BLOB クラスのゴールデン フィクスチャが存在します
  `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` コンパニオンチャンクあり
  `fixtures/da/ingest/sample_chunk_records.txt` にリストされています。無視されたテスト
  `regenerate_da_ingest_fixtures` はフィクスチャを更新しますが、
  新しい `BlobClass` バリアントが追加されるとすぐに、`manifest_fixtures_cover_all_blob_classes` が失敗する
  Norito/JSON バンドルを更新せずに。これにより、DA-2 では常に Torii、SDK、およびドキュメントが正直に保たれます
  新しいブロブ サーフェスを受け入れます。【fixtures/da/ingest/README.md:1】【crates/iroha_torii/src/da/tests.rs:2902】

## CLI および SDK ツール (DA-8)- `iroha app da submit` (新しい CLI エントリポイント) は、共有取り込みビルダー/パブリッシャーをラップするようになりました。
  Taikai バンドル フローの外側の任意の BLOB を取り込むことができます。コマンドは次の場所にあります
  `crates/iroha_cli/src/commands/da.rs:1` は、ペイロード、消去/保持プロファイル、および
  CLI を使用して正規の `DaIngestRequest` に署名する前の、オプションのメタデータ/マニフェスト ファイル
  設定キー。実行が成功すると、`da_request.{norito,json}` および `da_receipt.{norito,json}` が維持されます。
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` 経由でオーバーライド) なので、リリース アーティファクトは
  取り込み中に使用された正確な Norito バイトを記録します。
- コマンドのデフォルトは `client_blob_id = blake3(payload)` ですが、次によるオーバーライドを受け入れます。
  `--client-blob-id`、メタデータ JSON マップ (`--metadata-json`) および事前生成されたマニフェストを尊重します
  (`--manifest`)、オフライン準備用の `--no-submit` とカスタム用の `--endpoint` をサポートします
  Torii ホスト。受信 JSON はディスクに書き込まれるだけでなく、stdout にも出力され、
  DA-8「submit_blob」ツール要件と SDK パリティ作業のブロック解除。
- `iroha app da get` は、すでに機能しているマルチソース オーケストレーターの DA に焦点を当てたエイリアスを追加します。
  `iroha app sorafs fetch`。オペレーターは、マニフェスト + チャンクプラン アーティファクトをポイントできます (`--manifest`、
  `--plan`、`--manifest-id`) **または**、単に `--storage-ticket` 経由で Torii ストレージ チケットを渡します。とき
  チケット パスが使用され、CLI が `/v2/da/manifests/<ticket>` からマニフェストをプルし、バンドルを永続化します。
  `artifacts/da/fetch_<timestamp>/` (`--manifest-cache-dir` でオーバーライド) の下で、**マニフェストを派生します。
  `--manifest-id` の hash** を指定し、提供された `--gateway-provider` を使用してオーケストレーターを実行します。
  リスト。ペイロード検証は、ゲートウェイ ID が設定されている間も、埋め込まれた CAR/`blob_hash` ダイジェストに依存します。
  現在はマニフェスト ハッシュなので、クライアントとバリデーターは単一の BLOB 識別子を共有します。すべての高度なノブ
  SoraFS フェッチャー サーフェスはそのままです (マニフェスト エンベロープ、クライアント ラベル、ガード キャッシュ、匿名トランスポート)
  オーバーライド、スコアボード エクスポート、および `--output` パス)、マニフェスト エンドポイントは次の方法でオーバーライドできます。
  カスタム Torii ホストの場合は `--manifest-endpoint` であるため、エンドツーエンドの可用性チェックは完全に
  オーケストレーター ロジックを複製しない `da` 名前空間。
- `iroha app da get-blob` は、`GET /v2/da/manifests/{storage_ticket}` を介して Torii から正規マニフェストを直接取得します。
  このコマンドは、マニフェスト ハッシュ (BLOB ID) を使用してアーティファクトにラベルを付けるようになりました。
  `manifest_{manifest_hash}.norito`、`manifest_{manifest_hash}.json`、および `chunk_plan_{manifest_hash}.json`
  `artifacts/da/fetch_<timestamp>/` (またはユーザー指定の `--output-dir`) の下で、正確な内容をエコーします。
  フォローアップ オーケストレーターのフェッチには `iroha app da get` 呼び出し (`--manifest-id` を含む) が必要です。
  これにより、オペレーターがマニフェストのスプール ディレクトリにアクセスできなくなり、フェッチャーが常に
  Torii によって発行された署名付きアーティファクト。 JavaScript Torii クライアントは、このフローを次のようにミラーリングします。
  `ToriiClient.getDaManifest(storageTicketHex)` 一方、Swift SDK は現在公開しています
  `ToriiClient.getDaManifestBundle(...)`。どちらもデコードされた Norito バイト、マニフェスト JSON、マニフェスト ハッシュを返します。チャンク プランにより、SDK 呼び出し元は CLI や Swift にシェルアウトせずにオーケストレーター セッションをハイドレートできます。
  クライアントはさらに `fetchDaPayloadViaGateway(...)` を呼び出して、ネイティブ経由でこれらのバンドルをパイプすることもできます。
  SoraFS オーケストレーター ラッパー。【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】
- `/v2/da/manifests` 応答は、`manifest_hash` と両方の CLI + SDK ヘルパー (`iroha app da get`、
  `ToriiClient.fetchDaPayloadViaGateway`、および Swift/JS ゲートウェイ ラッパー) は、このダイジェストを
  埋め込まれた CAR/BLOB ハッシュに対してペイロードの検証を継続しながら、正規マニフェスト識別子を使用します。
- `iroha app da rent-quote` は、供給されたストレージ サイズに対する決定的な家賃とインセンティブの内訳を計算します
  そして保持期間。ヘルパーは、アクティブな `DaRentPolicyV1` (JSON または Norito バイト) または
  組み込みのデフォルト、ポリシーを検証し、JSON 概要 (`gib`、`months`、ポリシー メタデータ、
  `DaRentQuote` フィールド）により、監査人はガバナンス議事録内で正確な XOR 料金を引用することができます。
  アドホックスクリプトを作成します。このコマンドは、JSON の前に 1 行の `rent_quote ...` 概要も出力するようになりました。
  ペイロードを使用すると、インシデント中に見積もりが生成されるときに、コンソール ログとランブックを簡単にスキャンできるようになります。
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` (またはその他のパス) を渡します。
  きれいに印刷された概要を永続化し、次の場合に `--policy-label "governance ticket #..."` を使用します。
  アーティファクトは特定の投票/構成バンドルを引用する必要があります。 CLI はカスタム ラベルをトリミングし、空白を拒否します
  証拠バンドル内で `policy_source` 値を意味のあるものに保つための文字列。参照
  サブコマンドの場合は `crates/iroha_cli/src/commands/da.rs`、`docs/source/da/rent_policy.md`
  ポリシースキーマの場合。【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- ピン レジストリ パリティが SDK に拡張されるようになりました: `ToriiClient.registerSorafsPinManifest(...)`
  JavaScript SDK は、`iroha app sorafs pin register` によって使用される正確なペイロードを構築し、正規化を適用します
  POST 前のチャンカー メタデータ、ピン ポリシー、エイリアス プルーフ、および後続ダイジェスト
  `/v2/sorafs/pin/register`。これにより、CI ボットとオートメーションが CLI にシェルアウトすることがなくなります。
  マニフェストの登録を記録し、ヘルパーには TypeScript/README が同梱されているため、DA-8 の
  「submit/get/prove」ツールの同等性は、Rust/Swift と並んで JS 上で完全に満たされています。【javascript/iroha_js/src/toriiClient.js:1045】【javascript/iroha_js/test/toriiClient.test.js:788】
- `iroha app da prove-availability` は上記すべてをチェーンします。ストレージ チケットを取得し、
  正規マニフェスト バンドルは、マルチソース オーケストレーター (`iroha app sorafs fetch`) を実行します。
  提供された `--gateway-provider` リストは、ダウンロードされたペイロードとスコアボードを下に保持します。
  `artifacts/da/prove_availability_<timestamp>/`、既存の PoR ヘルパーをすぐに呼び出します
  (`iroha app da prove`) フェッチされたバイトを使用します。オペレーターはオーケストレーターのノブを微調整できます
  (`--max-peers`、`--scoreboard-out`、マニフェスト エンドポイント オーバーライド) およびプルーフ サンプラー
  (`--sample-count`、`--leaf-index`、`--sample-seed`) 一方、単一のコマンドでアーティファクトが生成される
  DA-5/DA-9 監査で期待される内容: ペイロードのコピー、スコアボードの証拠、および JSON 証明の概要。- `da_reconstruct` (DA-6 の新機能) は、正規マニフェストとチャンクによって発行されたチャンク ディレクトリを読み取ります。
  (`chunk_{index:05}.bin` レイアウト) を保存し、検証しながらペイロードを確定的に再アセンブルします。
  Blake3 のすべての取り組み。 CLI は `crates/sorafs_car/src/bin/da_reconstruct.rs` の下に存在し、次のように出荷されます。
  SoraFS ツール バンドルの一部。一般的なフロー:
  1. `iroha app da get-blob --storage-ticket <ticket>`: `manifest_<manifest_hash>.norito` とチャンク プランをダウンロードします。
  2.`iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     (または `iroha app da prove-availability`、フェッチ アーティファクトを以下に書き込みます)
     `artifacts/da/prove_availability_<ts>/` であり、`chunks/` ディレクトリ内にチャンクごとのファイルが保持されます)。
  3.`cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`。

  回帰フィクスチャは `fixtures/da/reconstruct/rs_parity_v1/` の下に存在し、完全なマニフェストをキャプチャします
  および `tests::reconstructs_fixture_with_parity_chunks` によって使用されるチャンク マトリックス (データ + パリティ)。で再生成します

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  フィクスチャは次のものを放出します。

  - `manifest.{norito.hex,json}` — 正規の `DaManifestV1` エンコーディング。
  - `chunk_matrix.json` — ドキュメント/テスト参照の順序付けされたインデックス/オフセット/長さ/ダイジェスト/パリティ行。
  - `chunks/` — データ シャードとパリティ シャードの両方の `chunk_{index:05}.bin` ペイロード スライス。
  - `payload.bin` — パリティ対応ハーネス テストで使用される確定的ペイロード。
  - `commitment_bundle.{json,norito.hex}` — ドキュメント/テスト用の決定論的な KZG コミットメントを備えたサンプル `DaCommitmentBundle`。

  ハーネスは欠落または切り詰められたチャンクを拒否し、最終ペイロード Blake3 ハッシュを `blob_hash` に対してチェックします。
  CI が再構築をアサートできるように、概要の JSON BLOB (ペイロード バイト、チャンク カウント、ストレージ チケット) を出力します。
  証拠。これにより、オペレーターと QA が使用できる決定論的再構成ツールに関する DA-6 要件が終了します。
  ジョブは、特注のスクリプトを配線しなくても呼び出すことができます。

## TODO 解決策の概要

以前にブロックされたすべての取り込み TODO が実装され、検証されました。- **圧縮ヒント** — Torii は呼び出し元が提供したラベル (`identity`、`gzip`、`deflate`、
  `zstd`) を検証し、正規化マニフェスト ハッシュが一致するように検証前にペイロードを正規化します。
  解凍されたバイト。【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **ガバナンスのみのメタデータ暗号化** - Torii は、ガバナンス メタデータを
  構成された ChaCha20-Poly1305 キー、不一致のラベルを拒否し、2 つの明示的なラベルを表示します
  設定ノブ (`torii.da_ingest.governance_metadata_key_hex`、
  `torii.da_ingest.governance_metadata_key_label`) 回転を決定論的に保つため。【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **大規模なペイロードのストリーミング** — マルチパートの取り込みはライブです。クライアントストリームの決定性
  `client_blob_id` をキーとする `DaIngestChunk` エンベロープ、Torii は各スライスを検証し、それらをステージングします
  `manifest_store_dir` の下で、`is_last` フラグが設定されるとマニフェストをアトミックに再構築します。
  単一呼び出しアップロードで見られる RAM のスパイクを排除します。【crates/iroha_torii/src/da/ingest.rs:392】
- **マニフェスト バージョン管理** - `DaManifestV1` は明示的な `version` フィールドを保持し、Torii は拒否します
  不明なバージョン、新しいマニフェスト レイアウトの出荷時に決定的なアップグレードを保証します。【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR フック** — PDP コミットメントはチャンク ストアから直接派生し、永続化されます。
  マニフェストの横にあるため、DA-5 スケジューラーは正規データからサンプリング チャレンジを開始できます。の
  `Sora-PDP-Commitment` ヘッダーは、`/v2/da/ingest` と `/v2/da/manifests/{ticket}` の両方に同梱されるようになりました。
  応答により、SDK は将来のプローブが参照する署名済みコミットメントをすぐに学習します。【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da/ingest.rs:476】
- **シャード カーソル ジャーナル** — レーン メタデータは `da_shard_id` (デフォルトは `lane_id`) を指定する場合があります。
  Sumeragi は、`(shard_id, lane_id)` ごとに最高の `(epoch, sequence)` を保持するようになりました。
  `da-shard-cursors.norito` は DA スプールと並行しているため、再シャードされた/不明なレーンは削除され、再起動は維持されます
  リプレイ決定論的。メモリ内のシャード カーソル インデックスは、コミット時に高速で失敗するようになりました。
  デフォルトのレーン ID ではなくマップされていないレーンにより、カーソルの進行と再生エラーが発生します
  明示的であり、ブロック検証は専用のメソッドを使用してシャード カーソル回帰を拒否します。
  `DaShardCursorViolation` オペレーターの理由 + テレメトリ ラベル。スタートアップ/キャッチアップで DA が停止するようになりました
  Kura に未知のレーンまたは退行カーソルが含まれている場合は、水分補給をインデックスし、問題のあるレーンを記録します。
  DA を提供する前にオペレーターが修正できるようにブロックの高さを調整state.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/shard_cursor.rs】【crates/iroha_core/src/ sumeragi/main_loop.rs]【crates/iroha_core/src/state.rs】【crates/iroha_core/src/block.rs】【docs/source/nexus_lanes.md:47】
- **シャード カーソル ラグ テレメトリ** — `da_shard_cursor_lag_blocks{lane,shard}` ゲージがどのように報告するか破片は検証中の高さをはるかに下回ります。レーンが見つからない、古い、または不明であると、遅延が設定されます。
  必要な高さ (またはデルタ) を設定し、正常に進むとそれがゼロにリセットされるため、定常状態は平坦なままになります。
  オペレーターは、ゼロ以外のラグを警告し、問題のあるレーンの DA スプール/ジャーナルを検査する必要があります。
  ブロックをリプレイしてクリアする前に、レーン カタログで偶発的なリシャーディングがないか確認してください。
  ギャップ。
- **機密コンピューティング レーン** — でマークされたレーン
  `metadata.confidential_compute=true` および `confidential_key_version` は次のように扱われます。
  SMPC/暗号化された DA パス: Sumeragi は、ゼロ以外のペイロード/マニフェスト ダイジェストとストレージ チケットを強制します。
  フル レプリカ ストレージ プロファイルを拒否し、SoraFS チケット + ポリシー バージョンをインデックスなしで作成します。
  ペイロードバイトを公開します。リプレイ中にKuraからハイドレートを受け取るので、バリデーターも同じものを回復します
  再起動後の機密性メタデータ。【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/confidential.rs】【crates/iroha_core/src/da/confidential_store.rs】【crates/iroha_core/src/state.rs】

## 実装メモ- Torii の `/v2/da/ingest` エンドポイントは、ペイロード圧縮を正規化し、リプレイ キャッシュを強制するようになりました。
  正規のバイトを決定的にチャンク化し、`DaManifestV1` を再構築し、エンコードされたペイロードをドロップします
  レシートを発行する前に、SoraFS オーケストレーション用に `config.da_ingest.manifest_store_dir` に転送します。の
  ハンドラーは `Sora-PDP-Commitment` ヘッダーも添付するので、クライアントはエンコードされたコミットメントをキャプチャできます。
  【crates/iroha_torii/src/da/ingest.rs:220】
- 正規の `DaCommitmentRecord` を永続化した後、Torii は
  `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` ファイルはマニフェスト スプールの横にあります。
  各エントリは生の Norito `PdpCommitment` バイトでレコードをバンドルするため、DA-3 はビルダーと
  DA-5 スケジューラーは、マニフェストやチャンク ストアを再読み取りすることなく、同一の入力を取り込みます。【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK ヘルパーは、すべてのクライアントに Norito 解析の再実装を強制することなく、PDP ヘッダー バイトを公開します。
  `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` Rust、Python をカバー `ToriiClient`
  `decode_pdp_commitment_header` をエクスポートし、`IrohaSwift` には一致するヘルパーが同梱されるため、モバイルになります
  クライアントは、エンコードされたサンプリング スケジュールをすぐに隠しておくことができます。【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii は `GET /v2/da/manifests/{storage_ticket}` も公開するため、SDK とオペレーターはマニフェストをフェッチできます
  ノードのスプール ディレクトリに触れることなく、チャンク プランを作成できます。応答は Norito バイトを返します
  (base64)、レンダリングされたマニフェスト JSON、`sorafs fetch` に対応できる `chunk_plan` JSON BLOB、および関連する
  16 進ダイジェスト (`storage_ticket`、`client_blob_id`、`blob_hash`、`chunk_root`) なので、ダウンストリーム ツールで
  ダイジェストを再計算せずにオーケストレーターにフィードし、同じ `Sora-PDP-Commitment` ヘッダーを
  取り込み応答をミラーリングします。 `block_hash=<hex>` をクエリ パラメーターとして渡すと、決定論的な値が返されます。
  `sampling_plan` は `block_hash || client_blob_id` をルートとし (バリデータ間で共有)、
  `assignment_hash`、要求された `sample_window`、およびサンプリングされた `(index, role, group)` タプルの範囲
  2D ストライプ レイアウト全体なので、PoR サンプラーとバリデーターは同じインデックスを再生できます。サンプラー
  `client_blob_id`、`chunk_root`、および `ipa_commitment` を割り当てハッシュに混合します。 「いろはアプリだゲット」
  --block-hash ` now writes `sampling_plan_.json` をマニフェスト + チャンク プランの横に追加します。
  ハッシュは保存され、JS/Swift Torii クライアントは同じ `assignment_hash_hex` を公開するため、バリデーターは
  そして証明者は単一の決定論的プローブセットを共有します。 Torii がサンプリング計画を返すとき、「iroha app da」
  代わりに、prove-availability` now reuses that deterministic probe set (seed derived from `sample_seed`)
  オペレーターがバリデーターの割り当てを省略した場合でも、PoR 証人がバリデーターの割り当てと一致するようにアドホック サンプリングを行う
  `--block-hash` override.【crates/iroha_torii_shared/src/da/sampling.rs:1】【crates/iroha_cli/src/commands/da.rs:523】【javascript/iroha_js/src/toriiClient.js:15903】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:170】

### 大規模なペイロードのストリーミング フロー構成された単一リクエスト制限を超えるアセットを取り込む必要があるクライアントは、
`POST /v2/da/ingest/chunk/start` を呼び出してストリーミング セッションを実行します。 Torii は次のように応答します。
`ChunkSessionId` (要求された BLOB メタデータから BLAKE3 から派生) およびネゴシエートされたチャンク サイズ。
後続の各 `DaIngestChunk` リクエストには以下が含まれます。

- `client_blob_id` — 最終的な `DaIngestRequest` と同じです。
- `chunk_session_id` — スライスを実行中のセッションに結び付けます。
- `chunk_index` および `offset` — 決定的な順序付けを強制します。
- `payload` — ネゴシエートされたチャンク サイズまで。
- `payload_hash` — スライスの BLAKE3 ハッシュ。Torii は BLOB 全体をバッファリングせずに検証できます。
- `is_last` — 端末スライスを示します。

Torii は検証済みのスライスを `config.da_ingest.manifest_store_dir/chunks/<session>/` の下に保持し、
べき等性を尊重するために、リプレイ キャッシュ内の進行状況を記録します。最後のスライスが着地すると、Torii
ペイロードをディスク上で再構築します (メモリのスパイクを避けるためにチャンク ディレクトリを介してストリーミングします)。
シングルショットアップロードと同様に正規のマニフェスト/レシートを計算し、最終的に応答します。
ステージングされたアーティファクトを消費することによる `POST /v2/da/ingest`。失敗したセッションは明示的に中止することも、
`config.da_ingest.replay_cache_ttl` の後にガベージ コレクションが行われます。この設計はネットワーク形式を維持します
Norito に適しており、クライアント固有の再開可能なプロトコルを回避し、既存のマニフェスト パイプラインを再利用します。
変わらない。

**実装ステータス** 正規の Norito タイプは現在、
`crates/iroha_data_model/src/da/`:

- `ingest.rs` は、`DaIngestRequest`/`DaIngestReceipt` を定義します。
  Torii によって使用される `ExtraMetadata` コンテナ。【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` は `DaManifestV1` および `ChunkCommitment` をホストし、Torii はその後に発行します
  チャンク化が完了しました。【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` は共有エイリアス (`BlobDigest`、`RetentionPolicy`、
  `ErasureProfile` など)、以下に記載されているデフォルトのポリシー値をエンコードします。【crates/iroha_data_model/src/da/types.rs:240】
- マニフェスト スプール ファイルが `config.da_ingest.manifest_store_dir` に配置され、SoraFS オーケストレーションの準備が整いました。
  ストレージ入場に引き込むウォッチャー。【crates/iroha_torii/src/da/ingest.rs:220】
- Sumeragi は、DA バンドルをシールまたは検証するときにマニフェストの可用性を強制します。
  スプールにマニフェストがないか、ハッシュが異なる場合、ブロックは検証に失敗します。
  コミットメントより。【crates/iroha_core/src/sumeragi/main_loop.rs:5335】【crates/iroha_core/src/sumeragi/main_loop.rs:14506】

リクエスト、マニフェスト、および受信ペイロードのラウンドトリップ カバレッジは、
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`、Norito コーデックを保証
アップデート後も安定した状態を保ちます。【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**保持のデフォルト。** ガバナンスは、期間中に最初の保持ポリシーを承認しました。
SF-6; `RetentionPolicy::default()` によって適用されるデフォルトは次のとおりです。- ホット層: 7 日間 (`604_800` 秒)
- コールド層: 90 日 (`7_776_000` 秒)
- 必要なレプリカ: `3`
- ストレージクラス: `StorageClass::Hot`
- ガバナンスタグ: `"da.default"`

レーンが採用する場合、下流のオペレーターはこれらの値を明示的にオーバーライドする必要があります。
より厳しい要件。

## Rust クライアントの証明アーティファクト

Rust クライアントを組み込んだ SDK は、CLI にシェルアウトする必要がなくなりました。
正規の PoR JSON バンドルを生成します。 `Client` は 2 つのヘルパーを公開します。

- `build_da_proof_artifact` は、によって生成された正確な構造を返します。
  `iroha app da prove --json-out` (提供されたマニフェスト/ペイロード アノテーションを含む)
  [`DaProofArtifactMetadata`]経由。【crates/iroha/src/client.rs:3638】
- `write_da_proof_artifact` はビルダーをラップし、アーティファクトをディスクに永続化します。
  (デフォルトではきれいな JSON + 末尾の改行) 自動化でファイルを添付できるようにする
  リリースまたはガバナンスの証拠バンドルへ。【crates/iroha/src/client.rs:3653】

### 例

```rust
use iroha::{
    da::{DaProofArtifactMetadata, DaProofConfig},
    Client,
};

let client = Client::new(config);
let manifest = client.get_da_manifest_bundle(storage_ticket)?;
let payload = std::fs::read("artifacts/da/payload.car")?;
let metadata = DaProofArtifactMetadata::new(
    "artifacts/da/manifest.norito",
    "artifacts/da/payload.car",
);

// Build the JSON artefact in-memory.
let artifact = client.build_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
)?;

// Persist it next to other DA artefacts.
client.write_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
    "artifacts/da/proof_summary.json",
    true,
)?;
```

ヘルパーを離れる JSON ペイロードは、フィールド名に至るまで CLI と一致します。
(`manifest_path`、`payload_path`、`proofs[*].chunk_digest` など)、既存
自動化では、形式固有のブランチを使用せずにファイルの差分/パーケット/アップロードを行うことができます。

## 証明検証ベンチマーク

DA プルーフ ベンチマーク ハーネスを使用して、代表的なペイロードに対する検証者の予算を事前に検証します。
ブロックレベルのキャップを締める:

- `cargo xtask da-proof-bench` はマニフェスト/ペイロードのペアからチャンク ストアを再構築し、PoR をサンプルします
  終了し、設定された予算に対して検証を行います。 Taikai メタデータは自動入力され、
  フィクスチャ ペアに一貫性がない場合、ハーネスは合成マニフェストに戻ります。 `--payload-bytes`のとき
  明示的な `--payload` を指定せずに設定すると、生成された BLOB は次のように書き込まれます。
  `artifacts/da/proof_bench/payload.bin` なので、フィクスチャは変更されません。【xtask/src/da.rs:1332】【xtask/src/main.rs:2515】
- レポートのデフォルトは `artifacts/da/proof_bench/benchmark.{json,md}` で、プルーフ/実行、合計、および
  プルーフごとのタイミング、予算通過率、推奨予算 (最も遅いイテレーションの 110%)
  `zk.halo2.verifier_budget_ms`と並びます。【artifacts/da/proof_bench/benchmark.md:1】
- 最新の実行 (合成 1 MiB ペイロード、64 KiB チャンク、32 プルーフ/実行、10 回の反復、250 ミリ秒のバジェット)
  上限内で 100% の反復を行う 3 ミリ秒の検証予算を推奨しました。【artifacts/da/proof_bench/benchmark.md:1】
- 例 (決定論的なペイロードを生成し、両方のレポートを書き込みます):

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

ブロック アセンブリでは同じバジェットが適用されます: `sumeragi.da_max_commitments_per_block` と
`sumeragi.da_max_proof_openings_per_block` DA バンドルがブロックに埋め込まれる前にゲートします。
各コミットメントにはゼロ以外の `proof_digest` が含まれている必要があります。ガードはバンドルの長さを次のように扱います。
明示的な証明の要約がコンセンサスを通過するまでの証明開始カウント。
≤128-オープニングターゲットはブロック境界で強制可能です。【crates/iroha_core/src/sumeragi/main_loop.rs:6573】

## PoR 障害の処理とスラッシュストレージ担当者は、一連の PoR 障害と、それらの障害に沿った結合スラッシュの推奨事項を明らかにするようになりました。
評決。設定されたストライクしきい値を超える連続障害が発生すると、次のような推奨事項が表示されます。
プロバイダーとマニフェストのペア、スラッシュをトリガーしたストリークの長さ、および提案された
プロバイダーの保証金と `penalty_bond_bps` から計算されたペナルティ。クールダウンウィンドウ (秒) を維持します
同じインシデントで発火した重複したスラッシュ。【crates/sorafs_node/src/lib.rs:486】【crates/sorafs_node/src/config.rs:89】【crates/sorafs_node/src/bin/sorafs-node.rs:343】

- Storage Worker Builder を介してしきい値/クールダウンを構成します (デフォルトはガバナンスを反映しています)
  ペナルティポリシー）。
- スラッシュ推奨事項は評決概要 JSON に記録されるため、ガバナンス/監査人は添付できるようになります
  それらを証拠バンドルに追加します。
- ストライプ レイアウト + チャンクごとのロールが、Torii のストレージ ピン エンドポイントを介してスレッド化されるようになりました。
  (`stripe_layout` + `chunk_roles` フィールド) とストレージ ワーカーに永続化されるため、
  監査/修復ツールは、上流からレイアウトを再取得することなく、行/列の修復を計画できます。

### ハーネスの配置 + 修理

現在 `cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>`
`(index, role, stripe/column, offsets)` に対して配置ハッシュを計算し、最初に行を実行し、次に行を実行します。
ペイロードを再構築する前に列 RS(16) を修復します。

- 配置はデフォルトで `total_stripes`/`shards_per_stripe` (存在する場合) になり、チャンクに戻ります。
- 欠落または破損したチャンクは、最初に行パリティを使用して再構築されます。残りのギャップは次のように修復されます。
  ストライプ（カラム）パリティ。修復されたチャンクはチャンク ディレクトリに書き戻され、JSON
  summary は、配置ハッシュと行/列修復カウンターをキャプチャします。
- 行 + 列パリティが欠落セットを満たすことができない場合、ハーネスはすぐに失敗し、回復不能になります。
  監査人が修復不可能なマニフェストにフラグを立てることができるようにインデックスを作成します。