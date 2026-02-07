---
lang: zh-hant
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9d7b44d46ef97c20058221aedf1f0b4a27ba85d204c3be4fe4933da31d9e207
source_last_modified: "2025-12-29T18:16:35.160066+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 發布清單

每當您更新開發者門戶時，請使用此清單。它確保了
CI 構建、GitHub Pages 部署和手動冒煙測試涵蓋了每個部分
在發布或路線圖里程碑落地之前。

## 1. 本地驗證

- `npm run sync-openapi -- --version=current --latest`（添加一個或多個
  當 Torii OpenAPI 更改為凍結快照時，`--mirror=<label>` 進行標記）。
- `npm run build` – 確認 `Build on Iroha with confidence` 英雄副本仍然存在
  出現在 `build/index.html` 中。
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – 驗證
  校驗和清單（測試下載的 CI 時添加 `--descriptor`/`--archive`
  文物）。
- `npm run serve` – 啟動校驗和門控預覽助手來驗證
  在調用 `docusaurus serve` 之前查看清單，因此審閱者永遠不會瀏覽
  未簽名的快照（`serve:verified` 別名保留用於顯式調用）。
- 抽查您通過 `npm run start` 觸及的降價和實時重新加載
  服務器。

## 2. 拉取請求檢查

- 驗證 `docs-portal-build` 作業在 `.github/workflows/check-docs.yml` 中成功。
- 確認 `ci/check_docs_portal.sh` 運行（CI 日誌顯示英雄煙霧檢查）。
- 確保預覽工作流程上傳清單 (`build/checksums.sha256`) 並
  預覽驗證腳本成功（CI 日誌顯示
  `scripts/preview_verify.sh` 輸出）。
- 將已發布的預覽 URL 從 GitHub Pages 環境添加到 PR
  描述。

## 3. 部分簽核

|部分|業主|清單 |
|--------|---------|------------|
|主頁 |開發版本 |英雄副本渲染、快速入門卡鏈接到有效路線、CTA 按鈕解析。 |
| Norito | Norito 工作組 |概述和入門指南引用了最新的 CLI 標誌和 Norito 架構文檔。 |
| SoraFS |存儲團隊|快速啟動運行完成，記錄清單報告字段，驗證獲取模擬指令。 |
| SDK 指南 | SDK 線索 | Rust/Python/JS 指南編譯當前示例並鏈接到實時存儲庫。 |
|參考|文檔/開發版本 |索引列出了最新規格，Norito 編解碼器參考與 `norito.md` 匹配。 |
|預覽神器|文檔/開發版本 | `docs-portal-preview` 工件附加到 PR、煙霧檢查通過、與審閱者共享的鏈接。 |
|安全與嘗試沙箱 |文檔/DevRel · 安全 |配置 OAuth 設備代碼登錄 (`DOCS_OAUTH_*`)、執行 `security-hardening.md` 檢查表、通過 `npm run build` 或 `npm run probe:portal` 驗證 CSP/受信任類型標頭。 |

將每一行標記為 PR 審核的一部分，或記下任何後續任務，以便了解狀態
跟踪保持準確。

## 4. 發行說明

- 包括 `https://docs.iroha.tech/`（或環境 URL
  從部署作業）在發行說明和狀態更新中。
- 明確指出任何新的或更改的部分，以便下游團隊知道在哪裡
  重新運行他們自己的冒煙測試。