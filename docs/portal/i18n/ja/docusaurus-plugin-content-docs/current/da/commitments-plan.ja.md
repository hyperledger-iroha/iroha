---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/da/commitments-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e5ca0a0f774abeffc5c00885793b958d5622bfcbfc6630cc562cc66a3ce58973
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/da/commitments-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note 正規ソース
このページは `docs/source/da/commitments_plan.md` を反映します。旧ドキュメントが
退役するまで両方のバージョンを同期してください。
:::

# Sora Nexus Data Availability コミットメント計画 (DA-3)

_ドラフト: 2026-03-25 -- オーナー: Core Protocol WG / Smart Contract Team / Storage Team_

DA-3 は Nexus のブロック形式を拡張し、各 lane が DA-2 で受け付けた blobs を記述する
決定論的なレコードを埋め込めるようにします。本ノートは、正規データ構造、ブロック
パイプラインのフック、ライトクライアント証明、Torii/RPC のサーフェスを整理し、
バリデータが入場判定やガバナンス検証で DA コミットメントに依存できるようになる前に
必要な要素を明確化します。全ての payload は Norito でエンコードされ、SCALE や ad-hoc
JSON は使用しません。

## 目的

- 各 Nexus ブロック内に blob 単位のコミットメント (chunk root + manifest hash + 任意の
  KZG commitment) を載せ、ピアがオフレジャーのストレージを参照せず availability 状態を
  再構成できるようにする。
- ライトクライアントが特定ブロックに manifest hash が最終確定したことを検証できる
  決定論的 membership proof を提供する。
- relays、SDK、ガバナンス自動化が全ブロック再生なしで availability を監査できるように、
  Torii のクエリ (`/v2/da/commitments/*`) と proof を提供する。
- `SignedBlockWire` の正規エンベロープを維持し、Norito メタデータヘッダとブロック hash
  派生に新構造を通す。

## スコープ概要

1. `iroha_data_model::da::commitment` の **データモデル追加** と
   `iroha_data_model::block` のブロックヘッダ変更。
2. Torii が発行する DA receipt を `iroha_core` が取り込むための **executor hooks**
   (`crates/iroha_core/src/queue.rs` と `crates/iroha_core/src/block.rs`)。
3. WSV がコミットメントのクエリに高速応答できるようにする **永続化/インデックス**
   (`iroha_core/src/wsv/mod.rs`)。
4. `/v2/da/commitments` 配下で list/query/prove を提供する **Torii RPC 追加**。
5. `integration_tests/tests/da/commitments.rs` で wire layout と proof フローを検証する
   **統合テスト/fixture**。

## 1. データモデル追加

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // lane policy (merkle_sha256 or kzg_bls12_381)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub kzg_commitment: Option<KzgCommitment>,
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- `KzgCommitment` は `iroha_crypto::kzg` で使用している 48-byte の点を再利用します。
  ない場合は Merkle proof のみへフォールバックします。
- `proof_scheme` は lane カタログ由来です。Merkle lane は KZG payload を拒否し、
  `kzg_bls12_381` lane は非ゼロの KZG commitment を要求します。Torii は現在 Merkle
  commitment のみを生成し、KZG 設定の lane を拒否します。
- `KzgCommitment` は `iroha_crypto::kzg` の 48-byte ポイントを再利用します。
  Merkle lane で欠けている場合は Merkle proof のみへフォールバックします。
- `proof_digest` は DA-5 の PDP/PoTR 統合を見据え、同一レコード内にサンプリング
  スケジュールの hash を列挙できるようにします。

### 1.2 ブロックヘッダ拡張

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```

bundle hash はブロック hash と `SignedBlockWire` のメタデータに反映されます。DA データが

実装メモ: `BlockPayload` と透過的な `BlockBuilder` に `da_commitments` の setter/getter
が追加されています (`BlockBuilder::set_da_commitments` と
`SignedBlock::set_da_commitments`)。これによりホストはブロック封印前に bundle を付与できます。
すべての helper constructor は Torii が実データを流すまで `None` を既定にします。

### 1.3 Wire encoding

- `SignedBlockWire::canonical_wire()` は既存のトランザクションリスト直後に
  `DaCommitmentBundle` の Norito ヘッダを追加します。version byte は `0x01` です。
- `SignedBlockWire::decode_wire()` は未知の `version` を持つ bundle を拒否し、
  `norito.md` の Norito ポリシーに合わせます。
- hash 派生の更新は `block::Hasher` にのみ存在します。既存 wire format をデコードする
  ライトクライアントは Norito ヘッダが存在を宣言するため自動的に新フィールドを取得します。

## 2. ブロック生成フロー

1. Torii DA ingest が `DaIngestReceipt` を確定し、内部キューに発行します
   (`iroha_core::gossiper::QueueMessage::DaReceipt`)。
2. `PendingBlocks` は構築中のブロックの `lane_id` と一致する receipts を収集し、
   `(lane_id, client_blob_id, manifest_hash)` で重複排除します。
3. 封印直前に builder が `(lane_id, epoch, sequence)` でコミットメントをソートし、
   Norito codec で bundle をエンコードして `da_commitments_hash` を更新します。
4. 完全な bundle は WSV に保存され、`SignedBlockWire` とともにブロックへ出力されます。

ブロック生成に失敗した場合 receipts はキューに残り、次の試行で回収されます。builder は
lane ごとに最後に含めた `sequence` を記録してリプレイ攻撃を防ぎます。

## 3. RPC / Query サーフェス

Torii は 3 つの endpoint を公開します。

| Route | Method | Payload | Notes |
|-------|--------|---------|-------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (lane/epoch/sequence の範囲フィルタ、pagination) | `DaCommitmentPage` を返し、total count、commitments、block hash を含みます。 |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (lane + manifest hash または `(epoch, sequence)` タプル)。 | `DaCommitmentProof` (record + Merkle path + block hash) を返します。 |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` | stateless helper。ブロック hash 計算を再実行し、包含を検証します。`iroha_crypto` に直接リンクできない SDK 向け。 |

