---
lang: zh-hans
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65aff839e8970e96edb07dfb9655cb4e79f56d1d885b7782647f5dc8f328027b
source_last_modified: "2025-12-29T18:16:35.921274+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 桥接证明

桥接证明提交通过标准指令路径 (`SubmitBridgeProof`) 并以经过验证的状态登陆证明注册表。当前表面涵盖 ICS 风格的 Merkle 证明和具有固定保留和清单绑定的透明 ZK 有效负载。

## 验收规则

- 范围必须有序/非空并遵守 `zk.bridge_proof_max_range_len`（0 禁用上限）。
- 可选高度窗口拒绝过时/未来的证明：`zk.bridge_proof_max_past_age_blocks` 和 `zk.bridge_proof_max_future_drift_blocks` 是根据摄取证明的块高度进行测量的（0 禁用护栏）。
- 桥接证明不得与同一后端的现有证明重叠（保留固定证明并块重叠）。
- 清单哈希值必须非零；有效负载的大小上限为 `zk.max_proof_size_bytes`。
- ICS 有效负载遵循配置的 Merkle 深度上限并使用声明的哈希函数验证路径；透明有效负载必须声明一个非空后端标签。
- 固定证明免于保留修剪；未固定的校样仍然遵循全局 `zk.proof_history_cap`/grace/batch 设置。

## Torii API 表面

- `GET /v2/zk/proofs` 和 `GET /v2/zk/proofs/count` 接受桥接感知过滤器：
  - `bridge_only=true` 仅返回桥接证明。
  - `bridge_pinned_only=true` 缩小为固定桥校样。
  - `bridge_start_from_height` / `bridge_end_until_height` 夹紧桥范围窗口。
- `GET /v2/zk/proof/{backend}/{hash}` 返回桥元数据（范围、清单哈希、有效负载摘要）以及证明 id/状态/VK 绑定。
- 完整的 Norito 证明记录（包括有效负载字节）仍然可以通过 `GET /v2/proofs/{proof_id}` 供离线验证者使用。

## 桥接接收事件

桥车道通过 `RecordBridgeReceipt` 指令发出打印的收据。执行该指令
记录 `BridgeReceipt` 有效负载并在事件上发出 `DataEvent::Bridge(BridgeEvent::Emitted)`
流，取代了之前的仅日志存根。 CLI `iroha bridge emit-receipt` 帮助程序提交
键入指令，以便索引器可以确定地消耗收据。

## 外部验证草图（ICS）

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