---
lang: zh-hant
direction: ltr
source: docs/source/ivm_isi_kotodama_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3f40329b9968530dea38745b49f7fee4d55aeb461e515e6f97b5b5986cb27e3f
source_last_modified: "2026-01-21T19:17:13.238594+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM ⇄ ISI ⇄ 數據模型 ⇄ Kotodama — 對齊審核

本文檔審核 Iroha 虛擬機 (IVM) 指令集和系統調用表面如何映射到 Iroha 特殊指令 (ISI) 和 `iroha_data_model`，以及 Kotodama 如何編譯到該堆棧中。它確定了當前的差距並提出了具體的改進建議，使這四層能夠確定性地、符合人體工程學地組合在一起。

關於字節碼目標的注意事項：Kotodama 智能合約編譯為 Iroha 虛擬機 (IVM) 字節碼 (`.to`)。他們並不將“risc5”/RISC-V 作為獨立架構。此處引用的任何 RISC-V 類編碼都是 IVM 混合指令格式的一部分，並且仍然是實現細節。

## 範圍和來源
- IVM：`crates/ivm/src/{instruction.rs,ivm.rs,syscalls.rs,host.rs,mock_wsv.rs}` 和 `crates/ivm/docs/*`。
- ISI/數據模型：`crates/iroha_data_model/src/isi/*`、`crates/iroha_core/src/smartcontracts/isi/*` 和文檔 `docs/source/data_model_and_isi_spec.md`。
- Kotodama：`crates/kotodama_lang/src/*`，`crates/ivm/docs/*` 中的文檔。
- 核心集成：`crates/iroha_core/src/{state.rs,executor.rs,smartcontracts/ivm/cache.rs}`。

術語
- “ISI”是指通過執行器改變世界狀態的內置指令類型（例如，RegisterAccount、Mint、Transfer）。
- “系統調用”是指 IVM `SCALL`，具有 8 位數字，委託主機進行賬本操作。

---

## 當前映射（已實現）

### IVM 說明
- 算術、內存、控制流、加密、向量和 ZK 助手在 `instruction.rs` 中定義，並在 `ivm.rs` 中實現。這些是獨立的和確定性的；加速路徑（SIMD/Metal/CUDA）具有 CPU 回退。
- 系統/主機邊界通過 `SCALL`（操作碼 0x60）。數字列在 `syscalls.rs` 中，包括世界操作（註冊/取消註冊域/帳戶/資產、鑄造/銷毀/轉移、角色/權限操作、觸發器）以及幫助程序（`GET_PRIVATE_INPUT`、`COMMIT_OUTPUT`、`GET_MERKLE_PATH` 等）。

### 主機層
- 特徵 `IVMHost::syscall(number, &mut IVM)` 存在於 `host.rs` 中。
- DefaultHost 僅實現非賬本助手（分配、堆增長、輸入/輸出、ZK 證明助手、功能發現）——它不執行世界狀態突變。
- `mock_wsv.rs` 中存在演示 `WsvHost`，它通過寄存器 x10..x13 中的臨時整數→ID 映射，使用 `AccountId`/`AssetDefinitionId` 將資產操作子集（傳輸/鑄造/燃燒）映射到小型內存中 WSV。

### ISI 和數據模型
- 內置 ISI 類型和語義在 `iroha_core::smartcontracts::isi::*` 中實現，並在 `docs/source/data_model_and_isi_spec.md` 中記錄。
- `InstructionBox` 使用具有穩定“wire ID”和 Norito 編碼的註冊表；本機執行調度是核心中的當前代碼路徑。### IVM的核心集成
- `State::execute_trigger(..)` 克隆緩存的 `IVM`，附加 `CoreHost::with_accounts_and_args`，然後調用 `load_program` + `run`。
- `CoreHost` 實現 `IVMHost`：有狀態系統調用通過指針 ABI TLV 佈局進行解碼、映射到內置 ISI (`InstructionBox`) 並排隊。一旦虛擬機返回，主機將這些 ISI 交給常規執行程序，因此權限、不變量、事件和遙測與本機執行保持相同。不接觸 WSV 的幫助程序系統調用仍委託給 `DefaultHost`。
- `executor.rs` 繼續原生運行內置 ISI；將驗證器執行器本身遷移到 IVM 仍然是未來的工作。

