<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: docs/source/kura_wsv_security_performance_audit_20260219.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 194721ce71f5593cc9e4df6313c6e3aa85c5c3dc0e3efe4a28d0ded968c0584a
source_last_modified: "2026-02-19T08:31:06.766140+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Kura / WSV 安全與效能審計 (2026-02-19)

## 範圍

此次審計涵蓋：

- Kura 持久性與預算路徑：`crates/iroha_core/src/kura.rs`
- 生產 WSV/狀態提交/查詢路徑：`crates/iroha_core/src/state.rs`
- IVM WSV 模擬主機表面（測試/開發範圍）：`crates/ivm/src/mock_wsv.rs`

超出範圍：不相關的板條箱和全系統基準測試重新運行。

## 風險總結

- 嚴重：0
- 高：4
- 中：6
- 低：2

## 調查結果（依嚴重程度排序）

### 高

1. **Kura writer 對 I/O 故障感到恐慌（節點可用性風險）**
- 成分：庫拉
- 類型：安全性 (DoS)、可靠性
- 詳細資訊：寫入器循環在追加/索引/fsync錯誤上發生恐慌，而不是傳回可復原的錯誤，因此瞬時磁碟故障可以終止節點進程。
- 證據：
  - `crates/iroha_core/src/kura.rs:1697`
  - `crates/iroha_core/src/kura.rs:1724`
  - `crates/iroha_core/src/kura.rs:1845`
  - `crates/iroha_core/src/kura.rs:1854`
  - `crates/iroha_core/src/kura.rs:1860`
- 影響：遠端負載+本機磁碟壓力可能會導致崩潰/重啟循環。2. **Kura 逐出在 `block_store` 互斥體下進行完整資料/索引重寫**
- 成分：庫拉
- 類型：效能、可用性
- 詳細資訊：`evict_block_bodies` 在持有 `block_store` 鎖定的同時透過臨時檔案重寫 `blocks.data` 和 `blocks.index`。
- 證據：
  - 鎖定取得：`crates/iroha_core/src/kura.rs:834`
  - 完全重寫循環：`crates/iroha_core/src/kura.rs:921`、`crates/iroha_core/src/kura.rs:942`
  - 原子替換/同步：`crates/iroha_core/src/kura.rs:956`、`crates/iroha_core/src/kura.rs:960`
- 影響：逐出事件可能會長時間拖延大型歷史記錄的寫入/讀取。

3. **狀態提交在繁重的提交工作中保留粗略的 `view_lock`**
- 組件：生產 WSV
- 類型：效能、可用性
- 詳細資訊：區塊提交在提交事務、區塊哈希和世界狀態時保留獨佔的 `view_lock`，從而在大塊下造成讀者飢餓。
- 證據：
  - 鎖定開始：`crates/iroha_core/src/state.rs:17456`
  - 鎖內工作：`crates/iroha_core/src/state.rs:17466`、`crates/iroha_core/src/state.rs:17476`、`crates/iroha_core/src/state.rs:17483`
- 影響：持續大量的提交會降低查詢/共識回應能力。4. **IVM JSON 管理別名允許特權突變，無需呼叫者檢查（測試/開發主機）**
- 元件：IVM WSV 模擬主機
- 類型：安全性（測試/開發環境中的權限升級）
- 詳細資訊：JSON 別名處理程序直接路由到不需要呼叫者範圍的權限令牌的角色/權限/對等突變方法。
- 證據：
  - 管理者別名：`crates/ivm/src/mock_wsv.rs:4274`、`crates/ivm/src/mock_wsv.rs:4371`、`crates/ivm/src/mock_wsv.rs:4448`
  - 非門控突變體：`crates/ivm/src/mock_wsv.rs:1035`、`crates/ivm/src/mock_wsv.rs:1055`、`crates/ivm/src/mock_wsv.rs:855`
  - 文件文檔中的範圍註釋（測試/開發意圖）：`crates/ivm/src/mock_wsv.rs:295`
- 影響：測試合約/工具可以自我提升並使整合工具中的安全假設失效。

### 中等

5. **Kura 預算檢查重新編碼每個排隊上的待處理區塊（每次寫入 O(n)）**
- 成分：庫拉
類型：性能
- 詳細資訊：每個佇列透過迭代掛起區塊並透過規範的線大小路徑序列化每個區塊來重新計算掛起佇列位元組。
- 證據：
  - 佇列掃描：`crates/iroha_core/src/kura.rs:2509`
  - 每塊編碼路徑：`crates/iroha_core/src/kura.rs:2194`、`crates/iroha_core/src/kura.rs:2525`
  - 在佇列預算檢查中呼叫：`crates/iroha_core/src/kura.rs:2580`、`crates/iroha_core/src/kura.rs:2050`
