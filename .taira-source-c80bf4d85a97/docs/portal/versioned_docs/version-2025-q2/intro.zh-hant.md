---
lang: zh-hant
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/intro.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e797879d1f77c8cfd62fcc67874d584f6bdeee9395faafe52fc33f26ce2e6a21
source_last_modified: "2025-12-29T18:16:35.904811+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 歡迎來到 SORA Nexus 開發者門戶

SORA Nexus 開發者門戶捆綁了交互式文檔、SDK
Nexus 運算符和 Hyperledger Iroha 的教程和 API 參考
貢獻者。它通過提供實踐指南來補充主要文檔網站
並直接從該存儲庫生成規格。

## 你可以在這裡做什麼

- **學習 Norito** – 從概述和快速入門開始了解
  序列化模型和字節碼工具。
- **Bootstrap SDK** – 立即關注 JavaScript 和 Rust 快速入門；蟒蛇，
  隨著食譜遷移，Swift 和 Android 指南將加入其中。
- **瀏覽 API 參考** – Torii OpenAPI 頁面呈現最新的 REST
  規範和配置錶鍊接回規範的 Markdown
  來源。
- **準備部署** – 操作運行手冊（遙測、結算、Nexus
  覆蓋）正在從 `docs/source/` 移植並將登陸此站點
  遷移工作取得進展。

## 當前狀態

- ✅ Docusaurus v3 腳手架，帶有 Norito 和 SDK 快速入門的實時頁面。
- ✅ Torii OpenAPI 插件連接到 `npm run sync-openapi`。
- ⏳ 從 `docs/source/` 遷移剩餘指南。
- ⏳ 將預覽版本和 linting 添加到文檔 CI 中。

## 參與其中

- 請參閱 `docs/portal/README.md` 以了解本地開發命令（`npm install`、
  `npm run start`、`npm run build`）。
- 內容遷移任務與 `DOCS-*` 路線圖項目一起進行跟踪。
  歡迎貢獻——移植 `docs/source/` 的部分並添加頁面
  到側邊欄。
- 如果添加生成的工件（規格、配置表），請記錄構建
  命令，以便未來的貢獻者可以輕鬆刷新它。