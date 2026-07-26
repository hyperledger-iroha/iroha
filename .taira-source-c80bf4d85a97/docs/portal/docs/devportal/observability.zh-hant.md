---
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/observability.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5f6d17a605e4b90d9bb9cc041055c43b2f1b384fd13f732c0a56e5de5fe78bbd
source_last_modified: "2025-12-29T18:16:35.105743+00:00"
translation_last_reviewed: 2026-02-07
id: observability
title: Portal Observability & Analytics
sidebar_label: Observability
description: Telemetry, release tagging, and verification automation for the developer portal.
translator: machine-google-reviewed
---

DOCS-SORA 路線圖需要分析、綜合探針和斷開的鏈接
每個預覽版本的自動化。本說明記錄了現在的管道
隨門戶一起提供，以便操作員可以進行監控而不會洩漏訪客
數據。

## 發布標籤

- 設置 `DOCS_RELEASE_TAG=<identifier>`（回退到 `GIT_COMMIT` 或 `dev`）
  建設門戶。該值被注入到 `<meta name="sora-release">` 中
  因此探測器和儀表板可以區分部署。
- `npm run build` 發出 `build/release.json` （由
  `scripts/write-checksums.mjs`) 描述標籤、時間戳和可選
  `DOCS_RELEASE_SOURCE`。相同的文件被捆綁到預覽工件中，並且
  由鏈接檢查器報告引用。

## 隱私保護分析

- 將 `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` 配置為
  啟用輕量級跟踪器。有效負載包含`{事件、路徑、區域設置、
  發布，ts}` with no referrer or IP metadata, and `navigator.sendBeacon`
  盡可能使用以避免阻塞導航。
- 使用 `DOCS_ANALYTICS_SAMPLE_RATE` (0–1) 控制採樣。追踪器商店
  最後發送的路徑，並且永遠不會為同一導航發出重複事件。
- 該實現位於 `src/components/AnalyticsTracker.jsx` 中，並且是
  通過 `src/theme/Root.js` 全局安裝。

## 合成探針

- `npm run probe:portal` 針對常見路由發出 GET 請求
  （`/`、`/norito/overview`、`/reference/torii-swagger` 等）並驗證
  `sora-release` 元標記匹配 `--expect-release`（或
  `DOCS_RELEASE_TAG`）。示例：

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

每個路徑都會報告失敗，因此可以輕鬆地在探測成功時控制 CD。

## 斷鍊自動化

- `npm run check:links` 掃描 `build/sitemap.xml`，確保每個條目映射到
  本地文件（檢查 `index.html` 回退），並寫入
  `build/link-report.json` 包含發布元數據、總數、失敗、
  以及 `checksums.sha256` 的 SHA-256 指紋（公開為 `manifest.id`）
  因此每份報告都可以與工件清單聯繫起來。
- 當頁面丟失時，腳本以非零值退出，因此 CI 可以阻止發布
  陳舊或損壞的路線。報告引用了嘗試過的候選路徑，
  這有助於將路由回歸跟踪回文檔樹。

## Grafana 儀表板和警報

- `dashboards/grafana/docs_portal.json` 發布 **文檔門戶發布**
  Grafana 板。它運送以下面板：
  - *網關拒絕 (5m)* 使用 `torii_sorafs_gateway_refusals_total` 範圍
    `profile`/`reason`，以便 SRE 可以檢測不良策略推送或令牌故障。
  - *Alias 緩存刷新結果* 和 *Alias Proof Age p90* 跟踪
    `torii_sorafs_alias_cache_*` 證明在 DNS 削減之前存在新的證據
    結束了。
  - *Pin 註冊表清單計數* 加上 *活動別名計數* 統計鏡像
    PIN 註冊積壓和總別名，以便治理可以審核每個版本。
  - *網關 TLS 到期（小時）* 突出顯示發佈網關的 TLS 的時間
    證書即將到期（警報閾值為 72 小時）。
  - *複製 SLA 結果*和*複製待辦事項*密切關注
    `torii_sorafs_replication_*` 遙測，確保所有副本符合 GA
    發布後欄。
- 使用內置模板變量（`profile`、`reason`）重點關注
  `docs.sora` 發布配置文件或調查所有網關的峰值。
- PagerDuty 路由使用儀表板面板作為證據：名為的警報
  `DocsPortal/GatewayRefusals`、`DocsPortal/AliasCache` 和
  `DocsPortal/TLSExpiry` 當相應系列違反其規定時起火
  閾值。將警報的運行手冊鏈接到此頁面，以便值班工程師可以
  重放確切的 Prometheus 查詢。

## 把它放在一起

1. 在 `npm run build` 期間，設置發布/分析環境變量並
   讓構建後步驟發出 `checksums.sha256`、`release.json` 和
   `link-report.json`。
2. 針對預覽主機名運行 `npm run probe:portal`
   `--expect-release` 連接到同一標籤。保存標準輸出以供發布
   清單。
3. 運行 `npm run check:links` 以快速修復損壞的站點地圖條目和存檔
   生成的 JSON 報告以及預覽工件。 CI 放棄了
   最新報告 `artifacts/docs_portal/link-report.json`，以便治理可以
   直接從構建日誌下載證據包。
4. 將分析端點轉發到您的隱私保護收集器（貌似合理，
   自託管 OTEL 採集等）並確保採樣率記錄在案
   發布以便儀表板正確解釋計數。
5. CI 已通過預覽/部署工作流程連接這些步驟
   （`.github/workflows/docs-portal-preview.yml`，
   `.github/workflows/docs-portal-deploy.yml`），因此本地演練只需要
   涵蓋秘密特定行為。