- 影響：積壓情況下寫入吞吐量下降。6. **Kura 預算檢查對每個佇列執行重複的區塊儲存元資料讀取**
- 成分：庫拉
類型：性能
- 詳細資訊：每次檢查都會讀取持久索引計數和檔案長度，同時鎖定 `block_store`。
- 證據：
  - `crates/iroha_core/src/kura.rs:2538`
  - `crates/iroha_core/src/kura.rs:2548`
  - `crates/iroha_core/src/kura.rs:2575`
- 影響：熱隊列路徑上可避免的 I/O/鎖開銷。

7. **Kura 驅逐是從隊列預算路徑內聯觸發的**
- 成分：庫拉
- 類型：效能、可用性
- 細節：入隊路徑可以在接受新區塊之前同步呼叫驅逐。
- 證據：
  - 排隊呼叫鏈：`crates/iroha_core/src/kura.rs:2050`
  - 內嵌驅逐呼叫：`crates/iroha_core/src/kura.rs:2603`
- 影響：接近預算時，交易/區塊攝取的尾部延遲會激增。

8. **`State::view` 可能會在爭用情況下返回而不獲取粗略鎖**
- 組件：生產 WSV
- 類型：一致性/效能權衡
- 詳細資料：在寫入鎖爭用時，`try_read` 後備返回沒有設計粗略保護的視圖。
- 證據：
  - `crates/iroha_core/src/state.rs:14543`
  - `crates/iroha_core/src/state.rs:14545`
  - `crates/iroha_core/src/state.rs:18301`
- 影響：提高了活躍度，但呼叫者必須容忍爭用下較弱的跨組件原子性。9. **`apply_without_execution` 在 DA 遊標前進中使用硬式 `expect`**
- 組件：生產 WSV
- 類型：安全性（透過panic-on-invariant-break進行DoS）、可靠性
- 詳細資訊：如果 DA 遊標前進不變量失敗，則提交的區塊應用路徑會發生混亂。
- 證據：
  - `crates/iroha_core/src/state.rs:17621`
  - `crates/iroha_core/src/state.rs:17625`
- 影響：潛在的驗證/索引錯誤可能會導致節點終止故障。

10. **IVM TLV 發布系統呼叫在分配之前缺少顯式信封大小限制（測試/開發主機）**
- 元件：IVM WSV 模擬主機
- 類型：安全性（記憶體 DoS）、效能
- 詳細資訊：讀取標頭長度，然後指派/複製完整的 TLV 負載，在此路徑中沒有主機級上限。
- 證據：
  - `crates/ivm/src/mock_wsv.rs:3750`
  - `crates/ivm/src/mock_wsv.rs:3755`
  - `crates/ivm/src/mock_wsv.rs:3759`
- 影響：惡意測試有效負載可能會強制進行大量分配。

### 低

11. **Kura通知通道無限制（`std::sync::mpsc::channel`）**
- 成分：庫拉
- 類型：性能/內存衛生
- 詳細資訊：通知通道可以在持續的生產者壓力期間累積冗餘喚醒事件。
- 證據：
  - `crates/iroha_core/src/kura.rs:552`
- 影響：每個事件大小的記憶體成長風險較低，但可以避免。12. **管道邊車隊列在記憶體中是無限的，直到寫入器耗盡**
- 成分：庫拉
- 類型：性能/內存衛生
- 詳細資訊：邊車隊列 `push_back` 沒有明確的上限/背壓。
- 證據：
  - `crates/iroha_core/src/kura.rs:104`
  - `crates/iroha_core/src/kura.rs:3427`
- 影響：長時間寫入延遲期間潛在的記憶體成長。

## 現有測試覆蓋率和差距

### 庫拉

- 現有承保範圍：
  - 儲存預算行為：`store_block_rejects_when_budget_exceeded`、`store_block_rejects_when_pending_blocks_exceed_budget`、`store_block_evicts_when_block_exceeds_budget`（`crates/iroha_core/src/kura.rs:6820`、`crates/iroha_core/src/kura.rs:6949`、`crates/iroha_core/src/kura.rs:6984`）
  - 逐出正確性和補液：`evict_block_bodies_does_not_truncate_unpersisted`、`evicted_block_rehydrates_from_da_store` (`crates/iroha_core/src/kura.rs:8040`、`crates/iroha_core/src/kura.rs:8126`)
- 差距：
  - 沒有錯誤注入覆蓋用於追加/索引/fsync失敗處理而不會出現恐慌
  - 沒有針對大型掛起隊列和排隊預算檢查成本的效能回歸測試
  - 鎖爭用下沒有長期歷史驅逐延遲測試