全 payload は `iroha_data_model::da::commitment` に存在します。Torii の router は既存の
DA ingest endpoint の隣に handlers を配置し、token/mTLS ポリシーを再利用します。

## 4. Inclusion proofs とライトクライアント

- ブロックプロデューサは `DaCommitmentRecord` のシリアライズ済みリストに対して
  二分 Merkle tree を構築します。ルートが `da_commitments_hash` に反映されます。
- `DaCommitmentProof` は対象 record と `(sibling_hash, position)` のベクタを含み、
  検証者がルートを再構成できます。proof には block hash と署名済みヘッダが含まれ、
  ライトクライアントが finality を検証できます。
- CLI helper (`iroha_cli app da prove-commitment`) は proof の要求/検証フローをラップし、
  オペレーター向けに Norito/hex 出力を提供します。

## 5. Storage / Indexing

WSV は `manifest_hash` をキーにした専用 column family に commitments を保存します。
二次インデックスは `(lane_id, epoch)` と `(lane_id, sequence)` をカバーし、クエリが
フル bundle をスキャンしないようにします。各 record は封印されたブロック高を追跡し、
catch-up ノードが block log から高速にインデックスを再構築できるようにします。

## 6. Telemetry / Observability

- `torii_da_commitments_total` はブロックが少なくとも 1 件の record を封印したときに増加。
- `torii_da_commitment_queue_depth` は bundle 化を待つ receipts を (lane 別に) 追跡。
- Grafana ダッシュボード `dashboards/grafana/da_commitments.json` はブロック包含、
  queue 深さ、proof throughput を可視化し、DA-3 の release gate 監査に使えます。

## 7. テスト戦略

1. `DaCommitmentBundle` の encoding/decoding とブロック hash 派生更新の **単体テスト**。
2. `fixtures/da/commitments/` に Norito の正規 bytes と Merkle proof を収めた **golden fixture**。
3. 2 台のバリデータで sample blobs を ingest し、bundle 内容と query/proof 応答の一致を
   確認する **統合テスト**。
4. `integration_tests/tests/da/commitments.rs` (Rust) の **ライトクライアントテスト** で
   `/prove` を呼び出し Torii に接続せずに proof を検証。
5. `scripts/da/check_commitments.sh` による **CLI smoke** でオペレーター tooling を再現可能に保つ。

## 8. Rollout 計画

| Phase | Description | Exit Criteria |
|-------|-------------|---------------|
| P0 - Data model merge | `DaCommitmentRecord`、ブロックヘッダ更新、Norito codec を導入。 | `cargo test -p iroha_data_model` が新 fixtures で green。 |
| P1 - Core/WSV wiring | queue + block builder ロジックを通し、indexes を永続化し、RPC handlers を公開。 | `cargo test -p iroha_core` と `integration_tests/tests/da/commitments.rs` が bundle proof を伴って通る。 |
| P2 - Operator tooling | CLI helpers、Grafana dashboard、proof 検証 docs を出荷。 | `iroha_cli app da prove-commitment` が devnet で動作し、ダッシュボードが live データを表示。 |
| P3 - Governance gate | `iroha_config::nexus` で指定された lanes に DA commitments を要求する block validator を有効化。 | status と roadmap 更新で DA-3 を完了と記載。 |

## オープン質問

1. **KZG vs Merkle defaults** - 小さな blobs では KZG commitments を常に省略して
   ブロックサイズを削減すべきか? 提案: `kzg_commitment` をオプションのままにし、
   `iroha_config::da.enable_kzg` でゲートする。
2. **Sequence gaps** - lane の順序欠落を許可するか? 現行案はガバナンスが
   `allow_sequence_skips` を緊急 replay 用に有効化しない限り拒否。
3. **Light-client cache** - SDK チームは proofs 用の軽量 SQLite キャッシュを希望;
   DA-8 でフォローアップ予定。

これらの課題を実装 PR で解決すると、DA-3 は「ドラフト」(本書) から
「進行中」へ移行します。
