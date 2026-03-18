---
lang: zh-hant
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c01eb9c61f61a550dcfa542d29beedb5aa6554e5e0fa8f776f72949d8044843c
source_last_modified: "2026-01-05T09:28:11.846525+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-spec
title: Sora Nexus technical specification
description: Full mirror of `docs/source/nexus.md`, covering the architecture and design constraints for the Iroha 3 (Sora Nexus) ledger.
translator: machine-google-reviewed
---

:::注意規範來源
此頁面鏡像 `docs/source/nexus.md`。保持兩個副本對齊，直到翻譯待辦事項到達門戶。
:::

#！ Iroha 3 – Sora Nexus Ledger：技術設計規範

本文檔提出了 Iroha 3 的 Sora Nexus 賬本架構，將 Iroha 2 發展為圍繞數據空間 (DS) 組織的單一全球邏輯統一賬本。數據空間提供強​​大的隱私域（“私有數據空間”）和開放參與（“公共數據空間”）。該設計保留了全球賬本的可組合性，同時確保私有 DS 數據的嚴格隔離和機密性，並通過跨 Kura（塊存儲）和 WSV（世界狀態視圖）的糾刪碼引入了數據可用性擴展。

同一存儲庫構建 Iroha 2（自託管網絡）和 Iroha 3 (SORA Nexus)。執行由
共享 Iroha 虛擬機 (IVM) 和 Kotodama 工具鏈，因此合約和字節碼工件仍然存在
可跨自託管部署和 Nexus 全局分類賬移植。

目標
- 由許多協作驗證器和數據空間組成的一個全局邏輯分類賬。
- 用於許可操作的私有數據空間（例如，CBDC），數據永遠不會離開私有 DS。
- 開放參與的公共數據空間，類似以太坊的免許可訪問。
- 跨數據空間的可組合智能合約，需要獲得訪問私有 DS 資產的明確權限。
- 性能隔離，因此公共活動不會降低私有 DS 內部事務的性能。
- 大規模數據可用性：糾刪碼 Kura 和 WSV 可有效支持無限數據，同時保持私有 DS 數據的私密性。

非目標（初始階段）
- 定義代幣經濟學或驗證者激勵措施；調度和質押策略是可插入的。
- 引入新的 ABI 版本；根據 IVM 策略，使用顯式系統調用和指針 ABI 擴展更改目標 ABI v1。

術語
- Nexus 賬本：通過將數據空間（DS）塊組成單個有序歷史和狀態承諾而形成的全局邏輯賬本。
- 數據空間（DS）：一個有界的執行和存儲域，具有自己的驗證器、治理、隱私類別、DA 策略、配額和費用策略。存在兩個類別：公共 DS 和私有 DS。
- 私有數據空間：許可的驗證器和訪問控制；交易數據和狀態永遠不會離開 DS。只有承諾/元數據是全球錨定的。
- 公共數據空間：無需許可的參與；完整的數據和狀態是公開的。
- 數據空間清單（DS 清單）：聲明 DS 參數（驗證器/QC 密鑰、隱私類別、ISI 策略、DA 參數、保留、配額、ZK 策略、費用）的 Norito 編碼清單。清單哈希錨定在 Nexus 鏈上。除非被覆蓋，否則 DS 仲裁證書使用 ML-DSA-87（Dilithium5 級）作為默認後量子簽名方案。
- 空間目錄：一個全球鏈上目錄合約，用於跟踪 DS 清單、版本和治理/輪換事件，以實現可解決性和審計。
- DSID：數據空間的全局唯一標識符。用於命名所有對象和引用。
- 錨點：包含在 Nexus 鏈中的 DS 塊/標頭的加密承諾，用於將 DS 歷史記錄綁定到全局分類賬中。
- Kura：Iroha 塊存儲。此處通過糾刪碼 blob 存儲和承諾進行擴展。
- WSV：Iroha 世界狀態視圖。此處使用版本化、支持快照、糾刪碼的狀態段進行擴展。
- IVM：用於智能合約執行的 Iroha 虛擬機（Kotodama 字節碼 `.to`）。
 - AIR：代數中間表示。 STARK 式證明的計算代數視圖，將執行描述為具有轉換和邊界約束的基於字段的跟踪。

