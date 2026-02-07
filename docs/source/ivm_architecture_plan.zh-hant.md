---
lang: zh-hant
direction: ltr
source: docs/source/ivm_architecture_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da8a99adbbcf1d8b209a25da32e256c0dad2860633f373d7410a3a91d790c938
source_last_modified: "2026-01-21T19:17:13.236818+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM 架構重構計劃

該計劃抓住了重塑 Iroha 虛擬機的短期里程碑
(IVM) 進入更清晰的層，同時保留安全性和性能特徵。
它側重於隔離職責，使主機集成更安全，以及
準備 Kotodama 語言堆棧以提取到獨立的 crate 中。

## 目標

1. **分層運行時外觀** – 引入顯式運行時接口，以便 VM
   核心可以嵌入到狹窄的特徵後面，並且替代的前端可以發展
   無需接觸內部模塊。
2. **主機/系統調用邊界強化** – 通過路由系統調用調度
   專用適配器，在任何主機之前強制執行 ABI 策略和指針驗證
   代碼執行。
3. **語言/工具分離** – 將 Kotodama 特定代碼移至新 crate 並
   僅將字節碼執行面保留在 `ivm` 中。
4. **配置內聚** – 統一加速和功能切換，以便它們
   通過 `iroha_config` 驅動，消除生產中基於環境的旋鈕
   路徑。

## 階段分解

### 第 1 階段 – 運行時外觀（正在進行中）
- 添加 `runtime` 模塊，該模塊定義描述生命週期的 `VmEngine` 特徵
  操作（`load_program`、`execute`、主機管道）。
- 教 `IVM` 實現該特徵。  這保留了現有的結構，但允許
  消費者（和未來的測試）依賴於接口而不是具體的
  類型。
- 開始放棄從 `lib.rs` 的直接模塊重新導出，以便調用者通過
  盡可能使用外觀。

**安全/性能影響**：外觀限制對內部的直接訪問
狀態；僅暴露安全入口點。  這使得審核主機變得更容易
關於氣體或 TLV 處理的相互作用和原因。

### 第 2 階段 – 系統調用調度程序
- 引入一個 `SyscallDispatcher` 組件，該組件包裝 `IVMHost` 並強制執行 ABI
  策略和指針在一個位置驗證一次。
- 遷移默認主機和模擬主機以使用調度程序，刪除
  重複的驗證邏輯。
- 使調度程序可插拔，以便主機可以提供自定義儀器，而無需
  繞過安全檢查。
- 提供 `SyscallDispatcher::shared(...)` 幫助程序，以便克隆的虛擬機可以轉發
  通過共享 `Arc<Mutex<..>>` 主機進行系統調用，無需每個工作人員構建
  定制包裝紙。

**安全/性能影響**：集中控制可防止主機
忘記調用 `is_syscall_allowed`，它允許將來緩存指針
重複系統調用的驗證。

### 第 3 階段 – Kotodama 提取
- Kotodama 編譯器提取到 `crates/kotodama_lang`（來自 `crates/ivm/src/kotodama`）。
- 提供 VM 使用的最小字節碼 API (`compile_to_ivm_bytecode`)。

**安全/性能影響**：解耦降低了虛擬機的攻擊面
核心並允許語言創新，而不會有解釋器回歸的風險。### 第 4 階段 – 配置整合
- 通過 `iroha_config` 預設的線程加速選項（例如，啟用 GPU 後端），同時保留現有環境覆蓋（`IVM_DISABLE_CUDA`、`IVM_DISABLE_METAL`）作為運行時終止開關。
- 通過新外觀公開 `RuntimeConfig` 對象，以便主機選擇
  明確的確定性加速政策。

**安全/性能影響**：消除基於環境的切換避免靜默
配置漂移並確保跨部署的確定性行為。

## 接下來的步驟

- 通過添加外觀特徵並更新高級調用站點來完成第一階段
  依賴它。
- 審核公共再導出，以確保僅使用外觀和有意公開的 API
  從板條箱中洩漏出來。
- 在單獨的模塊中對系統調用調度程序 API 進行原型設計並遷移
  驗證後默認主機。

一旦實施，每個階段的進展將在 `status.md` 中跟踪
正在進行中。