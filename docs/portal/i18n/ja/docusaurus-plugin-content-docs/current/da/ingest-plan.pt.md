---
lang: ja
direction: ltr
source: docs/portal/docs/da/ingest-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note フォンテ カノニカ
エスペラ`docs/source/da/ingest_plan.md`。デュアス・ヴェルソエス・エムとしてのマンテンハ
:::

# Sora Nexus のデータ可用性の計画

_レディ: 2026-02-20 - 対応: コア プロトコル WG / ストレージ チーム / DA WG_

O ワークストリーム DA-2 estende Torii com uma API de ingestao de blob que Emite
メタダドス Norito は、SoraFS を複製したものです。エステ ドキュメント キャプチャー オ エスケマ
proposto、実装用の API の有効性を確認するための機能
すべてのシミュレーションを前に進めます (seguimentos DA-1)。トドOS
ペイロード形式 DEVEM 使用コーデック Norito; nao sao 許可フォールバック
セルデ/JSON。

## オブジェクト

- Aceitar blob grandes (segmentos Taikai、サイドカー デ レーン、アルテファトス デ)
  ガバナンカ) Torii による形式的決定論。
- Produzir マニフェスト Norito canonicos que descrevam o blob、parametros de codec、
  消去と政治の保持を許可します。
- SoraFS および enfileirar ジョブのストレージなしでチャンクのメタデータを永続化する
  レプリカオ。
- ピンの公開意図 + 政治のタグ レジストリなし SoraFS e observadores
  デ・ガバナンカ。
- 顧客の回復を決定するために承認された領収書を輸出する
  デ・パブリカオ。

## 上位 API (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

ペイロードは `DaIngestRequest` のコードは Norito です。米国として再投稿
`application/norito+v1` とレトルナム `DaIngestReceipt`。

|レスポスタ |意味 |
| --- | --- |
| 202 承認済み |チャンキング/レプリカオでの BLOB エンファイルレイド。レシートのトルネード。 |
| 400 不正なリクエスト |ヴィオラカオ・デ・エスケマ/タマンホ（ヴェジャ・バリダコエス）。 |
| 401 不正 |トークン API の有効/無効。 |
| 409 紛争 | Duplicado `client_blob_id` com メタデータが分岐しています。 |
| 413 ペイロードが大きすぎます | BLOB の設定の制限を超えます。 |
| 429 リクエストが多すぎます |レート制限アティンギド。 |
| 500 内部エラー | Falha inesperada (ログ + アラータ)。 |

## エスケマ Norito プロポスト

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

> 実装に関する注意事項: Rust パラメータのペイロードを代表するものとして
> `iroha_data_model::da::types` のアゴラ ヴィヴィム、リクエスト/受信のラッパー
> em `iroha_data_model::da::ingest` マニフェストの説明
> `iroha_data_model::da::manifest`。

O カンポ `compression` は、発信者がペイロードを準備していることを通知します。 Torii アセイタ
`identity`、`gzip`、`deflate`、e `zstd`、ハッシュ前の os バイトの解読、
チャンク化と検証により、オプションが明らかになります。

### 検証チェックリスト1. ヘッダー Norito が `DaIngestRequest` に対応していることを確認します。
2. Falhar se `total_size` diferir do tamanho canonico do ペイロード (descomprimido)
   最大限の設定を超えています。
3. `chunk_size` のファイル (可能性、<= 2 MiB)。
4. Garantir `data_shards + parity_shards` <= maximo グローバルパリティ >= 2。
5. `retention_policy.required_replica_count` はベースラインをテストします
   ガバナンカ。
6. Verificacao de assinatura contra hash canonico (除外署名)。
7. Rejeitar `client_blob_id` 重複、メタデータのペイロード以外のハッシュ
   フォームアイデンティコス。
8. fornecido、検証スキーマ + ハッシュ一致 com の Quando `norito_manifest`
   o マニフェスト recalculado apos o チャンク化。カソ・コントラリオ・ノード・ゲラ・マニフェスト
   エ・オ・アルマゼナ。
9. 政治的複製設定を行う: Torii reescreve o
   `RetentionPolicy` enviado com `torii.da_ingest.replication_policy` (ベジャ
   `replication-policy.md`) 再構築されたメタデータのマニフェスト
   retencao nao は、com o perfil imposto と一致します。

### Fluxo のチャンキングとレプリカ