### Kotodama → IVM
- 存在前端部分（詞法分析器/解析器/最小語義/IR/regalloc）。
- Codegen (`kotodama::compiler`) 發出 IVM 操作的子集並使用 `SCALL` 進行資產操作：
  - `MintAsset`→設置x10 =帳戶，x11 =資產，x12 =＆NoritoBytes（數字）； `SCALL SYSCALL_MINT_ASSET`。
  - `BurnAsset`/`TransferAsset` 類似（作為 NoritoBytes（數字）指針傳遞的數量）。
- 演示 `koto_*_demo.rs` 展示了使用 `WsvHost` 以及映射到 ID 的整數索引以進行快速測試。

---

## 差距和不匹配

1) 核心主機覆蓋率和奇偶校驗
- 狀態：`CoreHost` 現在存在於核心中，並將許多分類帳系統調用轉換為通過標準路徑執行的 ISI。覆蓋範圍仍然不完整（例如，某些角色/權限/觸發系統調用是存根），並且需要奇偶校驗測試來保證排隊的 ISI 產生與本機執行相同的狀態/事件。

2) 系統調用表面與 ISI/數據模型命名和覆蓋範圍
- NFT：系統調用現在公開與 `iroha_data_model::nft` 對齊的規範 `SYSCALL_NFT_*` 名稱。
- 角色/權限/觸發器：存在系統調用列表，但沒有參考實現或映射表將每個調用與核心中的具體 ISI 聯繫起來。
- 參數/語義：某些系統調用不指定參數編碼（類型 ID 與指針）或氣體語義； ISI 語義定義明確。

3) 用於跨虛擬機/主機邊界傳遞類型化數據的 ABI
- 指針 ABI TLV 現在以 `CoreHost` (`decode_tlv_typed`) 進行解碼，為 ID、元數據和 JSON 負載提供確定性路徑。仍然需要確保每個系統調用記錄預期的指針類型，並確保 Kotodama 發出正確的 TLV（包括策略拒絕類型時的錯誤處理）。

4) 氣體和誤差映射一致性
- IVM 操作碼按操作收取 Gas 費用； CoreHost 現在使用本機 Gas 計劃（包括批量傳輸和供應商 ISI 橋）為 ISI 系統調用返回額外的 Gas，並且 ZK 驗證系統調用重用機密 Gas 計劃。 DefaultHost 仍然保持最低的測試覆蓋成本。
- 錯誤表面不同：IVM 返回 `VMError::{OutOfGas,PermissionDenied,...}`； ISI 返回 `InstructionExecutionError` 類別（`Find`、`Repetition`、`InvariantViolation`、`Math`、`Type`、`Mintability`、`InvalidParameter`）。5) 加速路徑上的確定性
- IVM 矢量/CUDA/Metal 有 CPU 回退，但某些操作仍然保留（`SETVL`、PARBEGIN/PAREND），並且還不是確定性核心的一部分。
- IVM 和節點之間的 Merkle 樹不同（`ivm::merkle_tree` 與 `iroha_crypto::MerkleTree`） - 統一項已出現在 `roadmap.md` 中。

6) Kotodama 語言表面與預期賬本語義
- 編譯器發出一個小子集；大多數語言功能（狀態/結構、觸發器、權限、類型化參數/返回）尚未連接到主機/ISI 模型。
- 沒有能力/效果類型來確保系統調用對於權威機構來說是合法的。

---

## 建議（具體步驟）

### A. 在核心中實現生產 IVM 主機
- 添加實現 `ivm::host::IVMHost` 的 `iroha_core::smartcontracts::ivm::host` 模塊。
- 對於 `ivm::syscalls` 中的每個系統調用：
  - 通過規範的 ABI 解碼參數（參見 B.），構造相應的內置 ISI 或直接調用相同的核心邏輯，針對 `StateTransaction` 執行它，並將錯誤確定性地映射回 IVM 返回代碼。
  - 使用核心中定義的每個系統調用表確定性地充入氣體（如果將來需要，則通過 `SYSCALL_GET_PARAMETER` 暴露給 IVM）。最初，為每次調用從主機返回固定的額外氣體。