### 生產 WSV

- 現有承保範圍：
  - 爭用回退行為：`state_view_returns_when_view_lock_held` (`crates/iroha_core/src/state.rs:18293`)
  - 分層後端的鎖定順序安全：`state_commit_does_not_hold_tiered_backend_while_waiting_for_view_lock` (`crates/iroha_core/src/state.rs:18321`)
- 差距：
  - 沒有定量爭用測試斷言在繁重的世界提交下最大可接受的提交保持時間
  - 如果 DA 遊標前進不變量意外中斷，則不會進行無恐慌處理的迴歸測試

### IVM WSV 模擬主機- 現有承保範圍：
  - 權限 JSON 解析器語意與對等解析（`crates/ivm/src/mock_wsv.rs:5234`、`crates/ivm/src/mock_wsv.rs:5332`）
  - 圍繞 TLV 解碼和 JSON 解碼的系統呼叫冒煙測試（`crates/ivm/src/mock_wsv.rs:5962`、`crates/ivm/src/mock_wsv.rs:6078`）
- 差距：
  - 沒有未經授權的管理員別名拒絕測試
  - `INPUT_PUBLISH_TLV` 中沒有超大 TLV 包絡抑制測試
  - 沒有關於檢查點/恢復克隆成本的基準/護欄測試

## 優先修復計劃

### 第 1 階段（高強度強化）

1. 將 Kura writer `panic!` 分支替換為可恢復錯誤傳播 + 健康狀況惡化訊號。
- 目標檔：`crates/iroha_core/src/kura.rs`
- 驗收：
  - 注入的追加/索引/fsync失敗不會驚慌
  - 透過遙測/日誌記錄顯示錯誤，且編寫器仍然可控

2. 為 IVM 模擬主機 TLV 發布和 JSON 信封路徑新增有界信封檢查。
- 目標檔：`crates/ivm/src/mock_wsv.rs`
- 驗收：
  - 在大量分配處理之前拒絕過大的有效負載
  - 新測試涵蓋 TLV 和 JSON 超大情況

3. 對 JSON 管理員別名（或嚴格的僅測試功能標誌和清晰文件後面的門別名）強制執行明確呼叫者權限檢查。
- 目標檔：`crates/ivm/src/mock_wsv.rs`
- 驗收：
  - 未經授權的呼叫者無法透過別名改變角色/權限/對等狀態

### 第 2 階段（熱路徑效能）4.使Kura預算會計增量。
- 將每個入隊的完整掛起佇列重新計算替換為在入隊/保留/刪除時更新的維護計數器。
- 驗收：
  - 用於待處理位元組計算的排隊成本接近 O(1)
  - 回歸基準顯示隨著待定深度的增長穩定的延遲

5. 減少驅逐鎖保持時間。
- 選項：分段壓縮、具有鎖定釋放邊界的分塊複製或具有有界前景阻塞的後台維護模式。
- 驗收：
  - 大歷史驅逐延遲減少，前台操作保持回應

6. 在可行的情況下縮短粗 `view_lock` 關鍵部分。
- 評估分割提交階段或對分階段增量進行快照，以最大限度地減少獨佔保留視窗。
- 驗收：
  - 爭用指標顯示在大量區塊提交下減少了 99p 的保持時間

### 第三階段（運行護欄）

7. 為 Kura writer 和 sidecar 佇列背壓/上限引入有界/合併喚醒訊號。
8. 擴充遙測儀表板：
- `view_lock` 等待/保持分佈
- 逐出持續時間和每次運行回收的位元組數
- 預算檢查排隊延遲

## 建議的測試添加1. `kura_writer_io_failures_do_not_panic`（單元，故障注入）
2. `kura_budget_check_scales_with_pending_depth`（性能回歸）
3. `kura_eviction_does_not_block_reads_beyond_threshold`（整合/性能）
4. `state_commit_view_lock_hold_under_heavy_world_commit`（爭用回歸）
5. `state_apply_without_execution_handles_da_cursor_error_without_panic`（彈性）
6. `mock_wsv_admin_alias_requires_permissions`（安全回歸）
7. `mock_wsv_input_publish_tlv_rejects_oversize`（DoS防護）
8. `mock_wsv_checkpoint_restore_cost_regression`（性能基準）

## 關於範圍和置信度的註釋

- `crates/iroha_core/src/kura.rs` 和 `crates/iroha_core/src/state.rs` 的結果是生產路徑結果。
- 根據文件級文檔，`crates/ivm/src/mock_wsv.rs` 的結果明確為測試/開發主機範圍。
- 此審核本身不需要 ABI 版本控制變更。