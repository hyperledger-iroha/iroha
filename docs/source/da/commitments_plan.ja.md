---
lang: ja
direction: ltr
source: docs/source/da/commitments_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ea1b16b73a55e3e47dfe9d5bfc77dedce2e8fa9ff964d244856767f14931733
source_last_modified: "2026-01-22T15:38:30.660808+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus データ可用性コミットメント プラン (DA-3)

_草案: 2026-03-25 — 所有者: コアプロトコルWG / スマートコントラクトチーム / ストレージチーム_

DA-3 は Nexus ブロック形式を拡張し、すべてのレーンに確定的なレコードが埋め込まれるようにします。
DA-2 によって受け入れられる BLOB について説明します。このメモは正規データをキャプチャします
構造、ブロック パイプライン フック、ライト クライアント プルーフ、および Torii/RPC サーフェス
バリデーターが入場時に DA コミットメントに依存できるようになる前に、着陸する必要があります。
ガバナンスチェック。すべてのペイロードは Norito でエンコードされます。 SCALE またはアドホック JSON はありません。

## 目的

- BLOB ごとのコミットメントを実行 (チャンク ルート + マニフェスト ハッシュ + オプションの KZG)
  コミットメント）をすべての Nexus ブロック内で実行できるため、ピアは可用性を再構築できます
  簿外保管を参照せずに状態を維持します。
- ライトクライアントがメンバーシップを検証できるように、確定的なメンバーシップ証明を提供します。
  マニフェスト ハッシュが特定のブロックで確定されました。
- Torii クエリ (`/v1/da/commitments/*`) とリレーを可能にする証明を公開します。
  SDK とガバナンス自動化は、すべてをリプレイすることなく可用性を監査します。
  ブロック。
- 新しいエンベロープをスレッド化することで、既存の `SignedBlockWire` エンベロープを正規に保ちます。
  Norito メタデータ ヘッダーとブロック ハッシュ導出による構造。

## 範囲の概要

1. `iroha_data_model::da::commitment` plus ブロックの **データ モデルの追加**
   `iroha_data_model::block` でヘッダーが変更されます。
2. **Executor フック** により、`iroha_core` は、Torii によって発行された DA 受信を取り込みます
   (`crates/iroha_core/src/queue.rs` および `crates/iroha_core/src/block.rs`)。
3. **永続化/インデックス** により、WSV はコミットメント クエリに迅速に応答できます。
   (`iroha_core/src/wsv/mod.rs`)。
4. **Torii RPC 追加** (リスト/クエリ/証明エンドポイント用)
   `/v1/da/commitments`。
5. **統合テスト + 治具** でワイヤ レイアウトとプルーフ フローを検証します。
   `integration_tests/tests/da/commitments.rs`。

## 1. データモデルの追加

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

- `KzgCommitment` は、以下で使用される既存の 48 バイト ポイントを再利用します。
  `iroha_crypto::kzg`。マークルレーンは空のままにします。現在 `kzg_bls12_381` レーン
  チャンク ルートから派生した決定論的な BLAKE3-XOF コミットメントを受け取り、
  ストレージ チケットにより、外部証明者なしでもブロック ハッシュが安定した状態に保たれます。
- `proof_scheme` はレーン カタログから派生します。マークルレーンは迷走KZGを拒否します
  `kzg_bls12_381` レーンではゼロ以外の KZG コミットメントが必要です。
- `proof_digest` は DA-5 PDP/PoTR 統合を予期しているため、同じレコード
  BLOB をライブ状態に保つために使用されるサンプリング スケジュールを列挙します。

### 1.2 ブロックヘッダー拡張

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

バンドル ハッシュは、ブロック ハッシュと `SignedBlockWire` メタデータの両方にフィードされます。
オーバーヘッド。

実装メモ: `BlockPayload` と透過的な `BlockBuilder` が公開されるようになりました。
`da_commitments` セッター/ゲッター (`BlockBuilder::set_da_commitments` および
`SignedBlock::set_da_commitments`)、ホストは事前に構築されたバンドルを接続できます
ブロックを封印する前に。すべてのヘルパー コンストラクターのデフォルトのフィールドは `None`
Torii が実際のバンドルを通過するまで。

### 1.3 ワイヤーエンコーディング- `SignedBlockWire::canonical_wire()` は、Norito ヘッダーを追加します。
  既存のトランザクション リストの直後に `DaCommitmentBundle`。の
  バージョン バイトは `0x01` です。
