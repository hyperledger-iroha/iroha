---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9174f3d9559c8656bbe8062e64fc22d93ad654347409798f29231d09ba1628e6
source_last_modified: "2026-01-05T09:28:11.903551+00:00"
translation_last_reviewed: 2026-02-07
id: orchestrator-ops
title: SoraFS Orchestrator Operations Runbook
sidebar_label: Orchestrator Ops Runbook
description: Step-by-step operational guide for rolling out, monitoring, and rolling back the multi-source orchestrator.
translator: machine-google-reviewed
---

:::注意規範來源
:::

本操作手冊指導 SRE 準備、部署和操作多源獲取編排器。它對開發人員指南進行了補充，提供了針對生產部署進行調整的程序，包括分階段啟用和對等黑名單。

> **另請參閱：** [多源推出運行手冊](./multi-source-rollout.md) 重點關注整個艦隊的推出浪潮和緊急提供商拒絕。在使用本文檔進行日常協調器操作時，請參考它進行治理/分階段協調。

## 1. 飛行前檢查清單

1. **收集提供商的輸入**
   - 目標車隊的最新提供商廣告 (`ProviderAdvertV1`) 和遙測快照。
   - 有效負載計劃 (`plan.json`) 源自測試中的清單。
2. **渲染確定性記分板**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```

   - 驗證 `artifacts/scoreboard.json` 將每個生產提供商列為 `eligible`。
   - 將摘要 JSON 與記分板一起存檔；審核員在驗證變更請求時依賴塊重試計數器。
3. **使用固定裝置進行試運行** — 對 `docs/examples/sorafs_ci_sample/` 中的公共固定裝置執行相同的命令，以確保編排器二進製文件在接觸生產負載之前與預期版本匹配。

## 2. 分階段推出程序

1. **金絲雀階段（≤2個提供商）**
   - 重建記分板並使用 `--max-peers=2` 運行以將協調器限制為一個小子集。
   - 監控：
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - 一旦完整清單獲取的重試率保持在 1% 以下並且沒有提供商累積失敗，則繼續。
2. **斜坡階段（50%提供商）**
   - 增加 `--max-peers` 並使用新的遙測快照重新運行。
   - 堅持每次運行 `--provider-metrics-out` 和 `--chunk-receipts-out`。文物保留≥7天。
3. **全面推出**
   - 刪除 `--max-peers`（或將其設置為完整的合格計數）。
   - 在客戶端部署中啟用協調器模式：通過配置管理系統分發持久記分板和配置 JSON。
   - 更新儀表板以顯示 `sorafs_orchestrator_fetch_duration_ms` p95/p99 並重試每個區域的直方圖。

## 3. 對等黑名單和提升

使用 CLI 的評分策略覆蓋來對不健康的提供商進行分類，而無需等待治理更新。

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```

- `--deny-provider` 從當前會話的考慮中刪除列出的別名。
- `--boost-provider=<alias>=<weight>` 提高了提供商的調度程序權重。這些值會添加到標準化記分板權重中，並且僅適用於本地運行。
- 在事件票證中記錄覆蓋並附加 JSON 輸出，以便所有者團隊在根本問題得到解決後可以協調狀態。

對於永久性更改，請在清除 CLI 覆蓋之前修改源遙測（將違規者標記為受處罰）或使用更新的流預算刷新廣告。

## 4. 故障分類

當獲取失敗時：

1. 在重新運行之前捕獲以下工件：
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. 檢查 `session.summary.json` 中是否有人類可讀的錯誤字符串：
   - `no providers were supplied` → 驗證提供商路徑和廣告。
   - `retry budget exhausted ...` → 增加 `--retry-budget` 或刪除不穩定的對等點。
   - `no compatible providers available ...` → 審核違規提供商的範圍能力元數據。
3. 將提供商名稱與 `sorafs_orchestrator_provider_failures_total` 相關聯，並在指標出現峰值時創建後續票證。
4. 使用 `--scoreboard-json` 和捕獲的遙測數據重放離線獲取，以確定性地重現故障。

## 5.回滾

要恢復 Orchestrator 推出：

2. 刪除所有 `--boost-provider` 覆蓋，以便記分板恢復為中性權重。
3. 繼續抓取 Orchestrator 指標至少一天，以確認沒有正在進行的剩餘提取。

保持嚴格的工件捕獲和分階段部​​署可確保多源編排器可以跨異構提供商群安全運行，同時保持可觀察性和審計要求完好無損。