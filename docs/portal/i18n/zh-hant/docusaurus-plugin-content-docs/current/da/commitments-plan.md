---
lang: zh-hant
direction: ltr
source: docs/portal/docs/da/commitments-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

標題：數據可用性承諾計劃
sidebar_label：承諾計劃
描述：用於在 Nexus 中嵌入 DA 承諾的塊、RPC 和證明管道。
---

:::注意規範來源
:::

# Sora Nexus 數據可用性承諾計劃 (DA-3)

_起草時間：2026-03-25 — 所有者：核心協議工作組/智能合約團隊/存儲團隊_

DA-3 擴展了 Nexus 塊格式，因此每個通道都嵌入確定性記錄
描述 DA-2 接受的 blob。本註釋捕獲規範數據
結構、塊管道鉤子、輕客戶端證明和 Torii/RPC 表面
必須在驗證者在准入期間依賴 DA 承諾之前落地
治理檢查。所有有效負載均採用 Norito 編碼；沒有 SCALE 或臨時 JSON。

## 目標

- 攜帶每個 blob 的承諾（塊根 + 清單哈希 + 可選的 KZG
  每個 Nexus 塊內的承諾），以便同行可以重建可用性
  狀態，無需諮詢賬外存儲。
- 提供確定性的成員資格證明，以便輕客戶端可以驗證
  清單哈希在給定塊中最終確定。
- 公開 Torii 查詢 (`/v2/da/commitments/*`) 和證據，讓中繼，
  SDK 和治理自動化審計可用性，無需重放每個
  塊。
- 通過線程新的來保持現有的 `SignedBlockWire` 信封規範
  通過 Norito 元數據頭和塊哈希派生的結構。

## 範圍概述

1. **`iroha_data_model::da::commitment` plus 塊中的數據模型添加**
   `iroha_data_model::block` 中的標頭髮生變化。
2. **執行器掛鉤**，因此 `iroha_core` 攝取 Torii 發出的 DA 收據
   （`crates/iroha_core/src/queue.rs` 和 `crates/iroha_core/src/block.rs`）。
3. **持久化/索引**，以便 WSV 可以快速回答承諾查詢
   （`iroha_core/src/wsv/mod.rs`）。
4. **Torii RPC 添加**，用於列出/查詢/證明端點
   `/v2/da/commitments`。
5. **集成測試 + 夾具** 驗證線路佈局和驗證流程
   `integration_tests/tests/da/commitments.rs`。

## 1. 數據模型添加

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

- `KzgCommitment` 重用下使用的現有 48 字節點
  `iroha_crypto::kzg`。當缺席時，我們僅使用 Merkle 證明。
- `proof_scheme`源自車道目錄； Merkle Lanes 拒絕 KZG
  有效負載，而 `kzg_bls12_381` 通道需要非零 KZG 承諾。 Torii
  目前僅產生 Merkle 承諾並拒絕 KZG 配置的通道。
- `KzgCommitment` 重用下使用的現有 48 字節點
  `iroha_crypto::kzg`。當默克爾通道缺席時，我們會退回到默克爾證明
  僅。
- `proof_digest` 預計 DA-5 PDP/PoTR 集成，因此記錄相同
  枚舉用於保持 blob 存活的採樣計劃。

### 1.2 區塊頭擴展

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

捆綁哈希同時輸入到塊哈希和 `SignedBlockWire` 元數據中。
開銷。

實現說明：`BlockPayload` 和透明 `BlockBuilder` 現在公開
`da_commitments` 設置器/獲取器（請參閱 `BlockBuilder::set_da_commitments` 和
`SignedBlock::set_da_commitments`)，因此主機可以附加預構建的捆綁包
在密封塊之前。所有輔助構造函數默認字段為 `None`
直到 Torii 將真正的捆綁包穿過。

### 1.3 有線編碼

- `SignedBlockWire::canonical_wire()` 附加 Norito 標頭
  `DaCommitmentBundle` 緊接在現有事務列表之後。的
  版本字節為 `0x01`。
- `SignedBlockWire::decode_wire()` 拒絕 `version` 未知的捆綁包，
  與 `norito.md` 中描述的 Norito 策略匹配。
- 哈希推導更新僅存在於 `block::Hasher` 中；輕客戶端解碼
  現有的有線格式自動獲得新字段，因為 Norito
  標頭宣告其存在。

## 2. 區塊生產流程

1. Torii DA 攝取最終確定 `DaIngestReceipt` 並將其發佈在
   內部隊列 (`iroha_core::gossiper::QueueMessage::DaReceipt`)。
2. `PendingBlocks` 收集 `lane_id` 與下面的塊匹配的所有收據
   構建，通過 `(lane_id, client_blob_id, manifest_hash)` 進行重複數據刪除。
3. 在密封之前，區塊生成器按 `(lane_id,
   紀元，序列）`以保持散列確定性，用
   Norito 編解碼器，並更新 `da_commitments_hash`。
