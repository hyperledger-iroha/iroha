---
lang: ja
direction: ltr
source: docs/source/nexus_privacy_commitments.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e7ef8ab7f52ec333d3fb9686dd744e23a859058dc0dbb91cfafee1cb7d1452ac
source_last_modified: "2025-11-21T17:27:28.236084+00:00"
translation_last_reviewed: 2026-01-01
---

# プライバシーコミットメントと証明フレームワーク (NX-10)

> **ステータス:** 🈴 Completed (NX-10)  
> **オーナー:** Cryptography WG ・ Privacy WG ・ Nexus Core WG  
> **関連コード:** [`crates/iroha_crypto/src/privacy.rs`](../../crates/iroha_crypto/src/privacy.rs)

NX-10 はプライベート lane のための共通コミットメント面を導入する。各 dataspace は、
Merkle ルートまたは zk-SNARK 回路を再現可能なハッシュに結び付ける決定的な
デスクリプタを公開する。これにより Nexus のグローバルリングは、特別なパーサ無しで
cross-lane 転送と機密性証明を検証できる。

## 目的とスコープ

- ガバナンス manifest と SDK が admission 時に使う数値スロットで合意できるよう、
  コミットメント識別子を正準化する。
- 実行系、Torii、オフチェーン監査が一貫して証明を評価できるよう、再利用可能な
  検証ヘルパー (`iroha_crypto::privacy`) を提供する。
- zk-SNARK 証明を、正準 verifying key digest と決定的な public input エンコーディングに
  束縛する。アドホックな transcript は使わない。
- lane bundle とガバナンス証拠に、runtime が強制するのと同じハッシュを含めるための
  registry/export ワークフローを文書化する。

このノートの範囲外: DA fan-out の仕組み、relay messaging、settlement router の plumbing。
これらの層は `nexus_cross_lane.md` を参照。

## Lane コミットメントモデル

Registry は `LanePrivacyCommitment` の ordered list を保持する:

```rust
use iroha_crypto::privacy::{
    CommitmentScheme, LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment, SnarkCircuit,
    SnarkCircuitId,
};

let id = LaneCommitmentId::new(1);
let merkle = LanePrivacyCommitment::merkle(
    id,
    MerkleCommitment::from_root_bytes(root_bytes, 16),
);

let snark = LanePrivacyCommitment::snark(
    LaneCommitmentId::new(2),
    SnarkCircuit::new(
        SnarkCircuitId::new(42),
        verifying_key_digest,
        statement_hash,
        proof_hash,
    ),
);
```

- **`LaneCommitmentId`** — lane manifest に記録される 16-bit の安定 ID。
- **`CommitmentScheme`** — `Merkle` または `Snark`。将来の variants（bulletproofs など）が
  enum を拡張する。
- **Copy semantics** — すべての descriptors は `Copy` を実装し、configs が heap churn なしで
  再利用できる。

Registry は Nexus lane bundle（`scripts/nexus/lane_registry_bundle.py`）に同梱される。ガバナンスが
新しいコミットメントを承認すると、bundle は JSON manifest と admission が消費する Norito
overlay の両方を更新する。

### Manifest スキーマ (`privacy_commitments`)

Lane manifest は `privacy_commitments` 配列を公開する。各 entry は ID と scheme を割り当て、
scheme 固有パラメータを含む:

```json
{
  "lane": "cbdc",
  "governance": "council",
  "privacy_commitments": [
    {
      "id": 1,
      "scheme": "merkle",
      "merkle": {
        "root": "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "max_depth": 16
      }
    },
    {
      "id": 2,
      "scheme": "snark",
      "snark": {
        "circuit_id": 5,
        "verifying_key_digest": "0x...",
        "statement_hash": "0x...",
        "proof_hash": "0x..."
      }
    }
  ]
}
```

Registry bundler は manifest をコピーし、`summary.json` に `(id, scheme)` を記録する。CI
(`lane_registry_verify.py`) は manifest を再パースし、summary がディスク内容と一致することを
確認する。

## Merkle コミットメント

`MerkleCommitment` は正準 `HashOf<MerkleTree<[u8;32]>>` と最大監査パス深度を保持する。運用者は
prover または ledger snapshot から root を直接エクスポートする。registry 内で再ハッシュはしない。

検証フロー:

```rust
use iroha_crypto::privacy::{LanePrivacyCommitment, MerkleWitness, PrivacyWitness};

let witness = MerkleWitness::from_leaf_bytes(leaf_bytes, proof);
LanePrivacyCommitment::merkle(id, commitment)
    .verify(PrivacyWitness::Merkle(witness))?;
```

- `max_depth` を超える proof は `PrivacyError::MerkleProofExceedsDepth` を返す。
- Audit path は `iroha_crypto::MerkleProof` を再利用し、shielded pools と Nexus private lanes が
  同じシリアライゼーションを共有する。
- 外部 pools を取り込む hosts は witness 構築前に `MerkleTree::shielded_leaf_from_commitment`
  で shielded leaf を変換する。

### 運用チェックリスト

- `(id, root, depth)` を lane manifest summary と evidence bundle に公開する。
- 各 cross-lane 転送に対し Merkle inclusion proof を admission log に添付する。ヘルパーは root を
  byte-for-byte で再現するため、監査はエクスポート済みハッシュだけ比較すればよい。
- 深度の予算を telemetry (`nexus_privacy_commitments.merkle.depth_used`) で追跡し、rollout が
  設定上限を超える前にアラートが発火するようにする。

## zk-SNARK コミットメント

`SnarkCircuit` entries は 4 つのフィールドを結び付ける:

