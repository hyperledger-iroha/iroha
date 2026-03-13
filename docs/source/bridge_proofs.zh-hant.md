---
lang: zh-hant
direction: ltr
source: docs/source/bridge_proofs.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65aff839e8970e96edb07dfb9655cb4e79f56d1d885b7782647f5dc8f328027b
source_last_modified: "2025-12-29T18:16:35.921274+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 橋接證明

橋接證明提交通過標準指令路徑 (`SubmitBridgeProof`) 並以經過驗證的狀態登陸證明註冊表。當前表面涵蓋 ICS 風格的 Merkle 證明和具有固定保留和清單綁定的透明 ZK 有效負載。

## 驗收規則

- 範圍必須有序/非空並遵守 `zk.bridge_proof_max_range_len`（0 禁用上限）。
- 可選高度窗口拒絕過時/未來的證明：`zk.bridge_proof_max_past_age_blocks` 和 `zk.bridge_proof_max_future_drift_blocks` 是根據攝取證明的塊高度進行測量的（0 禁用護欄）。
- 橋接證明不得與同一後端的現有證明重疊（保留固定證明並塊重疊）。
- 清單哈希值必須非零；有效負載的大小上限為 `zk.max_proof_size_bytes`。
- ICS 有效負載遵循配置的 Merkle 深度上限並使用聲明的哈希函數驗證路徑；透明有效負載必須聲明一個非空後端標籤。
- 固定證明免於保留修剪；未固定的校樣仍然遵循全局 `zk.proof_history_cap`/grace/batch 設置。

## Torii API 表面

- `GET /v2/zk/proofs` 和 `GET /v2/zk/proofs/count` 接受橋接感知過濾器：
  - `bridge_only=true` 僅返回橋接證明。
  - `bridge_pinned_only=true` 縮小為固定橋校樣。
  - `bridge_start_from_height` / `bridge_end_until_height` 夾緊橋範圍窗口。
- `GET /v2/zk/proof/{backend}/{hash}` 返回橋元數據（範圍、清單哈希、有效負載摘要）以及證明 id/狀態/VK 綁定。
- 完整的 Norito 證明記錄（包括有效負載字節）仍然可以通過 `GET /v2/proofs/{proof_id}` 供離線驗證者使用。

## 橋接接收事件

橋車道通過 `RecordBridgeReceipt` 指令發出打印的收據。執行該指令
記錄 `BridgeReceipt` 有效負載並在事件上發出 `DataEvent::Bridge(BridgeEvent::Emitted)`
流，取代了之前的僅日誌存根。 CLI `iroha bridge emit-receipt` 幫助程序提交
鍵入指令，以便索引器可以確定地消耗收據。

## 外部驗證草圖（ICS）

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