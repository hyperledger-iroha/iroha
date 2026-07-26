<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: docs/source/isi_extension_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9648381ac7cc1716ffd3c48aca425ed17a6afe1ac73bdeff866ebbbd9147cf68
source_last_modified: "2026-03-30T18:22:55.972718+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# ISI 擴展計劃 (v1)

本說明簽署了新的 Iroha 特別說明的優先順序並捕獲
每條指令在執行之前都有不可協商的不變量。訂購匹配
安全性和可操作性風險第一，使用者體驗吞吐量第二。

## 優先權堆疊

1. **RotateAccountSignatory** – 在沒有破壞性遷移的情況下衛生金鑰輪替所需。
2. **DeactivateContractInstance** / **RemoveSmartContractBytes** – 提供確定性合約
   針對受損部署的終止開關和儲存回收。
3. **SetAssetKeyValue** / **RemoveAssetKeyValue** – 將元資料奇偶校驗擴展到具體資產
   餘額，以便可觀察性工具可以標記資產。
4. **BatchMintAsset** / **BatchTransferAsset** – 確定性扇出助手以保持有效負載大小
   虛擬機器回退壓力可控。

## 指令不變量

### 設定資產鍵值/刪除資產鍵值
- 重複使用 `AssetMetadataKey` 命名空間 (`state.rs`)，以便規範 WSV 金鑰保持穩定。
- 對帳戶元資料幫助程式執行相同的 JSON 大小和架構限制。
- 發出 `AssetEvent::MetadataInserted` / `AssetEvent::MetadataRemoved` 以及受影響的 `AssetId`。
- 需要與現有資產元資料編輯相同的權限代幣（定義擁有者或
  `CanModifyAssetMetadata` 式補助金）。
- 若資產記錄遺失則中止（無隱式建立）。### 旋轉帳號簽名
- `AccountId` 中簽署者的原子交換，同時保留帳戶元資料和鏈接
  資源（資產、觸發器、角色、權限、待處理事件）。
- 驗證目前簽署者與呼叫者相符（或透過明確令牌委派權限）。
- 如果新的公鑰已經支援另一個規範帳戶，則拒絕。
- 更新所有嵌入帳戶 ID 的規範金鑰，並在提交前使快取失效。
- 發出專用的 `AccountEvent::SignatoryRotated`，其中包含舊/新密鑰，用於審計追蹤。
- 遷移鷹架：依賴 `AccountAlias` + `AccountRekeyRecord` （參見 `account::rekey`）
  現有帳戶可以在滾動升級期間保持穩定的別名綁定，而不會出現雜湊中斷。

### 停用ContractInstance
- 刪除或刪除 `(namespace, contract_id)` 綁定，同時保留來源數據
  （誰、何時、原因代碼）用於故障排除。
- 需要與啟動相同的治理權限集，並使用策略掛鉤來禁止
  在未經進階核准的情況下停用核心系統命名空間。
- 當實例已處於非活動狀態時拒絕以保持事件日誌的確定性。
- 發出下游觀察者可以使用的 `ContractInstanceEvent::Deactivated`。### 刪除SmartContractBytes
- 僅當沒有清單或活動實例時才允許 `code_hash` 修剪儲存的字節碼
  參考工件；否則會因描述性錯誤而失敗。
- 權限門鏡註冊（`CanRegisterSmartContractCode`）加上操作員級別
  警衛（例如，`CanManageSmartContractStorage`）。
- 在刪除之前驗證提供的 `code_hash` 與儲存的正文摘要匹配，以避免
  陳舊的手柄。
- 發出帶有雜湊值和呼叫者元資料的 `ContractCodeEvent::Removed`。

### BatchMintAsset / BatchTransferAsset
- 全有或全無語意：要麼每個元組都成功，要麼指令無方中止
  影響。
- 輸入向量必須確定性排序（無隱式排序）並受配置限制
  （`max_batch_isi_items`）。
- 發出每項資產事件，以便下游會計保持一致；批次上下文是可加的，
  不是替代品。
- 權限檢查重複使用每個目標的現有單項邏輯（資產所有者、定義所有者、
  或授予的能力）在狀態突變之前。
- 建議訪問集必須聯合所有讀/寫鍵以保持樂觀並發正確。

## 實作鷹架- 資料模型現在附有用於平衡元資料的 `SetAssetKeyValue` / `RemoveAssetKeyValue` 支架
  編輯（`transparent.rs`）。
- 執行者訪客公開佔位符，一旦主機接線登陸，這些佔位符將控制權限
  （`default/mod.rs`）。
- 重新產生金鑰原型類型 (`account::rekey`) 為捲動遷移提供登陸區。
- 世界狀態包括 `AccountAlias` 鍵控的 `account_rekey_records`，因此我們可以暫存別名 →
  簽名遷移而不觸及歷史 `AccountId` 編碼。

## IVM 系統呼叫起草

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
本文檔在連接執行路徑和系統呼叫暴露時。