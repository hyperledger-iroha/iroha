---
lang: zh-hant
direction: ltr
source: docs/source/merge_ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44f1c681730f1c94d9d00e8f829a0134374ce6cb29f21727a27685e096f0da40
source_last_modified: "2026-01-17T06:10:29.077000+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 合併賬本設計 — 通道最終性和全局縮減

本說明最終確定了 Milestone 5 的合併賬本設計。它解釋了
非空塊策略、跨通道 QC 合併語義和最終工作流程
這將車道級執行與全球世界狀態承諾聯繫起來。

該設計擴展了 `nexus.md` 中描述的 Nexus 架構。術語如
“車道塊”、“車道QC”、“合併提示”和“合併分類賬”繼承它們的
該文件中的定義；本說明重點關注行為規則和
必須由運行時、存儲和 WSV 強制執行的實施指南
層。

## 1. 非空塊策略

**規則（必須）：** 僅當塊包含 at 時，車道提議者才發出塊
至少一個已執行的事務片段、基於時間的觸發器或確定性的
工件更新（例如，DA 工件匯總）。禁止空塊。

**影響：**

- Slot keep-alive：當沒有事務滿足其確定性提交窗口時，
該車道不發出任何方塊，只是前進到下一個槽位。合併賬本
仍保留在該車道的前一個提示上。
- 觸發器批處理：不產生狀態轉換的後台觸發器（例如，
重申不變量的 cron）被認為是空的並且必須被跳過或者
在生成塊之前與其他工作捆綁在一起。
- 遙測：`pipeline_detached_merged` 和後續指標處理被跳過
明確的插槽——操作員可以區分“無工作”和“管道停滯”。
- 重播：塊存儲不會插入合成的空佔位符。庫拉
如果沒有，重播循環只是觀察連續插槽的相同父哈希值
塊被發射。

**規範檢查：** 在區塊提案和驗證期間，`ValidBlock::commit`
斷言關聯的 `StateBlock` 至少攜帶一個已提交的覆蓋
（增量、工件、觸發器）。這與 `StateBlock::is_empty` 防護一致
這已經確保了無操作寫入被省略。強制執行發生在之前
要求籤名，以便委員會永遠不會對空有效負載進行投票。

## 2. 跨通道 QC 合併語義

由其委員會最終確定的每個車道塊 `B_i` 產生：

- `lane_state_root_i`：Poseidon2-SMT 對每個 DS 狀態根源的承諾已觸及
在街區裡。
- `merge_hint_root_i`：合併分類賬的滾動候選者（`tag =
"iroha:merge:candidate:v1\0"`).
- `lane_qc_i`：車道委員會在
  執行投票原像（塊哈希，`parent_state_root`，
  `post_state_root`、高度/視圖/紀元、chain_id 和模式標籤）。

合併節點收集最新提示`{(B_i, lane_qc_i, merge_hint_root_i)}`
所有車道 `i ∈ [0, K)`。

**合併條目（必須）：**

```
MergeLedgerEntry {
    epoch_id: u64,
    lane_tips: [Hash32; K],
    merge_hint_root: [Hash32; K],
    global_state_root: Hash32,
    merge_qc: QuorumCertificate,
}
```- `lane_tips[i]` 是通道塊的哈希值，通道的合併條目密封
  `i`。如果自上一個合併條目以來車道沒有發出任何塊，則該值為
  重複。
- `merge_hint_root[i]` 是對應通道的 `merge_hint_root`
  塊。當 `lane_tips[i]` 重複時，它會重複。
- `global_state_root` 等於 `ReduceMergeHints(merge_hint_root[0..K-1])`，
  Poseidon2 折疊與域分離標籤
  `"iroha:merge:reduce:v1\0"`。減少是確定性的並且必須
  在同行之間重建相同的價值。
- `merge_qc` 是合併委員會頒發的 BFT 法定人數證書
  序列化條目。

**合併 QC 有效負載（必須）：**

合併委員會成員簽署確定性摘要：

```
merge_qc_digest = blake2b32(
    "iroha:merge:qc:v1\0" ||
    chain_id ||
    norito(MergeLedgerSignPayload {
        view,
        epoch_id,
        lane_tips,
        merge_hint_roots,
        global_state_root,
    })
)
```

- `view` 是源自車道提示的合併委員會視圖（最大
  `view_change_index` 穿過由入口密封的車道標頭）。
- `chain_id` 是配置的鏈標識符字符串（UTF-8 字節）。
- 有效負載使用 Norito 編碼，字段順序如上所示。

生成的摘要存儲在 `merge_qc.message_digest` 中，並且是消息
由 BLS 簽名驗證。

**合併 QC 構建（必須）：**

- 合併委員會花名冊是當前提交拓撲驗證者集。
- 所需法定人數 = `commit_quorum_from_len(roster_len)`。
- `merge_qc.signers_bitmap` 對參與驗證者索引進行編碼（LSB優先）
  按照提交拓撲順序。
- `merge_qc.aggregate_signature` 是摘要的 BLS 正常聚合
  上面。

