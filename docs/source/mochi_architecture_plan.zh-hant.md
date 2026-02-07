---
lang: zh-hant
direction: ltr
source: docs/source/mochi_architecture_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ffca282b5a2bb2506f46ac2c7a8985ff2f7d10a46bc999a002277956c9f452b0
source_last_modified: "2026-01-05T09:28:12.023255+00:00"
translation_last_reviewed: 2026-02-07
title: MOCHI Architecture Plan
description: High-level design for the MOCHI local-network GUI supervisor.
translator: machine-google-reviewed
---

# MOCHI 架構計劃


## 目標

- 快速引導單點或多點（四節點 BFT）本地網絡。
- 將 `kagami`、`irohad` 和支持二進製文件封裝在友好的 GUI 工作流程中。
- 通過 Torii HTTP/WebSocket 端點顯示實時塊、事件和狀態數據。
- 為交易和 Iroha 特別指令 (ISI) 提供結構化構建器，並進行本地簽名和提交。
- 管理快照、重新生成流程和配置調整，無需手動編輯文件。
- 作為單個跨平台 Rust 二進製文件發布，沒有 webview 或 Docker 依賴性。

## 架構概述

MOCHI 分為兩個主要 crate，位於新的 `/mochi` 目錄中（請參閱
[MOCHI 快速入門](mochi/quickstart.md) 用於構建和使用說明）：

1. `mochi-core`：一個無頭庫，負責配置模板、密鑰和創世材料生成、監督子進程、驅動 Torii 客戶端以及管理文件系統狀態。
2. `mochi-ui-egui`：基於 `egui`/`eframe` 構建的桌面應用程序，用於呈現用戶界面並通過 `mochi-core` API 委託所有編排。

其他前端（例如 Tauri shell）可以稍後連接到 `mochi-core`，而無需重新設計管理程序邏輯。

## 流程模型

- 對等節點作為單獨的 `irohad` 子進程運行。 MOCHI 永遠不會將對等點作為庫進行鏈接，從而避免不穩定的內部 API 並匹配生產部署拓撲。
- 創世和密鑰材料是通過 `kagami` 調用以及用戶提供的輸入（鏈 ID、初始賬戶、資產）創建的。
- 配置文件從 TOML 模板生成，填寫 Torii 和 P2P 端口、存儲路徑、快照設置和可信對等列表。生成的配置存儲在每個網絡工作空間目錄下。
- 主管跟踪進程生命週期，流式傳輸日誌表面的 stdout/stderr，並輪詢 `/status`、`/metrics` 和 `/configuration` 端點的運行狀況。
- 瘦 Torii 客戶端層包裝 HTTP 和 WebSocket 調用，盡可能依賴 Iroha Rust 客戶端包，以避免重新實現 SCALE 編碼/解碼。

## `mochi-core` 支持的用戶流程- **網絡創建嚮導**：選擇單點或四點配置文件，選擇目錄，然後調用 `kagami` 來生成身份和創世。
- **生命週期控制**：啟動、停止、重新啟動對等點；表面實時指標；暴露日誌尾部；切換運行時配置端點（例如日誌級別）。
- **塊和事件流**：訂閱 `/block/stream` 和 `/events`，為 UI 面板存儲內存滾動緩衝區。
- **狀態資源管理器**：運行 Norito 支持的 `/query` 調用以列出域、帳戶、資產和資產定義以及分頁幫助程序和元數據摘要。
- **交易編輯器**：階段鑄造/轉移指令草稿，將其批處理為簽名交易，預覽 Norito 有效負載，通過 `/transaction` 提交，並監控生成的事件；保險庫簽名掛鉤仍然是未來的迭代。
- **快照和重新創世**：編排 Kura 快照導出/導入、擦除存儲並重新生成創世材料以進行快速重置。

## UI 層 (`mochi-ui-egui`)

- 使用 `egui`/`eframe` 傳送單個本機可執行文件，無需外部運行時。
- 佈局包括：
  - **網絡儀表板**，帶有對等卡、運行狀況指示器和快速操作。
  - **塊**面板流式傳輸最近的提交並允許高度搜索。
  - **事件**面板按哈希或帳戶過濾交易狀態。
  - **狀態資源管理器** 域、帳戶、資產和資產定義選項卡，其中包含分頁 Norito 結果以及用於檢查的原始轉儲。
- **Composer** 表單，具有可批量鑄造/傳輸調色板、隊列管理（添加/刪除/清除）、原始 Norito 預覽以及由簽名者庫支持的提交反饋，以便操作員可以在開發者和真實權限之間進行交換。
- **創世與快照**管理視圖。
- **運行時切換和數據目錄快捷方式的設置**。
- UI通過通道訂閱來自`mochi-core`的異步更新；核心公開了一個 `SupervisorHandle`，用於傳輸結構化事件（對等狀態、塊頭、交易更新）。

## 本地開發筆記

- 工作區配置設置 `ZSTD_SYS_USE_PKG_CONFIG=1`，以便 `zstd-sys` 鏈接到主機 `libzstd`，而不是獲取供應商提供的存檔。這使得 pqcrypto 相關的構建（和 MOCHI 測試）在離線或沙盒環境中運行。

## 包裝和分發

- MOCHI 捆綁（或在 `PATH` 上發現）`irohad`、`iroha_cli` 和 `kagami` 二進製文件。
- 使用 `rustls` 進行出站 HTTPS 以避免 OpenSSL 依賴性。
- 將所有生成的工件存儲在專用應用程序數據根（例如，`~/.local/share/mochi` 或等效平台）中，並具有每個網絡子目錄。 GUI 提供“在 Finder/Explorer 中顯示”幫助程序。
- 在啟動對等點之前自動檢測並保留 Torii (8080+) 和 P2P (1337+) 端口以防止衝突。

## 未來的擴展（超出 MVP 的範圍）- 備用前端（Tauri、CLI 無頭模式）共享 `mochi-core`。
- 用於分佈式測試集群的多主機編排。
- 共識內部結構的可視化工具（Sumeragi 輪狀態、八卦計時）。
- 與 CI 管道集成以實現自動化的臨時網絡快照。
- 用於自定義儀表板或特定領域檢查器的插件系統。

## 參考文獻

- [Torii 端點](https://docs.iroha.tech/reference/torii-endpoints.html)
- [對等配置參數](https://docs.iroha.tech/reference/peer-config/params.html)
- [`kagami` 存儲庫文檔](https://github.com/hyperledger-iroha/iroha)
- [Iroha 特別說明](https://iroha-test.readthedocs.io/en/iroha2-dev/references/isi/)