1. ペイロード em `chunk_size`、チャンク + raiz Merkle による BLAKE3 の計算。
2. Construir Norito `DaManifestV1` (struct nova) 妥協点をキャプチャ
   チャンク (role/group_id)、レイアウト消去 (感染の危険性)
   `ipa_commitment`)、政治的メタデータを保持します。
3. Enfileirar os バイトは canonico sob をマニフェストします
   `config.da_ingest.manifest_store_dir` (Torii エスクリーブ アルキーヴォス)
   `manifest.encoded` レーン/エポック/シーケンス/チケット/フィンガープリント) パラメータ
   orquestracao SoraFS os ingira e vincule o storage ticket aos godos
   パーシシドス。
4. `sorafs_car::PinIntent` com タグを介してピンを公開する意図
   政治。
5. 通知監視者によるイベント Norito `DaIngestPublished` の送信
   (clientes leves、governca、analitica)。
6. Retornar `DaIngestReceipt` ao caller (assinado pela chave de servico DA de)
   Torii) ヘッダーの送信 `Sora-PDP-Commitment` パラケオス SDK のキャプチャー
   即時コミットメント。 O 領収書アゴラ含む `rent_quote`
   (um Norito `DaRentQuote`) e `stripe_layout`、許可を与えてください
   レンダベース、リザーバ、ボーナス PDP/PoTR のレイアウトの期待値を確認します。
   消去 2D アオ ラド ストレージ チケット アンテ デ コンプロメーター フンドス。

## ストレージ/レジストリの設定

- Estender `sorafs_manifest` com `DaManifestV1`、ハビリタンド解析
  決定論的。
- 追加の新規ストリーム レジストリ `da.pin_intent` com ペイロード バージョン
  マニフェストのハッシュ + チケット ID を参照します。
- キャプチャーの遅延を監視するためのパイプラインの取得、
  チャンク化のスループット、レプリカのバックログ、感染症の感染。

## 精巣戦略- スキーマの検証単位のテスト、実行のチェック、検出のテスト
  重複者。
- ゴールデン Verificando エンコーディング Norito de `DaIngestRequest`、マニフェストをテストします。
  領収書。
- モック SoraFS を統合したハーネス + レジストリ、有効なフラックス
  チャンク+ピン。
- 消去と保存の組み合わせに関する所有権テスト
  アレアトリア。
- 不正なメタデータに対するペイロード Norito のファジング。

