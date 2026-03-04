---
lang: zh-hant
direction: ltr
source: docs/source/error_mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cba8780bcec4ebf562dc9c5725f328b0ea2d9009517efa5b5a504e2fb6be81fe
source_last_modified: "2026-01-11T04:52:11.136647+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 錯誤映射指南

最後更新：2025-08-21

本指南將 Iroha 中的常見故障模式映射到數據模型顯示的穩定錯誤類別。使用它來設計測試並使客戶端錯誤處理可預測。

原則
- 指令和查詢路徑發出結構化枚舉。避免恐慌；盡可能報告特定類別。
- 類別是穩定的，消息可能會演變。客戶端應該根據類別進行匹配，而不是根據自由格式的字符串進行匹配。

類別
-InstructionExecutionError::Find：實體缺失（資產、賬戶、域、NFT、角色、觸發器、權限、公鑰、區塊、交易）。示例：刪除不存在的元數據鍵會產生 Find(MetadataKey)。
-InstructionExecutionError::Repetition：重複註冊或ID衝突。包含指令類型和重複的IdBox。
-InstructionExecutionError::Mintability：違反 Mintability 不變性（`Once` 耗盡兩次、`Limited(n)` 透支或嘗試禁用 `Infinitely`）。示例：鑄造定義為 `Once` 的資產兩次，產生 `Mintability(MintUnmintable)`；配置 `Limited(0)` 會產生 `Mintability(InvalidMintabilityTokens)`。
-InstructionExecutionError::Math：數字域錯誤（溢出、被零除、負值、數量不足）。示例：燃燒超過可用量會產生 Math(NotEnoughQuantity)。
-InstructionExecutionError::InvalidParameter：無效的指令參數或配置（例如，過去的時間觸發）。用於格式錯誤的合約有效負載。
-InstructionExecutionError::Evaluate：指令形狀或類型的DSL/規範不匹配。示例：資產值的錯誤數字規格會產生 Evaluate(Type(AssetNumericSpec(..)))。
-InstructionExecutionError::InvariantViolation：違反不能用其他類別表達的系統不變量。示例：嘗試刪除最後一個簽名者。
-InstructionExecutionError::Query：當指令執行期間查詢失敗時，對 QueryExecutionFail 進行包裝。

查詢執行失敗
- 查找：查詢上下文中缺少實體。
- 轉換：查詢所期望的類型錯誤。
- NotFound：缺少實時查詢游標。
- CursorMismatch / CursorDone：光標協議錯誤。
- FetchSizeTooBig：超出服務器強制限制。
- GasBudgetExceeded：查詢執行超出了gas/具體化預算。
- InvalidSingularParameters：單一查詢不支持參數。
-CapacityLimit：已達到實時查詢存儲容量。

測試技巧
- 更喜歡靠近錯誤根源的單元測試。例如，數據模型測試中可能會產生資產數字規格不匹配。
- 集成測試應涵蓋代表性案例的端到端映射（例如，重複註冊、刪除時丟失密鑰、無所有權轉移）。
- 通過匹配枚舉變體而不是消息子字符串來保持斷言的彈性。