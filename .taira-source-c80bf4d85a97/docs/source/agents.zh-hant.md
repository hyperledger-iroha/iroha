---
lang: zh-hant
direction: ltr
source: docs/source/agents.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7f35a28d00188a3e1f3db76b56e6b29c708dbb75afa3dd009d416b7cd4314754
source_last_modified: "2025-12-29T18:16:35.916241+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 自動化代理執行指南

本頁總結了任何自動化代理的操作護欄
在 Hyperledger Iroha 工作空間內工作。它反映了規範
`AGENTS.md` 指南和路線圖參考，以便構建、文檔和
遙測變化看起來都是一樣的，無論它們是由人類還是由人類產生的
自動貢獻者。

每個任務都期望獲得確定性代碼以及匹配的文檔、測試、
和操作證據。將以下部分作為準備之前的參考
觸摸 `roadmap.md` 項目或回答行為問題。

## 快速啟動命令

|行動|命令 |
|--------|---------|
|構建工作區 | `cargo build --workspace` |
|運行完整的測試套件 | `cargo test --workspace` *（通常需要幾個小時）* |
|運行 Clippy 並顯示默認拒絕警告 | `cargo clippy --workspace --all-targets -- -D warnings` |
|格式化 Rust 代碼 | `cargo fmt --all` *（2024 年版）* |
|測試單個板條箱 | `cargo test -p <crate>` |
|運行一項測試 | `cargo test -p <crate> <test_name> -- --nocapture` |
| Swift SDK 測試 |從 `IrohaSwift/` 開始，運行 `swift test` |

## 工作流程基礎知識

- 在回答問題或更改邏輯之前閱讀相關代碼路徑。
- 將大型路線圖項目分解為易於處理的提交；永遠不要直接拒絕工作。
- 留在現有的工作區成員資格內，重複使用內部板條箱，並執行以下操作：
  **不**改變 `Cargo.lock` 除非明確指示。
- 僅在硬件要求的情況下使用功能標誌和功能切換
  加速器；在每個平台上保持確定性回退可用。
- 更新文檔和 Markdown 參考以及任何功能更改
  所以文檔總是描述當前的行為。
- 為每個新的或修改的功能添加至少一個單元測試。更喜歡內聯
  `#[cfg(test)]` 模塊或板條箱的 `tests/` 文件夾，具體取決於範圍。
- 完成工作後，更新 `status.md` 並附上簡短的摘要和參考
  相關文件；讓 `roadmap.md` 專注於仍需要工作的項目。

## 實施護欄

### 序列化和數據模型
- 到處使用 Norito 編解碼器（通過 `norito::{Encode, Decode}` 進行二進制編碼，
  JSON 通過 `norito::json::*`）。不要添加直接的 serde/`serde_json` 用法。
- Norito 有效負載必須通告其佈局（版本字節或標頭標誌），
  新格式需要相應的文檔更新（例如，
  `norito.md`、`docs/source/da/*.md`）。
- 創世數據、清單和網絡有效負載應保持確定性
  因此具有相同輸入的兩個對等點會產生相同的哈希值。

### 配置和運行時行為
- 與新的環境變量相比，更喜歡位於 `crates/iroha_config` 中的旋鈕。
  通過構造函數或依賴項注入顯式線程化值。
- 切勿對 IVM 系統調用或操作碼行為進行門控 - ABI v1 隨處可見。
- 添加新的配置選項時，更新默認值、文檔和任何相關的
  模板（`peer.template.toml`、`docs/source/configuration*.md` 等）。### ABI、系統調用和指針類型
- 將 ABI 政策視為無條件。添加/刪除系統調用或指針類型
  需要更新：
  - `ivm::syscalls::abi_syscall_list` 和 `crates/ivm/tests/abi_syscall_list_golden.rs`
  - `ivm::pointer_abi::PointerType` 加上黃金測試
  - 每當 ABI 哈希發生變化時，`crates/ivm/tests/abi_hash_versions.rs`
- 未知的系統調用必須映射到 `VMError::UnknownSyscall`，並且清單必須
  在入學考試中保留簽署的 `abi_hash` 平等檢查。

### 硬件加速和確定性
- 新的加密原語或繁重的數學必須通過硬件加速來實現
  路徑（METAL/NEON/SIMD/CUDA），同時保持確定性回退。
- 避免非確定性並行歸約；優先級是相同的輸出
  即使硬件不同，每個對等點也是如此。
- 保持 Norito 和 FASTPQ 裝置的可重複性，以便 SRE 可以審核整個車隊
  遙測。

### 文件和證據
- 鏡像門戶中任何面向公眾的文檔更改 (`docs/portal/...`)
  適用，以便文檔網站與 Markdown 源保持最新。
- 引入新工作流程時，添加運行手冊、治理說明或
  檢查表解釋如何排練、回滾和捕獲證據。
- 將內容翻譯成阿卡德語時，提供書面的語義渲染
  用楔形文字而不是音譯。

### 測試和工具期望
- 在本地運行相關測試套件（`cargo test`、`swift test`、
  集成工具）並在 PR 測試部分記錄命令。
- 使 CI 防護腳本 (`ci/*.sh`) 和儀表板與新遙測保持同步。
- 對於 proc-macros，將單元測試與 `trybuild` UI 測試配對以鎖定診斷。

## 準備發貨清單

1. 代碼編譯，`cargo fmt` 沒有產生差異。
2.更新的文檔（工作區 Markdown 加上門戶鏡像）描述了新的內容
   行為、新的 CLI 標誌或配置旋鈕。
3. 測試覆蓋每一個新的代碼路徑，並且在回歸時會確定性地失敗
   出現。
4. 遙測、儀表板和警報定義引用任何新指標或
   錯誤代碼。
5. `status.md` 包括引用相關文件的簡短摘要以及
   路線圖部分。

遵循此清單可以使路線圖執行可審核並確保每個
代理提供其他團隊可以信任的證據。