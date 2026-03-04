---
lang: zh-hant
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3c952f92e009ea4b2ccba55940737889e8e70f506d09899598bd913a2ac68d2d
source_last_modified: "2026-01-22T14:35:36.839904+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-refactor-plan
title: Sora Nexus ledger refactor plan
description: Mirror of `docs/source/nexus_refactor_plan.md`, detailing the phased clean-up work for the Iroha 3 codebase.
translator: machine-google-reviewed
---

:::注意規範來源
此頁面鏡像 `docs/source/nexus_refactor_plan.md`。保持兩個副本對齊，直到多語言版本登陸門戶。
:::

# Sora Nexus 賬本重構計劃

本文檔捕獲了 Sora Nexus Ledger（“Iroha 3”）重構的直接路線圖。它反映了當前存儲庫佈局以及在 genesis/WSV 簿記、Sumeragi 共識、智能合約觸發器、快照查詢、指針 ABI 主機綁定和 Norito 編解碼器中觀察到的回歸。目標是集中在一個連貫的、可測試的架構上，而不是試圖將所有修復都放在一個單一的補丁中。

## 0. 指導原則
- 跨異構硬件保留確定性行為；僅通過具有相同回退功能的選擇加入功能標誌來利用加速。
- Norito 是序列化層。任何狀態/模式更改都必須包括 Norito 編碼/解碼往返測試和夾具更新。
- 配置流經 `iroha_config`（用戶 → 實際 → 默認值）。從生產路徑中刪除臨時環境切換。
- ABI 政策仍然是 V1 並且不可協商。主機必須明確拒絕未知的指針類型/系統調用。
- `cargo test --workspace` 和黃金測試（`ivm`、`norito`、`integration_tests`）仍然是每個里程碑的基線門。

## 1. 存儲庫拓撲快照
- `crates/iroha_core`：Sumeragi 參與者、WSV、創世加載器、管道（查詢、覆蓋、zk 通道）、智能合約主機膠水。
- `crates/iroha_data_model`：鏈上數據和查詢的權威模式。
- `crates/iroha`：CLI、測試、SDK 使用的客戶端 API。
- `crates/iroha_cli`：操作員 CLI，當前鏡像 `iroha` 中的眾多 API。
- `crates/ivm`：Kotodama 字節碼 VM、指針 ABI 主機集成入口點。
- `crates/norito`：帶有 JSON 適配器和 AoS/NCB 後端的序列化編解碼器。
- `integration_tests`：跨組件斷言，涵蓋創世/引導、Sumeragi、觸發器、分頁等。
- 文檔已經概述了 Sora Nexus Ledger 目標（`nexus.md`、`new_pipeline.md`、`ivm.md`），但相對於代碼而言，實現是支離破碎且部分陳舊的。

## 2. 重構支柱和里程碑

### A 階段 – 基礎和可觀察性
1. **WSV 遙測 + 快照**
   - 在查詢、Sumeragi 和 CLI 使用的 `state`（`WorldStateSnapshot` 特徵）中建立規範快照 API。
   - 使用 `scripts/iroha_state_dump.sh` 通過 `iroha state dump --format norito` 生成確定性快照。
2. **創世/引導決定論**
   - 重構創世攝取以流經單個 Norito 支持的管道 (`iroha_core::genesis`)。
  - 添加集成/回歸覆蓋，重放創世加上第一個塊，並在arm64/x86_64上斷言相同的WSV根（在`integration_tests/tests/genesis_replay_determinism.rs`下跟踪）。
3. **跨板條箱固定性測試**
   - 展開 `integration_tests/tests/genesis_json.rs` 以驗證一個工具中的 WSV、管道和 ABI 不變量。
  - 引入一個 `cargo xtask check-shape` 腳手架，該腳手架會因架構漂移而發生恐慌（在 DevEx 工具積壓下跟踪；請參閱 `scripts/xtask/README.md` 操作項）。

### B 階段 – WSV 和查詢界面
1. **狀態存儲交易**
   - 將 `state/storage_transactions.rs` 折疊到事務適配器中，強制執行提交順序和衝突檢測。
   - 單元測試現在驗證資產/世界/觸發器修改在失敗時回滾。
2. **查詢模型重構**
   - 將分頁/光標邏輯移至 `crates/iroha_core/src/query/` 下的可重用組件中。對齊 `iroha_data_model` 中的 Norito 表示。
  - 添加具有確定性排序的觸發器、資產和角色的快照查詢（通過 `crates/iroha_core/tests/snapshot_iterable.rs` 跟踪當前覆蓋範圍）。
3. **快照一致性**
   - 確保 `iroha ledger query` CLI 使用與 Sumeragi/fetchers 相同的快照路徑。
   - CLI 快照回歸測試在 `tests/cli/state_snapshot.rs` 下進行（針對緩慢運行的功能門控）。

### C 階段 – Sumeragi 管道
1. **拓撲和紀元管理**
   - 將 `EpochRosterProvider` 提取到具有 WSV 權益快照支持的實現的特徵中。
  - `WsvEpochRosterAdapter::from_peer_iter` 為工作台/測試提供了一個簡單的模擬友好構造函數。
