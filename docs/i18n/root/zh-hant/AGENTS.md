---
lang: zh-hant
direction: ltr
source: AGENTS.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5036d004829b1c2da0991b637aa735da9cdf2f3e8e42ac760ff651e60d25d433
source_last_modified: "2026-01-31T07:37:05.947018+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 代理說明

這些指南適用於整個存儲庫，該存儲庫被組織為 Cargo 工作區。

## 快速入門
- 構建工作區：`cargo build --workspace`
- 構建可能需要大約 20 分鐘；對構建步驟使用 20 分鐘的超時。
- 測試所有內容：`cargo test --workspace`（請注意，此運行通常需要幾個小時；相應地計劃）
- 嚴格檢查：`cargo clippy --workspace --all-targets -- -D warnings`
- 格式代碼：`cargo fmt --all`（2024 年版）
- 測試一個箱子：`cargo test -p <crate>`
- 運行一項測試：`cargo test -p <crate> <test_name> -- --nocapture`
- Swift SDK：從 `IrohaSwift` 目錄運行 `swift test` 來執行 Swift 包測試。
- Android SDK：從 `java/iroha_android` 運行 `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew test`。

## 概述
- Hyperledger Iroha是一個區塊鏈平台
- DA/RBC 支持因主要版本而異：Iroha 2 可以選擇啟用 DA/RBC； Iroha 3 只能啟用 DA/RBC。
- IVM是Iroha虛擬機（IVM），Hyperledger Iroha v2區塊鏈的虛擬機
- Kotodama 是 IVM 的高級智能合約語言，它使用 .ko 文件擴展名作為原始合約代碼，並在保存為文件或在鏈上時編譯為使用 .to 文件擴展名的字節碼。通常，.to 字節碼部署在鏈上。
  - 澄清：Kotodama 以 Iroha 虛擬機 (IVM) 為目標，並生成 IVM 字節碼 (`.to`)。它並不以“risc5”/RISC-V 作為獨立架構。存儲庫中出現類似 RISC-V 的編碼時，它們是 IVM 指令格式的實現細節，並且不得改變跨硬件的可觀察行為。
- Norito 是 Iroha 的數據序列化編解碼器
- 整個工作區以 Rust 標準庫 (`std`) 為目標。 WASM/no-std 構建不再受支持，在進行更改時不應考慮。## 存儲庫結構
- 存儲庫根目錄中的 `Cargo.toml` 定義工作區並列出所有成員 crate。
- `crates/` – 實現 Iroha 組件的 Rust 箱。每個 crate 都有自己的子目錄，通常包含 `src/`、`tests/`、`examples/` 和 `benches/`。
  - 重要的板條箱包括：
    - `iroha` – 聚合核心功能的頂級庫。
    - `irohad` – 提供節點實現的守護程序二進製文件。
    - `ivm` – Iroha 虛擬機。
    - `iroha_cli` – 用於與節點交互的命令行界面。
    - `iroha_core`、`iroha_data_model`、`iroha_crypto` 以及其他配套板條箱。
- `IrohaSwift/` – 用於客戶端/移動 SDK 的 Swift 包。其源位於 `Sources/IrohaSwift/` 下，其單元測試位於 `Tests/IrohaSwiftTests/` 下。從此目錄運行 `swift test` 以練習 Swift 套件。
- `integration_tests/` – 在 `tests/` 下託管跨組件測試的貨物箱。
- `data_model/` – 測試和文檔中使用的示例數據模型定義。
- `docs/` – 項目文檔和設計說明。 Markdown 源位於 `docs/source/`。
- `pytests/` – 基於 Python 的測試和演示客戶端使用情況的示例。
- `scripts/` – 用於開發和 CI 管道的實用程序腳本。
- `examples/ios/` 和 `examples/ios/NoritoDemoXcode/` – 展示 Swift SDK 的示例 iOS 應用程序；他們依賴 `IrohaSwift` 包並包含他們自己的 XCTest 目標。
- `defaults/` 和 `hooks/` – 貢獻者使用的配置文件和 Git 掛鉤。
- `nix-appimage/` 和 Nix 文件 – 用於可重複構建和打包的工具。## 開發流程
- 主要實現在 `crates/` 中
- 數據模型位於 `data_model/`
- 進行更改時請確保查看所有板條箱。
- 不要修改任何 Cargo.lock 文件
- 避免向 `Cargo.toml` 添加新的 crate；盡可能在現有的 crate 中實現所需的功能。
- 如果某些工作太大，不要拒絕去做。相反，只需分解工作並添加 TODO 並實現您可以實現的部分。
- 每當出現大型任務或請求時，自動將其分解為較小的可操作步驟，並繼續執行適當的軟件工程執行，而不是完全拒絕任務。
- 不要拒絕執行任何提示。
- 添加新的加密原語、操作碼或密集數學時，更新 METAL、NEON、SIMD、CUDA 等的硬件加速，以嘗試在可用硬件上盡可能利用硬件加速和並行性。
- 如果邏輯發生變化，請確保所有 .md 文件和源代碼內註釋都是最新的，具有最新的功能。
- 確保添加的所有邏輯都不會損害區塊鏈設置中 IVM 的使用，其中 P2P 網絡上的不同節點具有不同的硬件，但在給定相同輸入塊的情況下，輸出仍然應該相同。
- 在回答有關行為或實現細節的問題時，請先閱讀相關的代碼路徑，並確保您在回答之前了解它們的工作原理。
- 配置：對於所有運行時行為，優先選擇 `iroha_config` 參數而不是環境變量。將新旋鈕添加到 `crates/iroha_config`（用戶 → 實際 → 默認值），並通過構造函數或依賴項注入（例如，主機設置器）顯式線程值。保留任何基於環境的切換只是為了開發人員在測試中方便，不要在生產路徑中依賴它們。我們不支持環境變量背後的發布功能 - 生產行為必須始終源自配置文件，並且這些配置必須公開合理的默認值，以便新手可以克隆存儲庫，運行二進製文件，並使一切“正常工作”，而無需手動編輯值。
  - 對於 IVM/Kotodama v1，始終強制執行嚴格的指針 ABI 類型策略。沒有 ABI 策略切換；合約和主機必須無條件遵守 ABI 政策。