## CLI および SDK のツール (DA-8)- `iroha app da submit` (novo エントリポイント CLI) ビルダー/パブリッシャーのアゴラエンボルブ
  ポッサム・インゲリル・ブロブの任意の摂取
  fora do fluxo de Taikai バンドル。おおコマンド万歳
  `crates/iroha_cli/src/commands/da.rs:1` コンソーム UM ペイロード、パーフィル
  メタデータ/マニフェストの事前保存オプションの消去/保存
  `DaIngestRequest` canonico com は CLI の設定を変更します。ベム・スセディダスを処刑する
  持続 `da_request.{norito,json}` e `da_receipt.{norito,json}` すすり泣き
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` 経由でオーバーライド) パラメータ
  OS のリリースレジストリ OS バイト Norito exatos usados durante a
  摂取。
- O comando usa por Padrao `client_blob_id = blake3(payload)` mas aceita
  `--client-blob-id` によるオーバーライド、メタデータの JSON のマッピング (`--metadata-json`)
  ジェラド以前のマニフェスト (`--manifest`)、準備パラサ `--no-submit` のサポート
  オフラインは `--endpoint` パラホスト Torii 個人設定です。 O レシート JSON e
  印象は標準出力ではありませんが、ディスコ、フェチャンド、要求はありません。
  DA-8 の「submit_blob」ツールをデストラバンドまたはトラバルホ デ パリダーデで SDK を実行します。
- `iroha app da get` adiciona um alias focado em DA para orquestrador マルチソース
  栄養補給`iroha app sorafs fetch`。オペラドール ポデム アポンタル パラ アートファトス
  マニフェスト + チャンク プラン (`--manifest`、`--plan`、`--manifest-id`) **ou**
  `--storage-ticket` 経由でストレージ チケット Torii を渡します。クアンド・オ・カミーニョ・ド
  チケットと使用法、`/v2/da/manifests/<ticket>` の CLI バイシャ マニフェスト、
  永続化またはバンドル sob `artifacts/da/fetch_<timestamp>/` (com をオーバーライド)
  `--manifest-cache-dir`)、`--manifest-id` の BLOB からの派生ハッシュ、エンタオ
  リスト `--gateway-provider` フォルネシダを実行します。トドOS
  ノブ avancados do fetcher SoraFS permanecem intactos (マニフェスト エンベロープ、
  クライアントのラベル、キャッシュのガード、トランスポートのオーバーライド、エクスポートの
  スコアボードのパス `--output`)、マニフェスト サーバーのエンドポイントのエンドポイント
  `--manifest-endpoint` パラホスト経由 Torii パーソナル、エンタオ OS チェック
  エンドツーエンドの可用性は、名前空間 `da` sem duplicar を有効にします
  ロジカ・ド・オルケストラドール。
- `iroha app da get-blob` baixa は、Torii 経由で canonicos direto をマニフェストします。
  `GET /v2/da/manifests/{storage_ticket}`。おお、コマンドー・エスクリーブ
  `manifest_{ticket}.norito`、`manifest_{ticket}.json` e
  `chunk_plan_{ticket}.json` すすり泣く `artifacts/da/fetch_<timestamp>/` (うーん
  `--output-dir` fornecido pelo usuario) enquanto imprime o commando exato de
  `iroha app da get` (`--manifest-id` を含む) フェッチの必要性
  オルケストラドール。マニフェストとディレトリオのスプールのためのすべてのオペラドール
  Torii の OS 芸術作品を確実に入手してください。 ○
  cliente Torii JavaScript espelha o mesmo fluxo via
  `ToriiClient.getDaManifest(storageTicketHex)`、レトルナンド OS バイト Norito
  デコディフィカド、マニフェスト JSON チャンク プラン、SDK インターフェイスの呼び出し元
  CLI を使用した orquestrador のセッション。 O SDK Swift のアゴラをメスマスとして公開
  地上権 (`ToriiClient.getDaManifestBundle(...)` mais)
  `fetchDaPayloadViaGateway(...)`)、canalizando はネイティブ ラッパー バンドルをバンドルしますorquestrador SoraFS クライアント iOS possam baixar マニフェスト、実行者
  CLI を呼び出して複数のソースとキャプチャを取得します。
  [IrohaSwift/ソース/IrohaSwift/ToriiClient.swift:240][IrohaSwift/ソース/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` 決定論的な計算と詳細な計算
  ストレージとフォルネシドの保持に対するインセンティブ。おお、ヘルパーさん
  `DaRentPolicyV1` ativa (JSON ou bytes Norito) ou o デフォルトの embutido を使用します。
  政治と政府の検証 JSON (`gib`、`months`、メタデータ
  政治と選挙の `DaRentQuote`) パラケ・オーディトレス・サイトム・チャージ XOR exatas
  アドホックな統治システム スクリプト。 O コマンド タンベム 出してください、レスモ エム
  uma linha `rent_quote ...` は、コンソールでペイロード JSON パラメータのログを実行する必要があります
  法的には、継続的な訓練が必要です。エンパレレ
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` コム
  `--policy-label "governance ticket #..."` パラ パーシスティル アートファトス カツオ クエリ
  構成ファイルのバンドルを参照してください。 CLIコルタ、ラベルパーソナライズド
  Recusa 文字列 vazias para que valores `policy_source` permanecam acionaveis
  tesouraria のダッシュボード。ヴェジャ
  `crates/iroha_cli/src/commands/da.rs` サブコマンドのパラメータ
  `docs/source/da/rent_policy.md` 政治的スキーマ。
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` encadeia tudo acima: ストレージ チケットを受け取ります。
  バイシャ、マニフェストのカノニコをバンドル、実行、またはマルチソースを実行
  (`iroha app sorafs fetch`) 反対リスト `--gateway-provider` フォルネシダ、持続
  o ペイロード baixado + スコアボード sob `artifacts/da/prove_availability_<timestamp>/`、
  即時呼び出しまたはヘルパー PoR 存在 (`iroha app da prove`) は OS を使用します
  バイトバイシャドス。オペラドール ポデム アジャスター オス ノブ ド オルケストラドール
  (`--max-peers`、`--scoreboard-out`、エンドポイントとマニフェストをオーバーライドします) e o
  サンプラー デ プルーフ (`--sample-count`、`--leaf-index`、`--sample-seed`) エンカントム
  Unico コマンド製品 OS Artefatos esperados por audio DA-5/DA-9: コピア ドゥ
  ペイロード、スコアボードの証拠、証明用の履歴書 JSON。