數據空間模型
- 身份：`DataSpaceId (DSID)` 標識 DS 並命名所有內容。 DS 可以以兩種粒度進行實例化：
  - 域 DS：`ds::domain::<domain_name>` — 執行和狀態範圍僅限於域。
  - 資產 DS：`ds::asset::<domain_name>::<asset_name>` — 執行和狀態範圍僅限於單個資產定義。
  兩種形式並存；事務可以自動觸及多個 DSID。
- 清單生命週期：DS 創建、更新（密鑰輪換、策略更改）和停用均記錄在空間目錄中。每個插槽 DS 工件都會引用最新的清單哈希。
- 類別：公共 DS（開放參與、公共 DA）和私人 DS（許可、機密 DA）。通過清單標誌可以實現混合策略。
- 每個 DS 的策略：ISI 權限、DA 參數 `(k,m)`、加密、保留、配額（每個塊的最小/最大 tx 份額）、ZK/樂觀證明策略、費用。
- 治理：DS 成員資格和驗證者輪換由清單的治理部分定義（鏈上提案、多重簽名或由 Nexus 交易和證明錨定的外部治理）。

能力清單和 UAID
- 通用帳戶：每個參與者都會收到一個跨越所有數據空間的確定性 UAID（`crates/iroha_data_model/src/nexus/manifest.rs` 中的 `UniversalAccountId`）。功能清單 (`AssetPermissionManifest`) 將 UAID 綁定到特定數據空間、激活/到期時期以及範圍為 `dataspace`、`program_id`、`method` 的允許/拒絕 `ManifestEntry` 規則的有序列表`asset`，以及可選的 AMX 角色。拒絕規則總是勝利；評估者發出帶有審核原因的 `ManifestVerdict::Denied` 或帶有匹配津貼元數據的 `Allowed` 撥款。
- 限額：每個允許條目攜帶確定性 `AllowanceWindow` 存儲桶（`PerSlot`、`PerMinute`、`PerDay`）以及可選的 `max_amount`。主機和 SDK 使用相同的 Norito 有效負載，因此跨硬件和 SDK 實現的實施保持相同。
- 審核遙測：只要清單更改狀態，空間目錄就會廣播 `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`)。新的 `SpaceDirectoryEventFilter` 表面允許 Torii/數據事件訂閱者監控 UAID 清單更新、撤銷和拒絕勝利決策，而無需自定義管道。

有關端到端操作員證據、SDK 遷移說明和清單發布清單，請將此部分與通用帳戶指南 (`docs/source/universal_accounts_guide.md`) 進行鏡像。每當 UAID 策略或工具發生變化時，請保持兩個文檔保持一致。

高層架構
1）全局組合層（Nexus鏈）
- 維護 1 秒 Nexus 塊的單一規範排序，以完成跨越一個或多個數據空間 (DS) 的原子事務。每個提交的事務都會更新統一的全局世界狀態（每個 DS 根的向量）。
- 包含最少的元數據加上聚合證明/QC，以確保可組合性、最終性和欺詐檢測（涉及的 DSID、之前/之後的每個 DS 狀態根、DA 承諾、每個 DS 有效性證明以及使用 ML-DSA-87 的 DS 仲裁證書）。不包含私人數據。
- 共識：規模為 22 人（3f+1，f=7）的單一全球流水線 BFT 委員會，通過劃時代的 VRF/權益機制從多達 20 萬潛在驗證者池中選出。 Nexus 委員會對交易進行排序並在 1 秒內完成區塊。

2）數據空間層（公共/私有）
- 執行全局事務的每 DS 片段，更新 DS 本地 WSV，並生成匯總到 1 秒 Nexus 塊中的每塊有效性工件（聚合的每 DS 證明和 DA 承諾）。
- 私有 DS 對授權驗證者之間的靜態數據和動態數據進行加密；只有承諾和 PQ 有效性證明離開 DS。
- 公共 DS 導出完整數據體（通過 DA）和 PQ 有效性證明。

