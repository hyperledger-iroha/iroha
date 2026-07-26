---
lang: zh-hant
direction: ltr
source: docs/portal/docs/intro.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f775ae297c910da91c6ce97e97ee36fb87f60218fcfb97639ace6eba39f2252
source_last_modified: "2025-12-29T18:16:35.118933+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 歡迎來到 SORA Nexus 開發者門戶

SORA Nexus 開發者門戶捆綁了交互式文檔、SDK
Nexus 運算符和 Hyperledger Iroha 的教程和 API 參考
貢獻者。它通過提供實踐指南來補充主要文檔網站
並直接從該存儲庫生成規格。登陸頁面現在帶有
主題 Norito/SoraFS 入口點、簽名的 OpenAPI 快照和專用
Norito 流媒體參考，以便貢獻者可以找到流媒體控制平面
無需挖掘根規範即可簽訂合同。

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

- ✅ 主題 Docusaurus v3 登陸，刷新版式，漸變驅動
  英雄/卡片，以及包含 Norito 流摘要的資源圖塊。
- ✅ Torii OpenAPI 插件連接到 `npm run sync-openapi`，帶有簽名快照
  由 `buildSecurityHeaders` 強制執行的檢查和 CSP 防護。
- ✅ 在 CI 中運行預覽和探測覆蓋範圍 (`docs-portal-preview.yml` +
  `scripts/portal-probe.mjs`)，現在控制流文檔，SoraFS 快速入門，
  以及發布工件之前的參考清單。
- ✅ Norito、SoraFS 和 SDK 快速入門以及參考部分已在
  側邊欄；來自 `docs/source/` 的新導入（流、編排、運行手冊）
  按其創作方式登陸此處。

## 參與其中

- 請參閱 `docs/portal/README.md` 以了解本地開發命令（`npm install`、
  `npm run start`、`npm run build`）。
- 內容遷移任務與 `DOCS-*` 路線圖項目一起跟踪。
  歡迎貢獻——移植 `docs/source/` 的部分並添加頁面
  到側邊欄。
- 如果添加生成的工件（規格、配置表），請記錄構建
  命令，以便未來的貢獻者可以輕鬆刷新它。