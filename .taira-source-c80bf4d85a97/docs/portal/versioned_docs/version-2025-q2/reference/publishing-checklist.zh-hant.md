---
lang: zh-hant
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9be80e0138e1e8aa453c703c53069837b24f29f6b463d14c846a01b015918f24
source_last_modified: "2025-12-29T18:16:35.907815+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 發布清單

每當您更新開發者門戶時，請使用此清單。它確保了
CI 構建、GitHub Pages 部署和手動冒煙測試涵蓋了每個部分
在發布或路線圖里程碑落地之前。

## 1. 本地驗證

- `npm run sync-openapi`（當 Torii OpenAPI 更改時）。
- `npm run build` – 確認 `Build on Iroha with confidence` 英雄副本仍然存在
  出現在 `build/index.html` 中。
- `cd build && sha256sum -c checksums.sha256` – 驗證校驗和清單
  生成生成的.
- 抽查您通過 `npm run start` 觸及的降價和實時重新加載
  服務器。

## 2. 拉取請求檢查

- 驗證 `docs-portal-build` 作業在 `.github/workflows/check-docs.yml` 中成功。
- 確認 `ci/check_docs_portal.sh` 運行（CI 日誌顯示英雄煙霧檢查）。
- 確保預覽工作流程上傳清單 (`build/checksums.sha256`) 並
  `sha256sum -c` 在 CI 中通過。
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

將每一行標記為 PR 審核的一部分，或記下任何後續任務，以便了解狀態
跟踪保持準確。

## 4. 發行說明

- 包括 `https://docs.iroha.tech/`（或環境 URL
  從部署作業）在發行說明和狀態更新中。
- 明確指出任何新的或更改的部分，以便下游團隊知道在哪裡
  重新運行他們自己的冒煙測試。