3) 原子跨數據空間事務 (AMX)
- 模型：每一筆用戶交易可能會涉及多個 DS（例如，域 DS 和一個或多個資產 DS）。它在單個 Nexus 塊中原子提交或中止；沒有部分影響。
- 1 秒內準備提交：對於每個候選交易，觸及的 DS 針對同一快照（時隙起始 DS 根）並行執行，並生成每個 DS PQ 有效性證明 (FASTPQ-ISI) 和 DA 承諾。僅當所有必需的 DS 證明都經過驗證並且 DA 證書到達時（≤300 毫秒目標），nexus 委員會才會提交交易；否則，交易將重新安排到下一個時隙。
- 一致性：聲明讀寫集；衝突檢測發生在針對槽開始根的提交時。每個 DS 的無鎖樂觀執行避免了全局停頓；原子性由 Nexus 提交規則強制執行（DS 中的全有或全無）。
- 隱私：私人 DS 僅導出與前/後 DS 根相關的證明/承諾。沒有原始私人數據離開 DS。4) 具有糾刪碼的數據可用性 (DA)
- Kura 將塊體和 WSV 快照存儲為糾刪碼 blob。公共 blob 被廣泛分片；私有 blob 僅存儲在私有 DS 驗證器中，並帶有加密塊。
- DA 承諾記錄在 DS 工件和 Nexus 塊中，從而在不洩露私人內容的情況下實現採樣和恢復保證。

塊和提交結構
- 數據空間證明神器（每 1s 插槽，每 DS）
  - 字段：dsid、slot、pre_state_root、post_state_root、ds_tx_set_hash、kura_da_commitment、wsv_da_commitment、manifest_hash、ds_qc (ML-DSA-87)、ds_validity_proof (FASTPQ-ISI)。
  - 沒有數據體的私有 DS 導出工件；公共 DS 允許通過 DA 檢索屍體。

- Nexus 塊（1s 節奏）
  - 字段：block_number、parent_hash、slot_time、tx_list（涉及 DSID 的原子跨 DS 交易）、ds_artifacts[]、nexus_qc。
  - 功能：完成所有需要 DS 工件驗證的原子事務；一步更新 DS 根的全局世界狀態向量。

共識與調度
- Nexus 鏈共識：單一全局、流水線 BFT（Sumeragi 級），具有 22 節點委員會（3f+1，f=7），目標是 1s 區塊和 1s 最終性。委員會成員是通過 VRF/股權從約 20 萬名候選人中劃時代選出的；輪換可以維持權力下放和抗審查性。
- 數據空間共識：每個 DS 在其驗證器中運行自己的 BFT，以生成每個時隙的工件（證明、DA 承諾、DS QC）。通道中繼委員會的大小使用數據空間 `fault_tolerance` 設置為 `3f+1`，並使用與 `(dataspace_id, lane_id)` 綁定的 VRF 紀元種子從數據空間驗證器池中按紀元確定性地進行採樣。私人 DS 已獲得許可；公共 DS 允許開放活動，但須遵守反女巫政策。全球聯繫委員會保持不變。
- 事務調度：用戶提交聲明觸及的 DSID 和讀寫集的原子事務。 DS在時隙內並行執行；如果所有 DS 工件都經過驗證並且 DA 證書都是及時的（≤300 毫秒），那麼 Nexus 委員會會將交易包含在 1s 區塊中。
- 性能隔離：每個 DS 都有獨立的內存池和執行。每個 DS 配額限制了每個塊可以提交的涉及給定 DS 的事務數量，以避免隊頭阻塞並保護私有 DS 延遲。

數據模型和命名空間
- DS 合格 ID：所有實體（域、帳戶、資產、角色）均符合 `dsid` 資格。示例：`ds::<domain>::account`、`ds::<domain>::asset#precision`。
- 全局引用：全局引用是一個元組 `(dsid, object_id, version_hint)`，可以放置在鏈上的 Nexus 層或 AMX 描述符中以供跨 DS 使用。
- Norito 序列化：所有跨 DS 消息（AMX 描述符、證明）均使用 Norito 編解碼器。生產路徑中沒有使用 serde。

智能合約和 IVM 擴展
- 執行上下文：將 `dsid` 添加到 IVM 執行上下文。 Kotodama 合約始終在特定數據空間內執行。
- 原子交叉 DS 基元：
  - `amx_begin()` / `amx_commit()` 在 IVM 主機中劃分原子多 DS 事務。
  - `amx_touch(dsid, key)` 聲明針對插槽快照根的衝突檢測的讀/寫意圖。
  - `verify_space_proof(dsid, proof, statement)` → 布爾
  - `use_asset_handle(handle, op, amount)` → 結果（僅當策略允許且句柄有效時才允許操作）