| Field | Description |
|-------|-------------|
| `circuit_id` | 回路/バージョンの組に対するガバナンス管理 ID。 |
| `verifying_key_digest` | 正準 verifying key の Blake3 ハッシュ (DER または Norito encoding)。 |
| `statement_hash` | 正準 public input encoding (Norito または Borsh) の Blake3 ハッシュ。 |
| `proof_hash` | `verifying_key_digest || proof_bytes` の Blake3 ハッシュ。 |

Runtime ヘルパー:

```rust
use iroha_crypto::privacy::{
    hash_proof, hash_public_inputs, LanePrivacyCommitment, PrivacyWitness, SnarkWitness,
};

let witness = SnarkWitness {
    public_inputs: encoded_inputs,
    proof: proof_bytes,
};

LanePrivacyCommitment::snark(id, circuit)
    .verify(PrivacyWitness::Snark(witness))?;
```

- `hash_public_inputs` と `hash_proof` は同じモジュールにある。SDK は manifest や proof 生成時に
  これらを呼び出し、format drift を防ぐ必要がある。
- 記録された statement hash と提示された public inputs が一致しない場合、
  `PrivacyError::SnarkStatementMismatch` を返す。
- Proof bytes は Groth16/Plonk などの正準圧縮形式であること。ヘルパーは hash binding のみを
  検証し、曲線検証の完全版は prover/verifier サービス側に置く。

### Evidence ワークフロー

1. verifying key digest と proof hash を lane bundle manifest にエクスポートする。
2. 監査者が `hash_proof` を再計算できるよう raw proof artefact を governance evidence package に添付する。
3. 正準 public input encoding (hex または Norito) を statement hash と一緒に公開し、
   決定的な replay を可能にする。

## Proof Ingestion Pipeline

1. **Lane manifests** は、各 contract または programmable-money bucket を支配する
   `LaneCommitmentId` を宣言する。
2. **Admission** は `ProofAttachment.lane_privacy` entries を消費し、`LanePrivacyRegistry::verify`
   でルーティングされた lane の witnesses を検証し、検証済み commitment ids を lane compliance
   (`privacy_commitments_any_of`) に渡す。これにより programmable-money の allow/deny ルールは
   queue する前に proof の存在を要求できる。
3. **Telemetry** は `nexus_privacy_commitments.{merkle,snark}` カウンタを `LaneId`、`commitment_id`、
   outcome (`ok`, `depth_mismatch`, etc.) と共に増加させる。
4. **Governance evidence** は同じヘルパーで acceptance reports を再生成する
   (`ci/check_nexus_lane_registry_bundle.sh` は SNARK metadata が入ると bundle verification に接続する)。

### オペレータ可視性

Torii の `/v2/sumeragi/status` エンドポイントは `lane_governance[].privacy_commitments` 配列を公開し、
オペレータと SDK が公開済み manifests と live registry を bundle を再読することなく比較できる。
Snapshot は `crates/iroha_core/src/sumeragi/status.rs` 内で生成され、Torii の REST/JSON handlers
(`crates/iroha_torii/src/routing.rs`) からエクスポートされ、各クライアント
(`javascript/iroha_js/src/toriiClient.js`,
`python/iroha_python/src/iroha_python/client.py`, `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`)
がデコードする。Merkle roots と SNARK digests の両方に対し manifest schema を反映する。

Commitment-only または split-replica lanes は、manifest が `privacy_commitments` セクションを
欠く場合、admission に失敗する。これにより programmable-money flows は、決定的な proof anchors
が bundle とともに提供されるまで開始できない。

## Runtime Registry & Admission

- `LanePrivacyRegistry` (`crates/iroha_core/src/interlane/mod.rs`) は manifest loader の
  `LaneManifestStatus` 構造体を snapshot し、`LaneCommitmentId` でキーされた per-lane commitment
  map を保持する。Transaction queue と `State` は各 manifest reload と一緒にこの registry を
  導入する (`Queue::install_lane_manifests`, `State::install_lane_manifests`) ため、admission と
  consensus validation は常に正準 commitment にアクセスできる。
- `LaneComplianceContext` はオプションの `Arc<LanePrivacyRegistry>` 参照を持つ
  (`crates/iroha_core/src/compliance/mod.rs`)。`LaneComplianceEngine` がトランザクションを評価する
  とき、programmable-money flows は Torii で露出される per-lane commitments と同じものを
  queue する前に参照できる。
- Admission と core validation は governance manifest handle の横に `Arc<LanePrivacyRegistry>` を
  保持し (`crates/iroha_core/src/queue.rs`, `crates/iroha_core/src/state.rs`)、programmable-money
  モジュールや将来の interlane hosts が manifests のローテーション中でも一貫した privacy
  descriptor を読むことを保証する。

## Runtime Enforcement

Lane privacy proofs は `ProofAttachment.lane_privacy` を通じてトランザクション attachments と
一緒に運ばれる。Torii の admission と `iroha_core` の validation は各 attachment を
`LanePrivacyRegistry::verify` でルーティングされた lane の registry に対して検証し、proven
commitment ids を記録して compliance engine に渡す。`privacy_commitments_any_of` を指定する
ルールは、対応する attachment が検証成功しない限り `false` を返すため、programmable-money
lanes は必要な witness なしに queue も commit もできない。ユニットカバレッジは
`interlane::tests` と lane compliance tests にあり、attachment 経路と policy guardrails を
安定させる。

すぐに試す場合は [`privacy.rs`](../../crates/iroha_crypto/src/privacy.rs) のユニットテストを参照し、
Merkle と zk-SNARK commitments の成功/失敗ケースを確認する。