**驗證（必須）：**

1. 對照 `lane_tips[i]` 驗證每個 `lane_qc_i` 並確認塊頭
   包括匹配的 `merge_hint_root_i`。
2. 確保沒有 `lane_qc_i` 指向 `Invalid` 或未執行的塊。的
   上面的非空策略確保標頭包含狀態覆蓋。
3. 重新計算 `ReduceMergeHints` 並與 `global_state_root` 進行比較。
4. 重新計算合併QC摘要並驗證簽名者位圖、仲裁閾值、
   並根據提交拓撲名冊聚合簽名。

**可觀察性：** 合併節點發出 Prometheus 計數器
`merge_entry_lane_repeats_total{i}` 突出顯示跳過插槽的通道
運營可見性。

## 3. 最終確定工作流程

### 3.1 車道級最終確定性

1. 事務在確定性時隙中按通道進行調度。
2. 執行器將覆蓋應用到 `StateBlock` 中，生成增量和
文物。
3. 驗證後，車道委員會簽署執行投票原像，
   綁定區塊哈希、狀態根和高度/視圖/紀元。元組
   `(block_hash, lane_qc_i, merge_hint_root_i)` 被認為是車道最終的。
4. 輕客戶端可以將車道提示視為 DS 限制證明的最終結果，但是
必須記錄關聯的 `merge_hint_root` 以與合併分類賬進行核對
稍後。通道委員會是針對每個數據空間的，不會取代全局提交
拓撲。委員會大小固定為 `3f+1`，其中 `f` 來自
數據空間目錄 (`fault_tolerance`)。驗證器池是數據空間的
驗證器（管理員管理的通道或公共通道的通道治理清單
權益選出的車道的權益記錄）。委員會成員是
使用綁定的 VRF 紀元種子每個紀元確定性採樣一次
`dataspace_id` 和 `lane_id`。如果池小於 `3f+1`，則通道最終確定
暫停，直到恢復仲裁（緊急恢復單獨處理）。

### 3.2 合併賬本最終性

1. 合併委員會收集最新的車道提示，驗證每個`lane_qc_i`，並
構造如上定義的 `MergeLedgerEntry`。
2. 驗證確定性約簡後，合併委員會簽署
條目（`merge_qc`）。
3. 節點將條目追加到合併分類賬日誌中，並將其與
車道塊參考。
4. `global_state_root`成為權威的世界國家承諾
紀元/時段。完整節點更新其 WSV 檢查點元數據以反映此情況
價值；確定性重放必須重現相同的減少。

### 3.3 WSV 和存儲集成

- `State::commit_merge_entry` 記錄每通道狀態根和
  最終 `global_state_root`，使用全局校驗和橋接通道執行。
- Kura 堅持 `MergeLedgerEntry` 與車道塊偽影相鄰，因此
  重播可以重建車道級和全局最終性序列。
- 當通道跳過一個槽位時，存儲僅保留前一個提示；不
  創建佔位符合併條目，直到至少一個通道產生新的
  塊。
- API表面（Torii，遙測）公開車道提示和最新合併
  入口，以便運營商和客戶可以協調每車道和全局視圖。

## 4. 實施注意事項- `crates/iroha_core/src/state.rs`：`State::commit_merge_entry` 驗證
  減少並將車道/全局元數據連接到世界狀態中，以便查詢
  觀察者可以訪問合併提示和權威的全局哈希。
- `crates/iroha_core/src/kura.rs`：`Kura::store_block_with_merge_entry` 入隊
  塊並在一步中保留關聯的合併條目，回滾
  追加失敗時內存中的塊，因此存儲永遠不會記錄塊
  沒有其密封元數據。合併賬本日誌被同步修剪
  在啟動恢復期間驗證塊高度，並緩存在內存中
  使用有界窗口（`kura.merge_ledger_cache_capacity`，默認 256）
  避免長時間運行的節點上無限制的增長。恢復截斷部分或
  過大的合併賬本尾部條目，並且附加拒絕上面的條目
  最大有效負載大小保護以限制分配。
- `crates/iroha_core/src/block.rs`：塊驗證拒絕沒有塊
  入口點（外部事務或時間觸發器）並且沒有確定性
  諸如 DA 捆綁包 (`BlockValidationError::EmptyBlock`) 等工件，確保
  在請求和攜帶簽名之前強制執行非空策略
  進入合併分類賬。
- 確定性縮減助手位於合併服務中：`reduce_merge_hint_roots`
  (`crates/iroha_core/src/merge.rs`) 實現了上述 Poseidon2 折疊。
  硬件加速鉤子仍然是未來的工作，但標量路徑現在強制執行
  確定性的規範約簡。
- 遙測集成：公開每車道合併重複和
  `global_state_root` 儀表仍在可觀測性積壓中進行跟踪，因此
  儀表板工作可以與合併服務的推出一起進行。
- 跨組件測試：合併減少的黃金重播覆蓋率是
  與集成測試待辦事項進行跟踪，以確保未來的更改
  `reduce_merge_hint_roots` 保持記錄的根穩定。