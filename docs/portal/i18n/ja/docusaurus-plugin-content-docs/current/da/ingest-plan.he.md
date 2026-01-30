---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/da/ingest-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0ffd5aca163243d61691d7a474ac74ce8b86b68f5d82c0b4bcb3b2df8ec1cabb
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/da/ingest-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note 正規ソース
このページは `docs/source/da/ingest_plan.md` を反映します。旧ドキュメントが
退役するまで両方のバージョンを同期してください。
:::

# Sora Nexus Data Availability インジェスト計画

_ドラフト: 2026-02-20 - オーナー: Core Protocol WG / Storage Team / DA WG_

DA-2 のワークストリームは、Norito メタデータを発行して SoraFS のレプリケーションを
起動する blob ingest API を Torii に追加します。この文書は、提案スキーマ、API
サーフェス、検証フローを整理し、DA-1 のシミュレーション待ちで実装が止まらないように
することが目的です。すべての payload 形式は Norito コーデックを MUST で使用し、
serde/JSON のフォールバックは許可されません。

## 目標

- 大容量 blob (Taikai セグメント、lane sidecar、ガバナンスアーティファクト) を
  Torii 経由で決定論的に受け付ける。
- blob、codec パラメータ、erasure プロファイル、retention ポリシーを記述する
  正規の Norito manifest を生成する。
- chunk メタデータを SoraFS の hot storage に永続化し、レプリケーション job を
  キューに積む。
- pin intent とポリシータグを SoraFS registry とガバナンス観測者に公開する。
- admission receipt を提供し、クライアントが決定論的な公開証明を得られるようにする。

## API サーフェス (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

payload は Norito でエンコードされた `DaIngestRequest`。レスポンスは
`application/norito+v1` を使用し、`DaIngestReceipt` を返します。

| レスポンス | 意味 |
| --- | --- |
| 202 Accepted | blob を chunking/replication にキューし、receipt を返す。 |
| 400 Bad Request | schema/サイズ違反 (検証チェック参照)。 |
| 401 Unauthorized | API トークンの欠落/無効。 |
| 409 Conflict | `client_blob_id` が重複し、メタデータが一致しない。 |
| 413 Payload Too Large | 設定された blob 長上限を超過。 |
| 429 Too Many Requests | レート制限超過。 |
| 500 Internal Error | 予期しない失敗 (ログ + アラート)。 |

## 提案 Norito スキーマ

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

> 実装メモ: これらの payload の正規 Rust 表現は `iroha_data_model::da::types`
> に移行済みです。request/receipt ラッパーは `iroha_data_model::da::ingest`、
> manifest 構造は `iroha_data_model::da::manifest` にあります。

`compression` フィールドは caller が payload をどう準備したかを示します。Torii は
`identity`/`gzip`/`deflate`/`zstd` を受け付け、hashing、chunking、任意 manifest 検証の
前にバイト列を展開します。

### 検証チェックリスト

1. リクエストの Norito ヘッダが `DaIngestRequest` と一致すること。
2. `total_size` が (展開後の) 正規 payload 長と一致しない、または上限超過の場合は失敗。
3. `chunk_size` のアライメント (2 の冪、<= 2 MiB) を強制。
4. `data_shards + parity_shards` <= グローバル上限、かつ parity >= 2。
5. `retention_policy.required_replica_count` はガバナンスの baseline を尊重。
6. 署名検証 (signature フィールドを除いた正規 hash に対して)。
7. `client_blob_id` の重複は、payload hash とメタデータが同一の場合のみ許可。
8. `norito_manifest` 提供時は、schema と hash が chunking 後に再計算した manifest と
   一致することを検証。なければノードが manifest を生成して保存。
9. 設定済みレプリケーションポリシーを強制: Torii は提出 `RetentionPolicy` を
   `torii.da_ingest.replication_policy` で上書きし ( `replication-policy.md` 参照 )、
   retention メタデータが一致しない事前生成 manifest を拒否。

### Chunking & Replication フロー

1. payload を `chunk_size` に分割し、各 chunk の BLAKE3 と Merkle root を算出。
2. Norito `DaManifestV1` (新 struct) を構築し、chunk commitment (role/group_id)、
   erasure layout (行/列 parity 数と `ipa_commitment`)、retention ポリシーと
   メタデータを格納。
3. 正規 manifest bytes を `config.da_ingest.manifest_store_dir` にキューし、Torii が
   lane/epoch/sequence/ticket/fingerprint で `manifest.encoded` を書き出す。SoraFS
   オーケストレーションがこれを取り込み、storage ticket と永続化データをリンク。
4. `sorafs_car::PinIntent` でガバナンスタグ + ポリシーの pin intent を公開。
5. Norito イベント `DaIngestPublished` を発行し、観測者 (ライトクライアント、
   ガバナンス、分析) に通知。
6. `DaIngestReceipt` を caller に返す (Torii DA サービスキーで署名)。同時に
   `Sora-PDP-Commitment` ヘッダを送出して SDK が即時に commitment を取得できるように
   する。receipt には `rent_quote` (Norito `DaRentQuote`) と `stripe_layout` を含め、
   送信者が storage ticket と併せてベース賃料、リザーブシェア、PDP/PoTR ボーナス期待値、
   2D erasure レイアウトを表示できる。

## Storage / Registry 更新

- `sorafs_manifest` を `DaManifestV1` で拡張し、決定論的なパースを可能にする。
- registry に `da.pin_intent` の新ストリームを追加し、manifest hash + ticket id を
  参照するバージョン付き payload を導入。
- ingest レイテンシ、chunking throughput、replication backlog、失敗数を追跡する
  observability パイプラインを更新。