- `SignedBlockWire::decode_wire()` は `version` が不明なバンドルを拒否します。
  `norito.md` で説明されている Norito ポリシーと一致します。
- ハッシュ導出の更新は `block::Hasher` にのみ存在します。ライトクライアントのデコード
  Norito であるため、既存のワイヤ形式は新しいフィールドを自動的に取得します。
  ヘッダーはその存在を宣伝します。

## 2. ブロック生産の流れ

1. Torii DA の取り込みは、署名されたレシートとコミットメント レコードを
   DAスプール(`da-receipt-*.norito` / `da-commitment-*.norito`)。耐久性のある
   受信ログは再起動時にカーソルをシードするため、再生された受信は引き続き順序付けされます
   決定論的に。
2. ブロックアセンブリはスプールからレシートをロードし、古くなった/すでに封印されたものをドロップします
   コミットされたカーソルのスナップショットを使用してエントリを作成し、エントリごとに連続性を強制します。
   `(lane, epoch)`。到達可能なレシートに一致するコミットメントがない場合、または
   マニフェスト ハッシュを分岐すると、提案は黙って省略されるのではなく中止されます。
3. 封印の直前に、ビルダーはコミットメントバンドルをスライスします。
   レシート駆動セット、`(lane_id, epoch, sequence)` でソート、エンコード
   Norito コーデックをバンドルし、`da_commitments_hash` をアップデートします。
4. 完全なバンドルは WSV に保存され、内部のブロックと一緒に出力されます。
   `SignedBlockWire`;コミットされたバンドルは受信カーソルを進めます (ハイドレートされた)
   再起動時に Kura から）、ディスクの増加に合わせて古いスプール エントリを削除します。

ブロック アセンブリと `BlockCreated` 取り込みにより、各コミットメントが再検証されます。
レーンカタログ: マークルレーンは漂遊 KZG コミットメントを拒否し、KZG レーンは
ゼロ以外の KZG コミットメントとゼロ以外の `chunk_root`、および不明なレーンは
落とした。 Torii の `/v1/da/commitments/verify` エンドポイントは同じガードをミラーリングします。
そして、決定論的な KZG コミットメントをすべてのスレッドに取り込みます。
`kzg_bls12_381` レコードにより、ポリシーに準拠したバンドルがブロック アセンブリに到達します。

DA-2 取り込み計画で説明されているマニフェスト フィクスチャは、ソースとしても機能します。
コミットメントバンドラーにとっての真実。 Torii テスト
`manifest_fixtures_cover_all_blob_classes` は、すべてのマニフェストを再生成します。
`BlobClass` バリアントであり、新しいクラスがフィクスチャを取得するまでコンパイルを拒否します。
各 `DaCommitmentRecord` 内のエンコードされたマニフェスト ハッシュが、
ゴールデン Norito/JSON ペア。【crates/iroha_torii/src/da/tests.rs:2902】

ブロックの作成が失敗した場合、受信はキューに残るため、次のブロックが
試みればそれらを拾うことができます。ビルダーは、最後に含まれた `sequence` を記録します。
リプレイ攻撃を避けるためのレーン。

## 3. RPC とクエリ サーフェス