- 將 `authority: &AccountId` 和 `&mut StateTransaction` 線程到主機中，以便權限檢查和事件與本機 ISI 相同。
- 更新 `State::execute_trigger(ExecutableRef::Ivm)` 以在 `vm.run()` 之前附加此主機，並返回與 ISI 相同的 `ExecutionStep` 語義（事件已在核心中發出；應驗證一致的行為）。

### B. 為鍵入的值定義確定性 VM/主機 ABI
- 在 VM 端使用 Norito 來獲取結構化參數：
  - 將指針（以 x10..x13 等形式）傳遞到包含 Norito 編碼值（例如 `AccountId`、`AssetDefinitionId`、`Numeric`、`Metadata` 等類型）的 VM 內存區域。
  - 主機通過 `IVM` 內存助手讀取字節並使用 Norito 進行解碼（`iroha_data_model` 已派生 `Encode/Decode`）。
- 在 Kotodama codegen 中添加最少的幫助程序，以將文字 ID 序列化到代碼/常量池中或在內存中準備調用幀。
- 金額為 `Numeric` 並作為 NoritoBytes 指針傳遞；其他復雜類型也通過指針傳遞。
- 將其記錄在 `crates/ivm/docs/calling_convention.md` 中並添加示例。### C. 使系統調用命名和覆蓋範圍與 ISI/數據模型保持一致
- 為了清晰起見，重命名 NFT 相關的系統調用：規範名稱現在遵循 `SYSCALL_NFT_*` 模式（`SYSCALL_NFT_MINT_ASSET`、`SYSCALL_NFT_SET_METADATA` 等）。
- 發布從每個系統調用到核心 ISI 語義的映射表（文檔 + 代碼註釋），包括：
  - 參數（寄存器與指針）、預期先決條件、事件和錯誤映射。
  - 煤氣費。
- 確保每個內置 ISI 都有一個可從 Kotodama 調用的系統調用（域、帳戶、資產、角色/權限、觸發器、參數）。如果 ISI 必須保持特權，請將其記錄下來並通過主機中的權限檢查來強制執行。

### D. 統一錯誤和氣體
- 在主機中添加轉換層：將 `InstructionExecutionError::{Find,Repetition,InvariantViolation,Math,Type,Mintability,InvalidParameter}` 映射到特定的 `VMError` 代碼或擴展結果約定（例如，設置 `x10=0/1` 並使用明確定義的 `VMError::HostRejected { code }`）。
- 在系統調用核心中引入氣體表；將其鏡像到 IVM 文檔中；確保成本在輸入規模上是可預測的並且與平台無關。