2. **共識流程簡化**
   - 將 `crates/iroha_core/src/sumeragi/*` 重新組織為模塊：`pacemaker`、`aggregation`、`availability`、`witness`，並在 `consensus` 下共享類型。
  - 用鍵入的 Norito 信封替換臨時消息傳遞，並引入視圖更改屬性測試（在 Sumeragi 消息積壓中跟踪）。
3. **車道/證明集成**
   - 將車道證明與 DA 承諾保持一致，並確保 RBC 門控是統一的。
   - 端到端集成測試 `integration_tests/tests/extra_functional/seven_peer_consistency.rs` 現在驗證啟用 RBC 的路徑。

### D 階段 – 智能合約和 Pointer-ABI 主機
1. **主機邊界審計**
   - 合併指針類型檢查 (`ivm::pointer_abi`) 和主機適配器 (`iroha_core::smartcontracts::ivm::host`)。
   - 指針表期望和主機清單綁定由 `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` 和 `ivm_host_mapping.rs` 涵蓋，它們執行黃金 TLV 映射。
2. **觸發執行沙箱**
   - 重構觸發器以通過通用 `TriggerExecutor` 運行，該 `TriggerExecutor` 強制執行氣體、指針驗證和事件日誌記錄。
  - 添加覆蓋故障路徑的呼叫/時間觸發器回歸測試（通過 `crates/iroha_core/tests/trigger_failure.rs` 跟踪）。
3. **CLI 和客戶端調整**
   - 確保 CLI 操作（`audit`、`gov`、`sumeragi`、`ivm`）依賴於共享的 `iroha` 客戶端功能以避免漂移。
   - CLI JSON 快照測試位於 `tests/cli/json_snapshot.rs` 中；讓它們保持最新，以便核心命令輸出繼續與規範的 JSON 參考相匹配。

### E 階段 – Norito 編解碼器強化
1. **架構註冊表**
   - 在 `crates/norito/src/schema/` 下創建 Norito 架構註冊表，以獲取核心數據類型的規範編碼。
   - 添加了驗證示例有效負載編碼的文檔測試（`norito::schema::SamplePayload`）。
2. **黃金賽程刷新**
   - 一旦重構落地，更新 `crates/norito/tests/*` 黃金裝置以匹配新的 WSV 模式。
   - `scripts/norito_regen.sh` 通過 `norito_regen_goldens` 幫助程序確定性地重新生成 Norito JSON 黃金。
3. **IVM/Norito 集成**
   - 通過 Norito 端到端驗證 Kotodama 清單序列化，確保指針 ABI 元數據一致。
   - `crates/ivm/tests/manifest_roundtrip.rs` 保留清單的 Norito 編碼/解碼奇偶校驗。

## 3. 跨領域關注點
- **測試策略**：每個階段都會促進單元測試→板條箱測試→集成測試。失敗的測試捕獲當前的回歸；新的測試阻止它們重新出現。
- **文檔**：每個階段落地後，更新 `status.md` 並將未完成的項目滾動到 `roadmap.md`，同時修剪已完成的任務。
- **性能基準**：維護 `iroha_core`、`ivm` 和 `norito` 中的現有基準；添加重構後的基線測量以驗證沒有回歸。
- **功能標誌**：僅為需要外部工具鏈的後端保留板條箱級切換（`cuda`、`zk-verify-batch`）。 CPU SIMD 路徑始終在運行時構建和選擇；為不受支持的硬件提供確定性標量回退。

## 4. 下一步立即行動
- A 階段腳手架（快照特徵 + 遙測接線）——查看路線圖更新中的可操作任務。
- 最近對 `sumeragi`、`state` 和 `ivm` 的缺陷審計發現了以下亮點：
  - `sumeragi`：死代碼津貼保護視圖更改證明廣播、VRF 重播狀態和 EMA 遙測導出。這些將一直處於封閉狀態，直到 C 階段的共識流程簡化和通道/證明集成交付成果落地。
  - `state`：`Cell` 清理和遙測路由移至 A 階段 WSV 遙測軌道，而 SoA/並行應用註釋併入 C 階段管道優化待辦事項中。
  - `ivm`：CUDA 切換曝光、包絡驗證和 Halo2/Metal 覆蓋映射到 D 階段主機邊界工作以及橫切 GPU 加速主題；內核保留在專用 GPU 待辦事項上，直到準備就緒。
- 準備跨團隊 RFC，總結該計劃，以便在進行侵入性代碼更改之前簽署。

## 5. 開放性問題
- RBC 在 P1 之後是否應該保持可選狀態，還是對於 Nexus 賬本通道是強制的？需要利益相關者做出決定。
- 我們是否在 P1 中強制執行 DS 可組合性組，或者在車道證明成熟之前將其禁用？
- ML-DSA-87 參數的規範位置是什麼？候選：新的 `crates/fastpq_isi` 箱子（待創建）。

---

_最後更新：2025-09-12_