- 資產處理和費用：
  - 資產操作由DS的ISI/角色策略授權；費用以 DS 的 Gas 代幣支付。稍後可以添加可選的功能令牌和更豐富的策略（多審批者、速率限制、地理圍欄），而無需更改原子模型。
- 確定性：所有新的系統調用都是純粹且確定性的給定輸入和聲明的 AMX 讀/寫集。沒有隱藏的時間或環境影響。

後量子有效性證明（廣義 ISI）
- FASTPQ‑ISI（PQ，無可信設置）：一種基於哈希的內核化參數，將傳輸設計推廣到所有 ISI 系列，同時針對 GPU 級硬件上的 20k 規模批次進行亞秒級驗證。
  - 運營概況：
    - 生產節點通過 `fastpq_prover::Prover::canonical` 構建證明器，現在始終初始化生產後端；確定性模擬已被刪除。 【crates/fastpq_prover/src/proof.rs:126】
    - `zk.fastpq.execution_mode`（配置）和 `irohad --fastpq-execution-mode` 允許操作員確定性地固定 CPU/GPU 執行，同時觀察者掛鉤記錄隊列的請求/解析/後端三元組審核。 【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:2192】【crates/iroha_telemetry/src/metrics.rs:8887】
- 算術化：
  - KV-Update AIR：將 WSV 視為通過 Poseidon2-SMT 提交的類型化鍵值映射。每個 ISI 都擴展到密鑰（帳戶、資產、角色、域、元數據、供應）上的一小組讀取-檢查-寫入行。
  - 操作碼門控約束：帶有選擇器列的單個 AIR 表強制執行每個 ISI 規則（保護、單調計數器、權限、範圍檢查、有界元數據更新）。
  - 查找參數：權限/角色、資產精度和策略參數的透明哈希提交表避免了嚴重的按位約束。
- 國家承諾和更新：
  - 聚合 SMT 證明：所有觸摸鍵（前/後）均通過使用具有重複數據刪除同級的壓縮前沿來針對 `old_root`/`new_root` 進行驗證。
  - 不變量：全局不變量（例如，每項資產的總供應量）通過效應行和跟踪計數器之間的多重集相等性來強制執行。
- 證明系統：
  - FRI 式多項式承諾 (DEEP-FRI)，具有高數量 (8/16) 和放大 8-16； Poseidon2 哈希；帶有 SHA-2/3 的 Fiat-Shamir 成績單。
  - 可選遞歸：DS 本地遞歸聚合，可根據需要將微批次壓縮為每個槽一個證明。
- 涵蓋的範圍和示例：
  - 資產：轉移、鑄造、銷毀、註冊/註銷資產定義、設置精度（有界）、設置元數據。
  - 帳戶/域：創建/刪除、設置密鑰/閾值、添加/刪除簽名者（僅限狀態；簽名檢查由 DS 驗證器證明，不在 AIR 內部證明）。
  - 角色/權限（ISI）：授予/撤銷角色和權限；通過查找表和單調策略檢查來強制執行。
  - 合同/AMX：AMX 開始/提交標記，能力鑄造/撤銷（如果啟用）；被證明是狀態轉換和政策計數器。
- Out-of-AIR 檢查以保持延遲：
  - 簽名和重加密（例如 ML-DSA 用戶簽名）由 DS 驗證器驗證並在 DS QC 中進行證明；有效性證明僅涵蓋狀態一致性和政策合規性。這可以保持證明的 PQ 和速度。
- 性能目標（示例性 32 核 CPU + 單個現代 GPU）：
  - 具有小按鍵觸摸的 20k 混合 ISI（≤8 個按鍵/ISI）：〜0.4–0.9 秒驗證，〜150–450 KB 驗證，〜5–15 毫秒驗證。
  - 更重的 ISI（更多鍵/豐富約束）：微批量（例如 10×2k）+ 遞歸以保持每個時隙 <1 秒。