### E. 決定論和共享原語
- 完成 Merkle 樹統一（參見路線圖），並刪除/別名 `ivm::merkle_tree` 為 `iroha_crypto`，具有相同的葉子和證明。
- 保留 `SETVL`/PARBEGIN/PAREND` 保留，直到端到端確定性檢查和確定性調度程序策略就位；文檔表明 IVM 今天忽略了這些提示。
- 確保加速路徑產生逐字節相同的輸出；如果不可行，請通過測試來保護功能，以確保 CPU 回退等效性。

### F.Kotodama 編譯器接線
- 將代碼生成擴展到 ID 和復雜參數的規範 ABI (B.)；停止使用整數→ID 演示地圖。
- 使用清晰的名稱添加直接映射到資產（域/帳戶/角色/權限/觸發器）之外的 ISI 系統調用的內置函數。
- 添加編譯時功能檢查和可選的 `permission(...)` 註釋；當靜態證明不可能時，回退到運行時主機錯誤。
- 在 `crates/ivm/tests/kotodama.rs` 中添加單元測試，使用解碼 Norito 參數並改變臨時 WSV 的測試主機端到端編譯和運行小型合約。

### G. 文檔和開發人員人體工程學
- 使用系統調用映射表和 ABI 註釋更新 `docs/source/data_model_and_isi_spec.md`。
- 在 `crates/ivm/docs/` 中添加新文檔“IVM 主機集成指南”，描述如何在真實的 `StateTransaction` 上實現 `IVMHost`。
- 在 `README.md` 和 crate 文檔中澄清 Kotodama 的目標是 IVM `.to` 字節碼，並且系統調用是進入世界狀態的橋樑。

---

## 建議映射表（初稿）

代表性子集 - 在主機實現期間最終確定和擴展。- SYSCALL_REGISTER_DOMAIN(id: ptr DomainId) → ISI 註冊
- SYSCALL_REGISTER_ACCOUNT(id: ptr AccountId) → ISI 註冊
- SYSCALL_REGISTER_ASSET(id: ptr AssetDefinitionId, mintable: u8) → ISI Register
- SYSCALL_MINT_ASSET（賬戶：ptr AccountId，資產：ptr AssetDefinitionId，金額：ptr NoritoBytes（數字））→ ISI Mint
- SYSCALL_BURN_ASSET（賬戶：ptr AccountId，資產：ptr AssetDefinitionId，金額：ptr NoritoBytes（數字））→ ISI Burn
- SYSCALL_TRANSFER_ASSET（從：ptr AccountId，到：ptr AccountId，資產：ptr AssetDefinitionId，金額：ptr NoritoBytes（數字））→ ISI Transfer
- SYSCALL_TRANSFER_V1_BATCH_BEGIN() / SYSCALL_TRANSFER_V1_BATCH_END() → ISI TransferAssetBatch（打開/關閉範圍；通過 `transfer_asset` 降低各個條目）
- SYSCALL_TRANSFER_V1_BATCH_APPLY(&NoritoBytes) → 當合約已經在鏈外序列化條目時提交預編碼批次
- SYSCALL_NFT_MINT_ASSET（id：ptr NftId，所有者：ptr AccountId）→ ISI 註冊
- SYSCALL_NFT_TRANSFER_ASSET（從：ptr AccountId，到：ptr AccountId，id：ptr NftId）→ ISI Transfer
- SYSCALL_NFT_SET_METADATA(id: ptr NftId, 內容: ptr 元數據) → ISI SetKeyValue
- SYSCALL_NFT_BURN_ASSET(id: ptr NftId) → ISI 取消註冊
- SYSCALL_CREATE_ROLE(id: ptr RoleId, 角色: ptr Role) → ISI 註冊
- SYSCALL_GRANT_ROLE（帳戶：ptr AccountId，角色：ptr RoleId）→ ISI Grant
- SYSCALL_REVOKE_ROLE（帳戶：ptr AccountId，角色：ptr RoleId）→ ISI 撤銷
- SYSCALL_SET_PARAMETER（參數：ptr 參數）→ ISI SetParameter

註釋
- “ptr T”表示寄存器中指向 T 的 Norito 編碼字節的指針，存儲在 VM 內存中；主機將其解碼為相應的`iroha_data_model`類型。
- 返回約定：成功設置`x10=1`；失敗會設置 `x10=0`，並可能因致命錯誤而引發 `VMError::HostRejected`。

---

## 風險和推出計劃
- 首先為一個狹窄的集合（資產+帳戶）連接主機並添加有針對性的測試。
- 在主機語義成熟的情況下，保持本機 ISI 執行作為權威路徑；在測試中在“影子模式”下運行兩條路徑，以斷言相同的最終效果和事件。
- 驗證奇偶校驗後，為生產中的 IVM 觸發器啟用 IVM 主機；稍後也考慮通過 IVM 路由常規事務。

---

## 傑出作品
- 最終確定傳遞 Norito 編碼指針 (`crates/ivm/src/kotodama_std.rs`) 的 Kotodama 幫助程序，並通過編譯器 CLI 顯示它們。
- 發布系統調用氣體表（包括輔助系統調用）並保持 CoreHost 執行/測試與其保持一致。
- ✅ 添加了涵蓋指針參數 ABI 的往返 Norito 固定裝置；請參閱 `crates/iroha_data_model/tests/norito_pointer_abi_roundtrip.rs` 了解 CI 中保存的清單和 NFT 指針覆蓋範圍。