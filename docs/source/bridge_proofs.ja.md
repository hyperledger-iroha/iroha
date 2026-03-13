---
lang: ja
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3f6049cbf3aa135e35e4cf06967993c11c1c571ca97dd11c469142dd620be77
source_last_modified: "2025-12-11T23:36:13.998930+00:00"
translation_last_reviewed: 2026-01-01
---

# Bridge proofs

Bridge proof の提出は標準の命令パス (`SubmitBridgeProof`) を通過し、検証済みステータスで proof registry に保存されます。現在のサーフェスは ICS 形式の Merkle proof と、保持期間の固定と manifest バインディングを持つ transparent-ZK payload をカバーします。

## 受け入れルール

- 範囲は順序付き/非空であり、`zk.bridge_proof_max_range_len` を満たすこと (0 で上限無効)。
- 省略可能な高さウィンドウは古い/未来の証明を拒否する: `zk.bridge_proof_max_past_age_blocks` と `zk.bridge_proof_max_future_drift_blocks` は proof を取り込むブロック高に対して評価される (0 でガード無効)。
- 同一 backend に対する既存の proof と重複できない (pinned proof は保持され、重複をブロックする)。
- Manifest hash は非ゼロであること。payload は `zk.max_proof_size_bytes` でサイズ上限。
- ICS payload は設定された Merkle 深度上限に従い、宣言された hash 関数で経路検証する。transparent payload は非空の backend ラベルを宣言する必要がある。
- Pinned proofs は保持期間の pruning 対象外。unpinned proofs は `zk.proof_history_cap`/grace/batch のグローバル設定を引き続き尊重する。

## Torii API サーフェス

- `GET /v2/zk/proofs` と `GET /v2/zk/proofs/count` は bridge 対応のフィルタを受け付ける:
  - `bridge_only=true` は bridge proof のみを返す。
  - `bridge_pinned_only=true` は pinned bridge proof に限定する。
  - `bridge_start_from_height` / `bridge_end_until_height` は bridge の範囲ウィンドウを制限する。
- `GET /v2/zk/proof/{backend}/{hash}` は bridge メタデータ (範囲、manifest hash、payload 要約) と proof id/status/VK bindings を返す。
- payload bytes を含む Norito の完全な proof レコードは `GET /v2/proofs/{proof_id}` で引き続き取得可能 (ノード外検証向け)。

## Bridge receipt イベント

Bridge lane は `RecordBridgeReceipt` 命令で型付き receipt を発行します。この命令を実行すると `BridgeReceipt` payload が記録され、イベントストリームに `DataEvent::Bridge(BridgeEvent::Emitted)` が発行され、従来のログ専用 stub を置き換えます。CLI の `iroha bridge emit-receipt` helper は型付き命令を送信し、インデクサが決定論的に receipt を消費できるようにします。

## 外部検証スケッチ (ICS)

```rust
use iroha_data_model::bridge::{BridgeHashFunction, BridgeProofPayload, BridgeProofRecord};
use iroha_crypto::{Hash, HashOf, MerkleTree};

fn verify_ics(record: &BridgeProofRecord) -> bool {
    let BridgeProofPayload::Ics(ics) = &record.proof.payload else {
        return false;
    };
    let leaf = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(ics.leaf_hash));
    let root =
        HashOf::<MerkleTree<[u8; 32]>>::from_untyped_unchecked(Hash::prehashed(ics.state_root));
    match ics.hash_function {
        BridgeHashFunction::Sha256 => ics.proof.clone().verify_sha256(&leaf, &root, ics.proof.audit_path().len()),
        BridgeHashFunction::Blake2b => ics.proof.clone().verify(&leaf, &root, ics.proof.audit_path().len()),
    }
}
```