## テスト戦略

- schema 検証、署名チェック、重複検出のユニットテスト。
- `DaIngestRequest`/manifest/receipt の Norito エンコードを検証する golden テスト。
- mock SoraFS + registry を立ち上げ、chunk + pin フローを検証する統合ハーネス。
- erasure プロファイルと retention 組み合わせのプロパティテスト。
- Norito payload を対象にした Fuzzing で不正メタデータに対処。

## CLI & SDK ツール (DA-8)

- `iroha app da submit` (新 CLI エントリポイント) は共有 ingest builder/publisher を
  包装し、Taikai bundle フロー外でも任意 blob を ingest できるようにします。コマンドは
  `crates/iroha_cli/src/commands/da.rs:1` にあり、payload、erasure/retention プロファイル、
  任意の metadata/manifest ファイルを受け取って CLI 設定キーで正規 `DaIngestRequest` を
  署名します。成功時は `da_request.{norito,json}` と `da_receipt.{norito,json}` を
  `artifacts/da/submission_<timestamp>/` に保存 (`--artifact-dir` で上書き可) し、
  リリース artefact が正確な Norito bytes を記録できるようにします。
- このコマンドは既定で `client_blob_id = blake3(payload)` を使いますが、
  `--client-blob-id` の override、`--metadata-json` の JSON マップ、`--manifest` の
  事前生成 manifest をサポートします。`--no-submit` によるオフライン準備と、
  `--endpoint` によるカスタム Torii ホストも可能です。receipt JSON は stdout に出力し
  つつディスクにも保存され、DA-8 の "submit_blob" 要件を満たして SDK パリティを前進
  させます。
- `iroha app da get` は `iroha app sorafs fetch` を動かす multi-source orchestrator の DA 向け
  エイリアスです。manifest + chunk-plan artefact (`--manifest`, `--plan`, `--manifest-id`)
  を指定するか、Torii storage ticket を `--storage-ticket` で渡せます。ticket パスでは
  `/v1/da/manifests/<ticket>` から manifest を取得し、`artifacts/da/fetch_<timestamp>/`
  に保存 (`--manifest-cache-dir` で上書き可)、`--manifest-id` 用に blob hash を導出して
  `--gateway-provider` で orchestrator を実行します。SoraFS fetcher の高度な knobs
  (manifest envelopes、client labels、guard caches、匿名 transport overrides、
  scoreboard export、`--output` パス) はそのまま維持され、`--manifest-endpoint` で
  manifest endpoint を差し替え可能です。これにより end-to-end availability 検証は
  `da` 名前空間内で完結し、orchestrator ロジックの複製が不要になります。
- `iroha app da get-blob` は Torii から `GET /v1/da/manifests/{storage_ticket}` で正規
  manifest を直接取得します。`manifest_{ticket}.norito`、`manifest_{ticket}.json`、
  `chunk_plan_{ticket}.json` を `artifacts/da/fetch_<timestamp>/` (または `--output-dir`)
  に書き込み、続く orchestrator fetch に必要な `iroha app da get` コマンド
  ( `--manifest-id` 含む ) を表示します。これにより manifest spool ディレクトリを
  直接触らず、Torii 署名 artefact を常に使用できます。JavaScript の Torii クライアントも
  `ToriiClient.getDaManifest(storageTicketHex)` で同じフローを提供し、Norito bytes、
  manifest JSON、chunk plan を返すことで SDK 呼び出し側が CLI なしで orchestrator
  セッションを組み立てられます。Swift SDK でも `ToriiClient.getDaManifestBundle(...)`
  と `fetchDaPayloadViaGateway(...)` を公開し、SoraFS オーケストレータのネイティブ
  ラッパーに bundle を渡して iOS クライアントが manifest 取得、multi-source fetch、
  証明取得を CLI なしで行えるようにしています。
  [IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240][IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` は指定した storage サイズと retention 期間に対する
  決定論的 rent とインセンティブ内訳を計算します。`DaRentPolicyV1` (JSON/Norito bytes)
  または内蔵デフォルトを読み込み、ポリシーを検証して JSON 要約 (`gib`, `months`,
  ポリシー metadata、`DaRentQuote` の各フィールド) を出力します。これにより監査担当は
  ad hoc スクリプトなしで XOR 料金を議事録に引用できます。コマンドは JSON payload の
  前に `rent_quote ...` を 1 行出力し、インシデント演習中のログ可読性を保ちます。
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` と `--policy-label "governance ticket #..."`
  を併用すると、正確な投票や設定バンドルを引用する整形 artefact を保存できます。
  CLI はカスタム label をトリムし、空文字列は拒否して `policy_source` を有効な状態に
  保ちます。サブコマンドは `crates/iroha_cli/src/commands/da.rs`、ポリシースキーマは
  `docs/source/da/rent_policy.md` を参照してください。
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` は一連の処理を連結します。storage ticket を受け取り、
  正規 manifest bundle を取得し、`--gateway-provider` で multi-source orchestrator
  (`iroha app sorafs fetch`) を実行、取得 payload + scoreboard を
  `artifacts/da/prove_availability_<timestamp>/` に保存し、既存の PoR helper
  (`iroha app da prove`) を即座に実行します。オペレータは orchestrator knobs
  (`--max-peers`, `--scoreboard-out`, manifest endpoint overrides) と proof sampler
  (`--sample-count`, `--leaf-index`, `--sample-seed`) を調整でき、1 回のコマンドで
  DA-5/DA-9 監査に必要な artefact (payload コピー、scoreboard 証拠、JSON 証明サマリ)
  を生成できます。