4. 完整的包存儲在 WSV 中並與內部的塊一起發出
   `SignedBlockWire`。

如果塊創建失敗，收據仍保留在隊列中，因此下一個塊
嘗試可以撿起它們；構建器記錄最後包含的 `sequence` 每
車道以避免重放攻擊。

## 3. RPC 和查詢界面

Torii 公開三個端點：

|路線 |方法|有效負載|筆記|
|--------|--------|---------|--------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery`（按泳道/紀元/序列、分頁進行範圍過濾）|返回 `DaCommitmentPage` 以及總計數、承諾和塊哈希。 |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest`（通道 + 清單哈希或 `(epoch, sequence)` 元組）。 |響應 `DaCommitmentProof`（記錄 + Merkle 路徑 + 區塊哈希）。 |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` |無狀態助手，重播塊哈希計算並驗證包含；由無法直接鏈接到 `iroha_crypto` 的 SDK 使用。 |

所有有效負載均位於 `iroha_data_model::da::commitment` 下。 Torii 路由器安裝座
現有 DA 攝取端點旁邊的處理程序可重用令牌/mTLS
政策。

## 4. 包含證明和輕客戶端

- 區塊生產者在序列化的基礎上構建二叉 Merkle 樹
  `DaCommitmentRecord` 列表。根提供 `da_commitments_hash`。
- `DaCommitmentProof` 打包目標記錄加上向量 `(sibling_hash,
  position)` 條目，以便驗證者可以重建根。證明還包括
  塊哈希和簽名標頭，以便輕客戶端可以驗證最終性。
- CLI 助手 (`iroha_cli app da prove-commitment`) 包裝證明請求/驗證
  為操作員提供循環和表面 Norito/十六進制輸出。

## 5. 存儲和索引

WSV 將承諾存儲在由 `manifest_hash` 鍵入的專用列族中。
二級索引涵蓋 `(lane_id, epoch)` 和 `(lane_id, sequence)` 所以查詢
避免掃描完整的捆綁包。每條記錄都跟踪密封它的區塊高度，
允許追趕節點從塊日誌中快速重建索引。

## 6. 遙測和可觀測性

- 每當一個塊密封至少一個時，`torii_da_commitments_total` 就會遞增
  記錄。
- `torii_da_commitment_queue_depth` 跟踪等待捆綁的收據（每
  車道）。
- Grafana 儀表板 `dashboards/grafana/da_commitments.json` 可視化塊
  包含、隊列深度和證明吞吐量，以便 DA-3 發布門可以審核
  行為。

## 7. 測試策略

1. **`DaCommitmentBundle` 編碼/解碼和塊哈希的單元測試**
   推導更新。
2. **`fixtures/da/commitments/` 捕獲規範下的黃金燈具**
   捆綁字節和 Merkle 證明。
3. **集成測試** 啟動兩個驗證器，攝取樣本 blob，以及
   斷言兩個節點都同意捆綁內容和查詢/證明
   回應。
4. **`integration_tests/tests/da/commitments.rs` 中的輕客戶端測試**
   （Rust）調用 `/prove` 並驗證證明，而不與 Torii 交談。
5. **CLI Smoke** 腳本 `scripts/da/check_commitments.sh` 保持操作員
   工具可重複。

## 8. 推出計劃

|相|描述 |退出標準 |
|--------|-------------|---------------|
| P0 — 數據模型合併 |登陸 `DaCommitmentRecord`、塊頭更新和 Norito 編解碼器。 | `cargo test -p iroha_data_model` 綠色，帶有新燈具。 |
| P1 — 核心/WSV 接線 |線程隊列 + 塊構建器邏輯、持久索引並公開 RPC 處理程序。 | `cargo test -p iroha_core`、`integration_tests/tests/da/commitments.rs` 通過捆綁證明斷言。 |
| P2 — 操作員工具 |發布 CLI 幫助程序、Grafana 儀表板和證明驗證文檔更新。 | `iroha_cli app da prove-commitment` 適用於 devnet；儀表板顯示實時數據。 |
| P3——治理門|啟用需要在 `iroha_config::nexus` 中標記的通道上進行 DA 承諾的塊驗證器。 |狀態條目+路線圖更新將DA-3標記為🈴。 |

## 開放問題

1. **KZG 與 Merkle 默認值** — 小斑點是否應該始終跳過 KZG 的承諾
   減小塊大小？建議：保留 `kzg_commitment` 可選並通過gate via
   `iroha_config::da.enable_kzg`。
2. **序列間隙** — 我們是否允許無序通道？目前的計劃拒絕存在差距
   除非治理切換 `allow_sequence_skips` 進行緊急重播。
3. **輕客戶端緩存** — SDK 團隊請求輕量級 SQLite 緩存
   證明；有待 DA-8 下的後續行動。

在實施 PR 中回答這些問題會將 DA-3 從 🈸（本文檔）移至 🈺
一旦代碼工作開始。