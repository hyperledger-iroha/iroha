---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/taikai-monitoring-dashboards.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Taikai Monitoring Dashboards
description: Portal summary of the viewer/cache Grafana boards that back SN13-C evidence
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Taikai 路由清單 (TRM) 準備情況取決於兩個 Grafana 板及其
同伴警報。此頁面反映了以下內容的亮點
`dashboards/grafana/taikai_viewer.json`，`dashboards/grafana/taikai_cache.json`，
和 `dashboards/alerts/taikai_viewer_rules.yml`，以便審閱者可以跟進
無需克隆存儲庫。

## 查看器儀表板 (`taikai_viewer.json`)

- **實時邊緣和延遲：** 面板可視化 p95/p99 延遲直方圖
  （`taikai_ingest_segment_latency_ms`、`taikai_ingest_live_edge_drift_ms`）每
  集群/流。觀察 p99 >900ms 或漂移 >1.5s（觸發
  `TaikaiLiveEdgeDrift` 警報）。
- **段錯誤：** 將 `taikai_ingest_segment_errors_total{reason}` 分解為
  暴露解碼失敗、沿襲重放嘗試或明顯不匹配。
  每當此面板上升到高於 SN13-C 事件時，請附上屏幕截圖
  “警告”帶。
- **查看者和 CEK 健康狀況：** 面板源自 `taikai_viewer_*` 指標跟踪
  CEK 輪換年齡、PQ 保護組合、重新緩衝計數和警報匯總。 CEK
  小組強制執行輪換 SLA，治理在批准新協議之前會對其進行審查
  別名。
- **別名遙測快照：** `/status → telemetry.taikai_alias_rotations`
  桌子直接位於板上，以便操作員可以確認清單摘要
  在附上治理證據之前。

## 緩存儀表板 (`taikai_cache.json`)

- **層壓力：** 面板圖表 `sorafs_taikai_cache_{hot,warm,cold}_occupancy`
  和 `sorafs_taikai_cache_promotions_total`。使用這些來查看是否有 TRM
  輪換使特定層超載。
- **QoS 拒絕：** `sorafs_taikai_qos_denied_total` 在緩存壓力時出現
  強制節流；每當速率偏離零時，就對鑽探日誌進行註釋。
- **出口利用率：** 幫助確認 SoraFS 出口與 Taikai 保持同步
  CMAF 窗口旋轉時的查看器。

## 警報和證據捕獲

- 分頁規則位於 `dashboards/alerts/taikai_viewer_rules.yml` 和映射一中
  與上述面板之一（`TaikaiLiveEdgeDrift`、`TaikaiIngestFailure`、
  `TaikaiCekRotationLag`，證明健康警告）。確保每一次生產
  集群將這些連接到 Alertmanager。
- 演習期間捕獲的快照/屏幕截圖必須存儲在
  `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` 以及假脫機文件和
  `/status` JSON。使用 `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`
  將執行附加到共享鑽取日誌中。
- 當儀表板更改時，將 JSON 文件的 SHA-256 摘要包含在
  門戶 PR 描述，以便審核員可以將託管 Grafana 文件夾與
  回購版本。

## 證據包清單

SN13-C 審查期望每次演習或事件都運送所列的相同物品
在 Taikai 主播操作手冊中。按以下順序捕獲它們，以便捆綁包
準備好進行治理審查：

1.複製最新的`taikai-anchor-request-*.json`，
   `taikai-trm-state-*.json` 和 `taikai-lineage-*.json` 文件來自
   `config.da_ingest.manifest_store_dir/taikai/`。這些線軸文物證明
   哪個路由清單 (TRM) 和沿襲窗口處於活動狀態。幫手
   `cargo xtask taikai-anchor-bundle --spool <dir> --copy-dir <out> --out <out>/anchor_bundle.json [--signing-key <ed25519>]`
   將復制假脫機文件，發出哈希值，並可選擇簽署摘要。
2.記錄`/v2/status`輸出過濾到
   `.telemetry.taikai_alias_rotations[]` 並將其存儲在假脫機文件旁邊。
   審閱者將報告的 `manifest_digest_hex` 和窗口邊界與
   複製的假脫機狀態。
3.導出上面列出的指標的Prometheus快照並截圖
   具有相關集群/流過濾器的查看器/緩存儀表板
   視圖。將原始 JSON/CSV 和屏幕截圖放入 artefact 文件夾中。
4. 包含引用規則的 Alertmanager 事件 ID（如果有）
   `dashboards/alerts/taikai_viewer_rules.yml` 並註意它們是否自動關閉
   一旦病情清除。

將所有內容存儲在 `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` 下，以便鑽取
審計和 SN13-C 治理審查可以獲取單個檔案。

## 練習節奏和記錄

- 每個月第一個星期二 15:00 UTC 進行 Taikai 錨定演習。
  該時間表在 SN13 治理同步之前保持證據新鮮。
- 捕獲上述工件後，將執行附加到共享賬本
  與 `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`。的
  幫助程序發出 `docs/source/sorafs/runbooks-index.md` 所需的 JSON 條目。
- 鏈接 Runbook 索引條目中的存檔工件併升級任何失敗的項目
  通過媒體平台 WG/SRE 在 48 小時內發出警報或儀表板回歸
  頻道。
- 保留練習摘要屏幕截圖集（延遲、漂移、錯誤、CEK 旋轉、
  緩存壓力）與線軸捆綁在一起，以便操作員可以準確地顯示如何
  排練期間儀表板表現良好。

請參閱 [Taikai Anchor Runbook](./taikai-anchor-runbook.md) 了解
完整的 Sev1 程序和證據清單。此頁面僅捕獲
SN13-C 在離開之前需要的儀表板特定指導 🈺。