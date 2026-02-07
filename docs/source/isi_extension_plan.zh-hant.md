---
lang: zh-hant
direction: ltr
source: docs/source/isi_extension_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f3502fc6de75095282d44ce778b00d1b0d554773de1861d1b92f7dc573dfafa2
source_last_modified: "2025-12-29T18:16:35.969398+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ISI 擴展計劃 (v1)

本說明簽署了新的 Iroha 特別說明的優先順序並捕獲
每條指令在執行之前都有不可協商的不變量。排序匹配
安全性和可操作性風險第一，用戶體驗吞吐量第二。

## 優先級堆棧

1. **RotateAccountSignatory** – 在沒有破壞性遷移的情況下衛生密鑰輪換所需。
2. **DeactivateContractInstance** / **RemoveSmartContractBytes** – 提供確定性合約
   針對受損部署的終止開關和存儲回收。
3. **SetAssetKeyValue** / **RemoveAssetKeyValue** – 將元數據奇偶校驗擴展到具體資產
   餘額，以便可觀察性工具可以標記資產。
4. **BatchMintAsset** / **BatchTransferAsset** – 確定性扇出助手以保持有效負載大小
   虛擬機回退壓力可控。

## 指令不變量

### 設置資產鍵值/刪除資產鍵值
- 重用 `AssetMetadataKey` 命名空間 (`state.rs`)，以便規範 WSV 密鑰保持穩定。
- 對帳戶元數據幫助程序執行相同的 JSON 大小和架構限制。
- 發出 `AssetEvent::MetadataInserted` / `AssetEvent::MetadataRemoved` 以及受影響的 `AssetId`。
- 需要與現有資產元數據編輯相同的權限令牌（定義所有者或
  `CanModifyAssetMetadata` 式補助金）。
- 如果資產記錄丟失則中止（無隱式創建）。

### 旋轉帳戶簽名
- `AccountId` 中籤名者的原子交換，同時保留帳戶元數據和鏈接
  資源（資產、觸發器、角色、權限、待處理事件）。
- 驗證當前簽名者與調用者匹配（或通過顯式令牌委派權限）。
- 如果新的公鑰已支持同一域中的另一個帳戶，則拒絕。
- 更新所有嵌入帳戶 ID 的規範密鑰，並在提交前使緩存失效。
- 發出專用的 `AccountEvent::SignatoryRotated`，其中包含舊/新密鑰，用於審計跟踪。
- 遷移腳手架：引入 `AccountLabel` + `AccountRekeyRecord` （參見 `account::rekey`）
  現有帳戶可以在滾動升級期間映射到穩定標籤，而不會出現哈希中斷。

### 停用ContractInstance
- 刪除或刪除 `(namespace, contract_id)` 綁定，同時保留來源數據
  （誰、何時、原因代碼）用於故障排除。
- 需要與激活相同的治理權限集，並使用策略掛鉤來禁止
  在未經高級批准的情況下停用核心系統名稱空間。
- 當實例已處於非活動狀態時拒絕以保持事件日誌的確定性。
- 發出下游觀察者可以使用的 `ContractInstanceEvent::Deactivated`。### 刪除SmartContractBytes
- 僅當沒有清單或活動實例時才允許 `code_hash` 修剪存儲的字節碼
  參考工件；否則會因描述性錯誤而失敗。
- 權限門鏡註冊（`CanRegisterSmartContractCode`）加上操作員級別
  警衛（例如，`CanManageSmartContractStorage`）。
- 在刪除之前驗證提供的 `code_hash` 與存儲的正文摘要匹配，以避免
  陳舊的手柄。
- 發出帶有哈希值和調用者元數據的 `ContractCodeEvent::Removed`。

### BatchMintAsset / BatchTransferAsset
- 全有或全無語義：要么每個元組都成功，要么指令無方中止
  影響。
- 輸入向量必須是確定性排序的（無隱式排序）並受配置限制
  （`max_batch_isi_items`）。
- 發出每項資產事件，以便下游會計保持一致；批處理上下文是可加的，
  不是替代品。
- 權限檢查重用每個目標的現有單項邏輯（資產所有者、定義所有者、
  或授予的能力）在狀態突變之前。
- 建議訪問集必須聯合所有讀/寫鍵以保持樂觀並發正確。

## 實施腳手架

- 數據模型現在帶有用於平衡元數據的 `SetAssetKeyValue` / `RemoveAssetKeyValue` 支架
  編輯（`transparent.rs`）。
- 執行者訪問者公開佔位符，一旦主機接線登陸，這些佔位符將控制權限
  （`default/mod.rs`）。
- 重新生成密鑰原型類型 (`account::rekey`) 為滾動遷移提供著陸區。
- 世界狀態包括由 `AccountLabel` 鍵控的 `account_rekey_records`，因此我們可以暫存標籤 →
  簽名遷移而不觸及歷史 `AccountId` 編碼。

## IVM 系統調用起草

- `DeactivateContractInstance` / `RemoveSmartContractBytes` 的主機墊片作為
  `SYSCALL_DEACTIVATE_CONTRACT_INSTANCE` (0x43) 和
  `SYSCALL_REMOVE_SMART_CONTRACT_BYTES` (0x44)，兩者都消耗鏡像 Norito TLV
  規範的 ISI 結構。
- 僅在主機處理程序鏡像 `iroha_core` 執行路徑後擴展 `abi_syscall_list()` 以保留
  ABI 哈希在開發過程中保持穩定。
- 更新 Kotodama，系統調用數量穩定後降低；為擴展添加黃金覆蓋
  同時表面。

## 狀態

上述排序和不變量已準備好實施。後續分支應參考
本文檔在連接執行路徑和系統調用暴露時。