---
id: multi-source-rollout
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Multi-Source Client Rollout & Blacklisting Runbook
sidebar_label: Multi-Source Rollout Runbook
description: Operational checklist for staged multi-source rollouts and emergency provider blacklisting.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
:::

## 目的

本操作手冊指導 SRE 和待命工程師完成兩個關鍵工作流程：

1. 在受控波浪中推出多源協調器。
2. 在不破壞現有會話穩定性的情況下將行為不當的提供商列入黑名單或取消優先級。

它假設已經部署了 SF-6 下交付的編排堆棧（`sorafs_orchestrator`、網關塊範圍 API、遙測導出器）。

> **另請參閱：** [Orchestrator Operations Runbook](./orchestrator-ops.md) 深入探討每次運行的過程（記分板捕獲、分階段推出切換、回滾）。在實時更改期間一起使用這兩個參考。

## 1. 飛行前驗證

1. **確認治理投入。 **
   - 所有候選提供商必鬚髮布包含範圍能力有效負載和流預算的 `ProviderAdvertV1` 信封。通過 `/v2/sorafs/providers` 進行驗證並與預期的功能字段進行比較。
   - 在每次金絲雀運行之前，提供延遲/故障率的遙測快照應小於 15 分鐘。
2. **舞台配置。 **
   - 將 Orchestrator JSON 配置保留在分層 `iroha_config` 樹中：

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     使用特定於部署的限制更新 JSON（`max_providers`，重試預算）。將相同的文件提供給暫存/生產，以便差異保持較小。
3. **練習規範裝置。 **
   - 填充清單/令牌環境變量並運行確定性提取：

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     環境變量應包含參與金絲雀的每個提供者的清單有效負載摘要（十六進制）和 Base64 編碼的流令牌。
   - `artifacts/canary.scoreboard.json` 與先前版本的差異。任何新的不合格提供者或體重轉移 >10% 都需要審查。
4. **驗證遙測是否已接線。 **
   - 在 `docs/examples/sorafs_fetch_dashboard.json` 中打開 Grafana 導出。確保在繼續操作之前在暫存中填充 `sorafs_orchestrator_*` 指標。

## 2. 緊急服務提供商黑名單

當提供程序提供損壞的塊、持續超時或未通過合規性檢查時，請遵循此過程。

1. **獲取證據。 **
   - 導出最新的獲取摘要（`--json-out` 輸出）。記錄失敗的塊索引、提供者別名和摘要不匹配。
   - 保存 `telemetry::sorafs.fetch.*` 目標的相關日誌摘錄。
2. **應用立即覆蓋。 **
   - 在分發給編排器的遙測快照中標記受到處罰的提供商（設置 `penalty=true` 或將 `token_health` 限制為 `0`）。下一個記分板構建將自動排除提供商。
   - 對於臨時煙霧測試，將 `--deny-provider gw-alpha` 傳遞到 `sorafs_cli fetch`，以便在不等待遙測傳播的情況下執行故障路徑。
   - 將更新後的遙測/配置包重新部署到受影響的環境（暫存 → 金絲雀 → 生產）。在事件日誌中記錄更改。
3. **驗證覆蓋。 **
   - 重新運行規範夾具獲取。確認記分板將提供商標記為不合格，原因為 `policy_denied`。
   - 檢查 `sorafs_orchestrator_provider_failures_total` 以確保被拒絕的提供商的計數器停止遞增。
4. **升級長期禁令。 **
   - 如果提供商將被阻止超過 24 小時，請提出治理票以輪換或暫停其廣告。在投票通過之前，保留拒絕列表並刷新遙測快照，以便提供商不會重新進入記分板。
5. **回滾協議。 **
   - 要恢復提供程序，請將其從拒絕列表中刪除、重新部署並捕獲新的記分板快照。將更改附加到事件事後分析中。

## 3. 分階段推出計劃

|相|範圍 |所需信號|通過/不通過標準|
|--------|--------------------|------------------|--------------------|
| **實驗室** |專用集成集群|針對夾具有效負載進行手動 CLI 獲取 |所有塊均成功，提供程序失敗計數器保持為 0，重試率 < 5%。 |
| **分期** |全面控制平面升級| Grafana 儀表板已連接；僅警告模式下的警報規則 | `sorafs_orchestrator_active_fetches` 每次測試運行後返回到零；沒有 `warn/critical` 警報觸發。 |
| **金絲雀** | ≤10% 的生產流量 |尋呼機靜音但實時監控遙測 |重試率 < 10%，提供者故障與已知的噪音對等點隔離，延遲直方圖與分段基線 ±20% 匹配。 |
| **全面上市** | 100% 推出 |尋呼機規則有效 | 24 小時內 `NoHealthyProviders` 錯誤為零，重試率穩定，儀表板 SLA 面板呈綠色。 |

對於每個階段：

1. 使用預期的 `max_providers` 更新協調器 JSON 並重試預算。
2. 針對規範夾具和環境中的代表性清單運行 `sorafs_cli fetch` 或 SDK 集成測試套件。
3. 捕獲記分板+摘要工件並將其附加到發布記錄中。
4. 在進入下一階段之前，與值班工程師一起檢查遙測儀表板。

## 4. 可觀察性和事件掛鉤

- **指標：** 確保 Alertmanager 監控 `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` 和 `sorafs_orchestrator_retries_total`。突然的峰值通常意味著提供商在負載下性能下降。
- **日誌：** 將 `telemetry::sorafs.fetch.*` 目標路由到共享日誌聚合器。構建已保存的 `event=complete status=failed` 搜索以加快分類速度。
- **記分板：** 將每個記分板製品保留為長期存儲。 JSON 還可以作為合規性審查和分階段回滾的證據線索。
- **儀表板：** 將規範的 Grafana 板 (`docs/examples/sorafs_fetch_dashboard.json`) 克隆到生產文件夾中，其中包含來自 `docs/examples/sorafs_fetch_alerts.yaml` 的警報規則。

## 5. 溝通和文檔

- 在操作變更日誌中記錄每個拒絕/提升更改，包括時間戳、操作員、原因和相關事件。
- 當提供商權重或重試預算發生變化時通知 SDK 團隊，以符合客戶端期望。
- GA 完成後，使用部署摘要更新 `status.md`，並在發行說明中存檔此 Runbook 參考。