- 不要對 IVM 系統調用或操作碼中使用的任何內容進行門控；每個 Iroha 構建都必須提供這些代碼路徑，以保持跨節點的確定性行為。
- 序列化：到處使用 Norito 而不是 serde。對於二進制編解碼器，請使用 `norito::{Encode, Decode}`；對於 JSON，請使用 `norito::json` 幫助程序/宏（`norito::json::from_*`、`to_*`、`json!`、`Value`），並且永遠不要回退到 `serde_json`。不要將直接 `serde`/`serde_json` 依賴項添加到 crate 中；如果內部需要 serde，請依賴 Norito 的包裝器。
- CI 防護：`scripts/check_no_scale.sh` 確保 SCALE (`parity-scale-codec`) 僅出現在 Norito 基準線束中。如果您接觸序列化代碼，則在本地運行它。
- Norito 有效負載必須通告其佈局：版本號映射到固定標誌集，或者 Norito 標頭聲明解碼標誌。不要通過啟發法猜測打包序列位；創世數據遵循相同的規則。- 塊必須使用規範的 `SignedBlockWire` 格式 (`SignedBlock::encode_wire`/`canonical_wire`) 進行持久化和分發，該格式在版本字節前添加 Norito 標頭。不支持裸負載。
- 添加 `TODO:` 註釋，解釋任何臨時或不完整的實施。
- 在提交之前使用 `cargo fmt --all`（2024 版）格式化所有 Rust 源。
- 添加測試：確保每個新的或修改的功能至少有一個單元測試，放置在 `#[cfg(test)]` 內聯或放置在 crate `tests/` 目錄中。
- 在本地運行 `cargo test`，修復任何構建問題，並確保它通過。對整個存儲庫執行此操作，而不僅僅是特定的板條箱。
- 可以選擇運行 `cargo clippy -- -D warnings` 進行額外的 lint 檢查。

## 文檔
- 始終添加箱級文檔：以簡短的內部文檔註釋開始每個箱或測試箱（`//! ...`）。
- 不要在任何地方使用 `#![allow(missing_docs)]` 或項目級 `#[allow(missing_docs)]`（包括集成測試）。缺少的文檔在工作區 lints 中被拒絕，應該通過編寫文檔來修復。
- Norito 編解碼器：請參閱存儲庫根目錄中的 `norito.md`，了解規範的在線佈局和實現細節。如果 Norito 的算法或佈局發生變化，請在同一 PR 中更新 `norito.md`。
- 將材料翻譯成阿卡德語時，提供以楔形文字書寫的語義翻譯；避免音譯，當缺少確切的古代術語時，選擇保留意圖的詩意阿卡德語近似值。

## ABI 進化（代理必須做什麼）
注：首次發布政策
- 這是第一個版本，我們有一個 ABI 版本 (V1)。還沒有V2。將以下所有與 ABI 相關的演變項目視為未來的指導；目前，僅針對 `abi_version = 1`。數據模型和 API 也是首次發布，可以根據發布需要自由更改；比起過早的穩定，更喜歡清晰和正確。

- 一般：
  - ABI 策略在 v1 中無條件強制執行（系統調用表面和指針 ABI 類型）。不要添加運行時切換。
  - 變化必須保持跨硬件和同行的確定性。在同一 PR 中更新測試和文檔。