Torii は 3 つのエンドポイントを公開します。|ルート |方法 |ペイロード |メモ |
|----------|----------|----------|----------|
| `/v1/da/commitments` | `POST` | `DaCommitmentQuery` (レーン/エポック/シーケンスによる範囲フィルター、ページネーション) |合計数、コミットメント、ブロック ハッシュを含む `DaCommitmentPage` を返します。 |
| `/v1/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (レーン + マニフェスト ハッシュまたは `(epoch, sequence)` タプル)。 | `DaCommitmentProof` (レコード + マークル パス + ブロック ハッシュ) で応答します。 |
| `/v1/da/commitments/verify` | `POST` | `DaCommitmentProof` |ブロック ハッシュ計算を再生し、含まれていることを検証するステートレス ヘルパー。 `iroha_crypto` に直接リンクできない SDK によって使用されます。 |

すべてのペイロードは `iroha_data_model::da::commitment` の下に存在します。 Toriiルーターマウント
既存の DA の隣にあるハンドラーは、トークン/mTLS を再利用するためにエンドポイントを取り込みます
政策。

## 4. 包含証明とライトクライアント

- ブロックプロデューサーは、シリアル化されたブロック上にバイナリマークルツリーを構築します。
  `DaCommitmentRecord` リスト。ルートは `da_commitments_hash` をフィードします。
- `DaCommitmentProof` は、ターゲット レコードと `(sibling_hash,
  検証者がルートを再構築できるように、position)` エントリを追加します。証拠には次のものも含まれます
  ブロック ハッシュと署名付きヘッダーにより、ライト クライアントがファイナリティを検証できます。
- CLI ヘルパー (`iroha_cli app da prove-commitment`) は証明リクエスト/検証をラップします。
  オペレータ用のサイクルおよびサーフェス Norito/hex 出力。

## 5. ストレージとインデックス作成

WSV は、`manifest_hash` をキーとする専用の列ファミリーにコミットメントを保存します。
セカンダリ インデックスは `(lane_id, epoch)` および `(lane_id, sequence)` をカバーするため、クエリは
バンドル全体をスキャンすることは避けてください。各記録はそれを封印したブロックの高さを追跡します。
これにより、キャッチアップ ノードがブロック ログからインデックスを迅速に再構築できるようになります。

## 6. テレメトリーと可観測性

- `torii_da_commitments_total` は、ブロックが少なくとも 1 つを封印するたびに増加します
  記録する。
- `torii_da_commitment_queue_depth` は、バンドルされるのを待っているレシートを追跡します (
  レーン）。
- Grafana ダッシュボード `dashboards/grafana/da_commitments.json` ブロックを視覚化
  DA-3 リリース ゲートが監査できるように、インクルージョン、キューの深さ、プルーフ スループットを設定します。
  行動。

## 7. テスト戦略

1. `DaCommitmentBundle` エンコード/デコードおよびブロック ハッシュの **単体テスト**
   導出の更新。
2. `fixtures/da/commitments/` で正規をキャプチャする **ゴールデン フィクスチャ**
   バイトとマークル証明をバンドルします。各バンドルはマニフェスト バイトを参照します
   `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` から、つまり
   `cargo test -p iroha_torii regenerate_da_ingest_fixtures -- --ignored --nocapture` を再生成しています
   `ci/check_da_commitments.sh` がコミットメントを更新する前に、Norito ストーリーの一貫性を維持します。
   証明。【fixture/da/ingest/README.md:1】
3. **統合テスト** 2 つのバリデーターを起動し、サンプル BLOB を取り込み、
   両方のノードがバンドルの内容とクエリ/証明に同意することを主張します。
   応答。
4. `integration_tests/tests/da/commitments.rs` の **ライトクライアント テスト**
   (Rust) `/prove` を呼び出し、Torii と通信せずに証明を検証します。
5. オペレーターを維持する **CLI スモーク** スクリプト `scripts/da/check_commitments.sh`
   再現可能なツール。

## 8.展開計画|フェーズ |説明 |終了基準 |
|------|-------------|------|
| P0 — データ モデルのマージ | Land `DaCommitmentRecord`、ブロック ヘッダーの更新、および Norito コーデック。 |新しい器具を備えた `cargo test -p iroha_data_model` グリーン。 |
| P1 — コア/WSV 配線 |キュー + ブロック ビルダー ロジックをスレッド化し、インデックスを永続化し、RPC ハンドラーを公開します。 | `cargo test -p iroha_core`、`integration_tests/tests/da/commitments.rs` はバンドル証明アサーションで合格します。 |
| P2 — オペレーターツール | CLI ヘルパー、Grafana ダッシュボード、および証明検証ドキュメントの更新を提供します。 | `iroha_cli app da prove-commitment` は devnet に対して機能します。ダッシュボードにはライブデータが表示されます。 |
| P3 — ガバナンスゲート | `iroha_config::nexus` でフラグが立てられたレーンで DA コミットメントを必要とするブロック バリデータを有効にします。 |ステータスエントリ + ロードマップ更新で DA-3 を 🈴 としてマークします。 |

## 未解決の質問

1. **KZG とマークルのデフォルト** — 小さな BLOB は常に KZG コミットメントをスキップすべきか
   ブロックサイズを減らすには？提案: `kzg_commitment` をオプションのままにし、ゲート経由でゲートする
   `iroha_config::da.enable_kzg`。
2. **シーケンス ギャップ** — 順序が乱れたレーンは許可されますか?現在の計画はギャップを拒否します
   ガバナンスが緊急再生用に `allow_sequence_skips` を切り替えない限り。
3. **ライトクライアント キャッシュ** — SDK チームは、軽量の SQLite キャッシュをリクエストしました。
   証拠; DA-8に基づく追跡調査は保留中。

実装 PR でこれらに答えると、DA-3 が 🈸 (このドキュメント) から 🈺 に移動します。
コード作業が始まると。