- DS 清單配置：
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`、`zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false`（簽名由 DS QC 驗證）
  - `attestation.qc_signature = "ml_dsa_87"`（默認值；必須明確聲明替代方案）
- 後備方案：
  - 複雜/自定義 ISI 可以使用通用 STARK (`zk.policy = "stark_fri_general"`)，通過 QC 證明+對無效證明進行削減，實現延遲證明和 1 秒最終確定。
  - 非 PQ 選項（例如，帶有 KZG 的 Plonk）需要可信設置，默認版本不再支持。

AIR 底漆（適用於 Nexus）
- 執行跟踪：具有寬度（寄存器列）和長度（步長）的矩陣。每一行都是 ISI 處理的一個邏輯步驟；列保存前/後值、選擇器和標誌。
- 限制：
  - 轉換約束：強制執行行到行關係（例如，post_balance = pre_balance − `sel_transfer = 1` 時藉方行的金額）。
  - 邊界約束：將公共 I/O（old_root/new_root、計數器）綁定到第一行/最後一行。
  - 查找/排列：確保針對提交表（權限、資產參數）的成員資格和多重集平等，而無需使用大量位電路。
- 承諾與驗證：
  - 證明者通過基於散列的編碼提交跟踪，並構造有效的 iff 約束成立的低次多項式。
  - 驗證者通過 FRI（基於哈希的後量子）和一些 Merkle 開口檢查低度；成本是步長的對數。
- 示例（轉賬）：寄存器包括 pre_balance、amount、post_balance、nonce 和選擇器。約束強制執行非負/範圍、守恆和隨機數單調性，而聚合的 SMT 多重證明將前/後葉子鏈接到舊/新根。ABI 和系統調用演變 (ABI v1)
- 要添加的系統調用（說明性名稱）：
  - `SYS_AMX_BEGIN`、`SYS_AMX_TOUCH`、`SYS_AMX_COMMIT`、`SYS_VERIFY_SPACE_PROOF`、`SYS_USE_ASSET_HANDLE`。
- 要添加的指針 ABI 類型：
  - `PointerType::DataSpaceId`、`PointerType::AmxDescriptor`、`PointerType::AssetHandle`、`PointerType::ProofBlob`。
- 所需更新：
  - 添加到 `ivm::syscalls::abi_syscall_list()`（保留訂購），按策略進行控制。
  - 將未知號碼映射到主機中的 `VMError::UnknownSyscall`。
  - 更新測試：系統調用列表黃金、ABI 哈希、指針類型 ID 黃金和策略測試。
  - 文檔：`crates/ivm/docs/syscalls.md`、`status.md`、`roadmap.md`。

隱私模型
- 私有數據包含：私有 DS 的交易主體、狀態差異和 WSV 快照永遠不會離開私有驗證器子集。
- 公開曝光：僅導出標頭、DA 承諾和 PQ 有效性證明。
- 可選的 ZK 證明：私有 DS 可以生成 ZK 證明（例如，餘額充足、滿足策略），從而能夠在不洩露內部狀態的情況下實現跨 DS 操作。
- 訪問控制：授權由 DS 內的 ISI/角色策略強制執行。能力令牌是可選的，可以在需要時稍後引入。

性能隔離和 QoS
- 每個 DS 具有獨立的共識、內存池和存儲。
- Nexus 每個 DS 的調度配額，以限制錨點包含時間並避免隊頭阻塞。
- 每個 DS 的合同資源預算（計算/內存/IO），由 IVM 主機強制執行。公共 DS 爭用不能消耗私人 DS 預算。
- 異步跨 DS 調用避免了私有 DS 執行中的長時間同步等待。

數據可用性和存儲設計
1) 糾刪碼
- 使用系統化的 Reed-Solomon（例如 GF(2^16)）對 Kura 塊和 WSV 快照進行 blob 級擦除編碼：參數 `(k, m)` 和 `n = k + m` 分片。
- 默認參數（建議，公共 DS）：`k=32, m=16` (n=48)，能夠通過約 1.5 倍的擴展從最多 16 個分片丟失中恢復。對於私有 DS：許可集中的 `k=16, m=8` (n=24)。兩者均可根據 DS Manifest 進行配置。
- 公共 Blob：分佈在許多 DA 節點/驗證器上的分片，具有基於採樣的可用性檢查。標頭中的 DA 承諾允許輕客戶端進行驗證。
- 私有 Blob：僅在私有 DS 驗證器（或指定託管人）內加密和分發的分片。全球鏈僅承載 DA 承諾（沒有分片位置或密鑰）。

2) 承諾和抽樣
- 對於每個 blob：計算分片上的 Merkle 根並將其包含在 `*_da_commitment` 中。通過避免橢圓曲線承諾來保持 PQ。
- DA 證明者：VRF 抽樣的區域證明者（例如，每個區域 64 名）頒發 ML-DSA-87 證書，證明分片抽樣成功。目標 DA 證明延遲 ≤300 毫秒。 Nexus 委員會驗證證書而不是拉取分片。

3) 庫拉整合
- 區塊將交易主體存儲為帶有 Merkle 承諾的糾刪碼 blob。
- 標頭帶有 blob 承諾；公共 DS 的主體可通過 DA 網絡檢索，私有 DS 的主體可通過專用通道檢索。

4) WSV 集成
- WSV 快照：定期將 DS 狀態檢查點到分塊、糾刪碼快照中，並在標頭中記錄承諾。在快照之間，維護更改日誌。公共快照被廣泛分片；私有快照保留在私有驗證器中。
- 攜帶證明的訪問：合約可以提供（或請求）由快照承諾錨定的狀態證明（Merkle/Verkle）。私人 DS 可以提供零知識證明而不是原始證明。

5）保留和修剪
- 公共 DS 無需修剪：通過 DA（水平縮放）保留所有 Kura 體和 WSV 快照。私人 DS 可以定義內部保留，但導出的承諾仍然不可變。 Nexus 層保留所有 Nexus 塊和 DS 工件承諾。

網絡和節點角色
- 全球驗證者：參與nexus共識，驗證Nexus區塊和DS工件，對公共DS執行DA檢查。
- 數據空間驗證器：運行 DS 共識、執行合約、管理本地 Kura/WSV、為其 DS 處理 DA。
- DA 節點（可選）：存儲/公開公共 blob，方便採樣。對於私有 DS，DA 節點與驗證者或受信任的託管人位於同一地點。

系統級改進和注意事項
- 排序/內存池解耦：採用 DAG 內存池（例如，Narwhal 風格）在連接層提供流水線 BFT，以在不更改邏輯模型的情況下降低延遲並提高吞吐量。
- DS 配額和公平性：每個 DS 每塊配額和權重上限，以避免隊頭阻塞並確保私有 DS 的可預測延遲。
- DS 證明 (PQ)：默認 DS 仲裁證書使用 ML-DSA-87（Dilithium5 級）。這是後量子的，比 EC 簽名更大，但在每個時隙一個 QC 上是可以接受的。 DS 可以明確選擇 ML-DSA-65/44（較小）或 EC 簽名（如果在 DS 清單中聲明）；強烈鼓勵公眾 DS 保留 ML-DSA-87。
- DA 證明者：對於公共 DS，使用頒發 DA 證書的 VRF 抽樣區域證明者。 Nexus 委員會驗證證書而不是原始分片採樣；私有 DS 將 DA 證明保留在內部。
- 遞歸和紀元證明：可以選擇將 DS 中的多個微批次聚合為每個時隙/紀元的一個遞歸證明，以保持證明大小並在高負載下驗證時間穩定。
- 通道擴展（如果需要）：如果單個全局委員會成為瓶頸，則引入具有確定性合併的 K 個並行排序通道。這在水平擴展的同時保留了單一的全局順序。
- 確定性加速：為散列/FFT 提供 SIMD/CUDA 功能門控內核，並具有位精確的 CPU 回退，以保持跨硬件確定性。
- 通道激活閾值（建議）：如果 (a) p95 最終確定性超過 1.2 秒持續超過 3 分鐘，或 (b) 每個區塊佔用率超過 85% 持續超過 5 分鐘，或 (c) 傳入交易速率在持續水平下需要 >1.2 倍的區塊容量，則啟用 2-4 個通道。通道通過 DSID 哈希確定性地存儲交易並合併到連接塊中。

費用和經濟（初始默認）
- Gas 單位：帶有計量計算/IO 的 per‑DS Gas 代幣；費用通過 DS 的原生天然氣資產支付。跨 DS 的轉換是一個應用程序問題。
- 納入優先：跨 DS 的循環賽，每個 DS 配額以保持公平性和 1 秒 SLO；在 DS 內，費用競價可以打破平局。
- 未來：可以在不改變原子性或 PQ 證明設計的情況下探索可選的全球費用市場或 MEV 最小化政策。

跨數據空間工作流程（示例）
1) 用戶提交涉及公共 DSP 和私有 DS S 的 AMX 交易：將資產 X 從 S 轉移到賬戶位於 P 的受益人 B。
2) 在槽內，P 和 S 各自針對槽快照執行其片段。 S驗證授權和可用性，更新其內部狀態，並產生PQ有效性證明和DA承諾（不洩露私人數據）。 P準備相應的狀態更新（例如根據策略在P中鑄造/銷毀/鎖定）及其證明。
3）Nexus委員會驗證DS證明和DA證書；如果兩者都在槽內驗證，則事務在 1s Nexus 塊中以原子方式提交，更新全局世界狀態向量中的兩個 DS 根。
4) 如果任何證明或 DA 證書丟失/無效，交易將中止（無影響），並且客戶端可以重新提交下一個時段。在任何步驟中都沒有私人數據離開 S。

- 安全考慮
- 確定性執行：IVM 系統調用保持確定性；跨 DS 結果由 AMX 提交和最終性驅動，而不是掛鐘或網絡計時。
- 訪問控制：私有 DS 中的 ISI 權限限制誰可以提交交易以及允許執行哪些操作。能力令牌對跨 DS 使用的細粒度權限進行編碼。
- 保密性：私有 DS 數據的端到端加密、僅在授權成員之間存儲的糾刪碼分片、用於外部證明的可選 ZK 證明。
- DoS 抵抗：內存池/共識/存儲層的隔離可防止公共擁塞影響私有 DS 進程。Iroha 組件的更改
- iroha_data_model：引入 `DataSpaceId`、DS 限定標識符、AMX 描述符（讀/寫集）、證明/DA 承諾類型。僅 Norito 序列化。
- ivm：為 AMX 添加系統調用和指針 ABI 類型（`amx_begin`、`amx_commit`、`amx_touch`）和 DA 證明；根據 v1 策略更新 ABI 測試/文檔。
- iroha_core：實現 Nexus 調度程序、空間目錄、AMX 路由/驗證、DS 工件驗證以及 DA 採樣和配額的策略實施。
- 空間目錄和清單加載器：通過 DS 清單解析線程 FMS 端點元數據（和其他公共服務描述符），以便節點在加入數據空間時自動發現本地服務端點。
- kura：具有糾刪碼、承諾、尊重私人/公共政策的檢索 API 的 Blob 存儲。
- WSV：快照、分塊、承諾；證明 API；與 AMX 衝突檢測和驗證集成。
- irohad：節點角色、DA 網絡、私有 DS 成員資格/身份驗證、通過 `iroha_config` 進行配置（生產路徑中沒有環境切換）。

配置和確定性
- 所有運行時行為均通過 `iroha_config` 配置並通過構造函數/主機進行線程化。沒有生產環境切換。
- 硬件加速（SIMD/NEON/METAL/CUDA）是可選的並且具有功能門控；確定性回退必須在硬件上產生相同的結果。
 - 後量子默認：默認情況下，所有 DS 必須使用 PQ 有效性證明 (STARK/FRI) 和 ML-DSA-87 進行 DS QC。替代方案需要明確的 DS 清單聲明和策略批准。

遷移路徑 (Iroha 2 → Iroha 3)
1）在數據模型中引入數據空間限定的ID和關係塊/全局狀態組合；添加功能標誌以在轉換期間保留 Iroha 2 舊模式。
2) 在功能標誌後面實現 Kura/WSV 糾刪碼後端，在早期階段將當前後端保留為默認值。
3) 添加 IVM 系統調用和 AMX（原子多 DS）操作的指針類型；擴展測試和文檔；保留 ABI v1。
4) 提供具有單個公共 DS 和 1s 區塊的最小 Nexus 鏈；然後僅添加第一個私人 DS 試點導出證明/承諾。
5) 通過 DS 本地 FASTPQ-ISI 證明和 DA 證明者擴展到完整的原子跨 DS 交易 (AMX)；跨 DS 啟用 ML-DSA-87 QC。

測試策略
- 數據模型類型、Norito 往返、AMX 系統調用行為和證明編碼/解碼的單元測試。
- IVM 測試新的系統調用和 ABI 黃金。
- 原子跨 DS 事務（正/負）、DA 證明者延遲目標（≤300 毫秒）以及負載下的性能隔離的集成測試。
- DS QC 驗證 (ML-DSA-87)、衝突檢測/中止語義和機密碎片洩漏預防的安全測試。

### NX-18 遙測和運行手冊資產

- **Grafana 板：** `dashboards/grafana/nexus_lanes.json` 現在導出 NX-18 請求的“Nexus Lane Finality & Oracles”儀表板。面板涵蓋 `iroha_slot_duration_ms` 上的 `histogram_quantile()`、`iroha_da_quorum_ratio`、DA 可用性警告 (`sumeragi_da_gate_block_total{reason="missing_local_data"}`)、oracle 價格/陳舊性/TWAP/理髮儀表以及實時 `iroha_settlement_buffer_xor` 緩衝面板，以便操作員可以證明 1s 插槽、DA 和沒有定制查詢的財務 SLO。
- **運行手冊：** `docs/source/runbooks/nexus_lane_finality.md` 記錄了儀表板附帶的待命工作流程（閾值、事件步驟、證據捕獲、混沌演習），實現了 NX-18 中的“發布操作員儀表板/運行​​手冊”要點。
- **遙測助手：**重用現有的 `scripts/telemetry/compare_dashboards.py` 來比較導出的儀表板（防止分段/產品漂移），在 `ci/check_nexus_lane_smoke.sh` 內運行 `scripts/telemetry/nx18_acceptance.py --json-out artifacts/nx18/nx18_acceptance.json <metrics.prom>` 以門控 DA/quorum/oracle/buffer/slot SLO，並在路由跟踪或混沌排練期間調用 `scripts/telemetry/check_nexus_audit_outcome.py`因此，每台 NX‑18 鑽頭都會存檔匹配的 `nexus.audit.outcome` 有效負載。

開放性問題（需要澄清）
1) 交易簽名：決策——最終用戶可以自由選擇其目標 DS 宣傳的任何簽名算法（Ed25519、secp256k1、ML-DSA 等）。主機必須在清單中強制執行多重簽名/曲線功能標誌，提供確定性回退，並記錄混合算法時的延遲影響。傑出：最終確定跨 Torii/SDK 的能力協商流程並更新准入測試。
2）Gas經濟性：每個DS可以以本地代幣計價Gas，而全球結算費用以SORA XOR支付。突出：定義標準轉換路徑（公共通道 DEX 與其他流動性來源）、賬本會計掛鉤以及補貼或零價格交易的 DS 保障措施。
3) DA 證明者：每個區域和閾值的目標數量（例如，64 個採樣，64 個 ML-DSA-87 簽名中的 43 個），以滿足 ≤300 毫秒的要求，同時保持耐久性。我們從第一天起就必須包括哪些區域？
4）默認DA參數：我們建議公共DS `k=32, m=16`和私有DS `k=16, m=8`。您是否想要某些 DS 類別具有更高的冗餘配置文件（例如 `k=30, m=20`）？
5）DS粒度：域名和資產都可以是DS。我們是否應該支持具有可選策略繼承的分層 DS（域 DS 作為資產 DS 的父級），還是在 v1 中保持平坦？
6）重ISI：對於無法產生亞秒級證明的複雜ISI，我們應該（a）拒絕它們，（b）跨塊分割成更小的原子步驟，還是（c）允許使用顯式標誌延遲包含？
7）跨DS衝突：客戶端聲明的讀/寫集是否足夠，或者主機應該為了安全而自動推斷和擴展它（以更多衝突為代價）？

附錄：遵守存儲庫政策
- Norito 用於通過 Norito 幫助程序進行所有有線格式和 JSON 序列化。
- 僅 ABI v1； ABI 策略沒有運行時切換。系統調用和指針類型的添加遵循記錄的演化過程和黃金測試。
- 跨硬件保留確定性；加速是可選的並且是門控的。
- 生產路徑中沒有 serde；生產中沒有基於環境的配置。