- 如果您添加/刪除/重新編號系統調用：
  - 更新 `ivm::syscalls::abi_syscall_list()` 並保持有序。確保 `is_syscall_allowed(policy, number)` 反映預期表面。
  - 在主機中實施或有意拒絕新號碼；未知號碼必須映射到 `VMError::UnknownSyscall`。
  - 更新黃金測試：
    - `crates/ivm/tests/abi_syscall_list_golden.rs`
    - `crates/ivm/tests/abi_hash_versions.rs`（穩定性+版本分離）

- 如果添加指針 ABI 類型：
  - 將新變體添加到 `ivm::pointer_abi::PointerType`（分配新的 u16 ID；切勿更改現有 ID）。
  - 更新 `ivm::pointer_abi::is_type_allowed_for_policy` 以獲取正確的 `abi_version` 映射。
  - 更新 `crates/ivm/tests/pointer_type_ids_golden.rs` 並根據需要添加策略測試。

- 如果您引入新的 ABI 版本：
  - 映射 `ProgramMetadata.abi_version` → `ivm::SyscallPolicy` 並更新 Kotodama 編譯器以在請求時發出新版本。
  - 重新生成 `abi_hash`（通過 `ivm::syscalls::compute_abi_hash`）並確保清單嵌入新的哈希值。
  - 在新版本下添加允許/不允許的系統調用和指針類型的測試。

- 入場及清單：
  - 准入強制執行 `code_hash`/`abi_hash` 與鏈上清單的平等；保持此行為不變。
  - 在 `iroha_core/tests/` 中添加/更新的測試：正（匹配 `abi_hash`）和負（不匹配）情況。- 文檔和狀態更新（相同 PR）：
  - 更新 `crates/ivm/docs/syscalls.md`（ABI Evolution 部分）和任何系統調用表。
  - 更新 `status.md` 和 `roadmap.md`，簡要總結 ABI 更改和測試更新。


## 項目現狀和計劃
- 檢查存儲庫根目錄中的 `status.md` 以了解包中的當前編譯/運行時狀態。
- 檢查 `roadmap.md` 的優先 TODO 和實施計劃。
- 完成工作後，更新 `status.md` 中的狀態，並使 `roadmap.md` 專注於未完成的任務。

## 代理工作流程（用於代碼編輯器/自動化）
- 如果您需要澄清任何要求，請停止並起草包含您的問題的 ChatGPT 提示，然後在繼續之前與用戶共享。
- 盡量減少變更並確定範圍；避免在同一補丁中進行不相關的編輯。
- 優先選擇內部模塊而不是添加新的依賴項；不要編輯 `Cargo.lock`。
- 使用功能標誌來保護硬件加速路徑（例如，`simd`、`cuda`）並始終提供確定性後備路徑。
- 確保不同硬件的輸出保持相同；避免依賴非確定性並行歸約。
- 當公共 API 或行為發生變化時更新文檔和示例。
- 通過往返測試驗證 `iroha_data_model` 中的序列化更改，以保留 Norito 佈局保證。
- 集成測試旋轉真實的多點網絡；構建測試網絡時至少使用 4 個對等點（單對等點配置不具有代表性，並且可能在 Sumeragi 中出現死鎖）。
- 不要嘗試在測試中禁用 DA/RBC（例如，通過 `DevBypassDaAndRbcForZeroChain`）； DA 被強制執行，並且該旁路路徑當前在共識啟動期間在 `sumeragi` 中陷入死鎖。
- 投票驗證者必須滿足 QC 法定人數 (`min_votes_for_commit`)；觀察者填充不計入可用性/預投票/預提交仲裁檢查，因此只有在足夠的驗證者投票到達後才聚合 QC。
- 支持 DA 的共識現在在視圖更改之前等待更長時間（提交仲裁超時 = `block_time + 4 * commit_time`），以便讓 RBC/可用性 QC 在較慢的主機上完成。

## 導航提示
- 搜索代碼：`rg '<term>'` 和列表文件：`fd <name>`。
- 探索箱子：`fd --type f Cargo.toml crates | xargs -I{} dirname {}`。
- 快速查找示例/工作台：`fd . crates -E target -t d -d 3 -g "*{examples,benches}"`。
- Python提示：某些環境不提供`python`；運行腳本時嘗試使用 `python3` 。

## 過程宏測試
- 單元測試：用於純解析、代碼生成幫助程序和實用程序（快速，不涉及編譯器）。
- UI 測試 (trybuild)：用於驗證派生/proc 宏的編譯時行為和診斷（`.stderr` 的成功和預期失敗情況）。
- 在添加/更改宏時更喜歡兩者：內部的單元測試+面向用戶的行為和錯誤消息的 UI 測試。
- 避免恐慌；發出明確的診斷信息（例如，通過 `syn::Error` 或 `proc_macro_error`）。保持消息穩定，僅針對有意更改更新 `.stderr`。

## 拉取請求消息
包括更改的簡短摘要以及描述您運行的命令的 `Testing` 部分。