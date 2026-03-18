---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c14c7c9e998078f3f713b334556944759cd414fd8b7e22312f8731eadaf9345f
source_last_modified: "2026-01-22T14:45:01.319734+00:00"
translation_last_reviewed: 2026-02-07
id: orchestrator-tuning
title: Orchestrator Rollout & Tuning
sidebar_label: Orchestrator Tuning
description: Practical defaults, tuning guidance, and audit checkpoints for taking the multi-source orchestrator to GA.
translator: machine-google-reviewed
---

:::注意規範來源
:::

# Orchestrator 推出和調整指南

本指南以[配置參考](orchestrator-config.md) 和
[多源部署操作手冊](multi-source-rollout.md)。它解釋了
如何為每個推出階段調整協調器，如何解釋
記分板文物，以及哪些遙測信號必須在之前就位
擴大交通。跨 CLI、SDK 和應用一致地應用建議
自動化，因此每個節點都遵循相同的確定性獲取策略。

## 1. 基線參數集

從共享配置模板開始並調整一小組旋鈕：
推廣工作正在進行中。下表列出了推薦值
最常見的階段；未列出的值將回退到默認值
`OrchestratorConfig::default()` 和 `FetchOptions::default()`。

|相| `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` |筆記|
|--------------------|-----------------|--------------------------------------------|------------------------------------|--------------------------------------------|------------------------------------|------|
| **實驗室/CI** | `3` | `2` | `2` | `2500` | `300` |嚴格的延遲上限和寬限窗口快速表面嘈雜的遙測。保持較低的重試次數以更快地暴露無效清單。 |
| **分期** | `4` | `3` | `3` | `4000` | `600` |反映生產默認情況，同時為探索性同行留出空間。 |
| **金絲雀** | `6` | `3` | `3` | `5000` | `900` |匹配默認值；設置 `telemetry_region` 以便儀表板可以基於金絲雀流量。 |
| **全面上市** | `None`（使用所有符合條件的）| `4` | `4` | `5000` | `900` |增加重試和失敗閾值以吸收瞬態故障，同時審計繼續加強確定性。 |

- `scoreboard.weight_scale` 保持默認 `10_000`，除非下游
  系統需要不同的整數分辨率。擴大規模並不
  更改提供商訂購；它只會發出更密集的信用分佈。
- 在階段之間遷移時，保留 JSON 包並使用
  `--scoreboard-out` 因此審計跟踪記錄了確切的參數集。

## 2.記分牌衛生

記分板結合了清單要求、提供商廣告和遙測。
前滾之前：

1. **驗證遙測新鮮度。 ** 確保引用的快照
   `--telemetry-json` 是在配置的寬限窗口內捕獲的。參賽作品
   早於配置的 `telemetry_grace_secs` 失敗並顯示
   `TelemetryStale { last_updated }`。將其視為硬停止並刷新
   在繼續之前導出遙測數據。
2. **檢查資格原因。 ** 通過以下方式保留文物
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`。每個條目
   帶有包含確切故障原因的 `eligibility` 塊。不要覆蓋
   能力不匹配或廣告過期；修復上游有效負載。
3. **查看權重增量。 ** 將 `normalised_weight` 字段與
   以前的版本。體重變化 >10% 應與故意廣告相關
   或遙測更改，必須在推出日誌中確認。
4. **存檔工件。 ** 配置 `scoreboard.persist_path`，以便每次運行都會發出
   最終記分牌快照。將工件附加到發布記錄
   與清單和遙測包一起。
5. **記錄提供者混合證據。 ** `scoreboard.json` 元數據_和_
   匹配 `summary.json` 必須公開 `provider_count`，
   `gateway_provider_count`，以及派生的 `provider_mix` 標籤，因此審閱者
   可以證明運行是否為 `direct-only`、`gateway-only` 或 `mixed`。
   網關因此捕獲報告 `provider_count=0` plus
   `provider_mix="gateway-only"`，而混合運行需要非零計數
   兩個來源。 `cargo xtask sorafs-adoption-check` 強制執行這些字段（並且
   當計數/標籤不一致時會失敗），所以總是一起運行
   `ci/check_sorafs_orchestrator_adoption.sh` 或您定制的捕獲腳本
   生成 `adoption_report.json` 證據包。當 Torii 網關為
   參與，將 `gateway_manifest_id`/`gateway_manifest_cid` 保留在記分牌中
   元數據，以便採用門可以將清單信封與
   捕獲的提供商組合。

詳細字段定義參見
`crates/sorafs_car/src/scoreboard.rs` 和 CLI 摘要結構公開
`sorafs_cli fetch --json-out`。

## CLI 和 SDK 標誌參考

`sorafs_cli fetch`（參見 `crates/sorafs_car/src/bin/sorafs_cli.rs`）和
`iroha_cli app sorafs fetch` 包裝器 (`crates/iroha_cli/src/commands/sorafs.rs`)
共享相同的協調器配置表面。使用以下標誌時
捕獲推出證據或重播規範賽程：

共享多源標誌參考（僅通過編輯此文件來保持 CLI 幫助和文檔同步）：

- `--max-peers=<count>` 限制了有多少合格的提供商能夠通過記分板過濾器。保留未設置以從每個符合條件的提供商進行流式傳輸，僅在有意執行單源回退時設置為 `1`。鏡像 SDK 中的 `maxPeers` 旋鈕（`SorafsGatewayFetchOptions.maxPeers`、`SorafsGatewayFetchOptions.max_peers`）。
- `--retry-budget=<count>` 轉發至 `FetchOptions` 強制執行的每塊重試限制。使用調整指南中的捲展表來獲取推薦值；收集證據的 CLI 運行必須與 SDK 默認值匹配才能保持奇偶性。
- `--telemetry-region=<label>` 使用區域/環境標籤標記 `sorafs_orchestrator_*` Prometheus 系列（和 OTLP 繼電器），以便儀表板可以分離實驗室、登台、金絲雀和 GA 流量。
- `--telemetry-json=<path>` 注入記分板引用的快照。將 JSON 保留在記分板旁邊，以便審核員可以重播運行（因此 `cargo xtask sorafs-adoption-check --require-telemetry` 可以證明哪個 OTLP 流提供了捕獲）。
- `--local-proxy-*`（`--local-proxy-mode`、`--local-proxy-norito-spool`、`--local-proxy-kaigi-spool`、`--local-proxy-kaigi-policy`）啟用橋觀察器掛鉤。設置後，編排器通過本地 Norito/Kaigi 代理流式傳輸塊，以便瀏覽器客戶端、防護緩存和 Kaigi 房間收到 Rust 發出的相同收據。
- `--scoreboard-out=<path>`（可選與 `--scoreboard-now=<unix_secs>` 配對）為審核員保留資格快照。始終將持久化的 JSON 與發布票證中引用的遙測和清單工件配對。
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` 在廣告元數據之上應用確定性調整。僅在排練時使用這些標誌；生產降級必須通過治理工件進行，以便每個節點都應用相同的策略包。
- `--provider-metrics-out` / `--chunk-receipts-out` 保留推出清單引用的每個提供商的運行狀況指標和塊收據；提交收養證據時附上這兩件物品。

示例（使用已發布的夾具）：

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

SDK 通過 Rust 中的 `SorafsGatewayFetchOptions` 使用相同的配置
客戶端（`crates/iroha/src/client.rs`），JS 綁定
(`javascript/iroha_js/src/sorafs.js`) 和 Swift SDK
（`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`）。讓那些幫手留在裡面
與 CLI 默認值同步，以便操作員可以跨自動化複製策略
沒有定制的翻譯層。

## 3. 獲取策略調整

`FetchOptions` 控制重試行為、並發性和驗證。當
調整：

- **重試：** 將 `per_chunk_retry_limit` 提高到超過 `4` 可增加恢復
  時間，但有掩蓋提供商錯誤的風險。更喜歡保留 `4` 作為上限
  依靠供應商輪換來發現表現不佳的人員。
- **故障閾值：** `provider_failure_threshold` 控制何時
  提供者在會話的剩餘時間內被禁用。將此值與
  重試策略：低於重試預算的閾值強制協調器
  在所有重試用完之前彈出對等點。
- **並發：** 保留 `global_parallel_limit` 未設置 (`None`)，除非
  特定環境無法滿足所公佈的範圍。設置後，確保
  該值≤提供商流預算的總和，以避免飢餓。
- **驗證切換：** `verify_lengths` 和 `verify_digests` 必須保留
  在生產中啟用。當混合供應商車隊時，它們保證確定性
  正在比賽中；僅在隔離的模糊測試環境中禁用它們。

## 4. 傳輸和匿名分期

使用 `rollout_phase`、`anonymity_policy` 和 `transport_policy` 字段
代表隱私姿態：- 首選 `rollout_phase="snnet-5"` 並允許默認匿名策略
  跟踪 SNNet-5 里程碑。僅通過 `anonymity_policy_override` 覆蓋
  當治理髮布簽署的指令時。
- 保持 `transport_policy="soranet-first"` 作為基線，而 SNNet-4/5/5a/5b/6a/7/8/12/13 是🈺
  （參見 `roadmap.md`）。僅將 `transport_policy="direct-only"` 用於記錄
  降級/合規演習，並等待 PQ 覆蓋率審查
  升級到 `transport_policy="soranet-strict"` — 如果出現以下情況，該層將快速失敗
  只剩下經典的繼電器。
- `write_mode="pq-only"` 僅應在每個寫入路徑（SDK、
  協調器、治理工具）可以滿足 PQ 要求。期間
  推出保留 `write_mode="allow-downgrade"`，以便緊急響應可以依靠
  在直接路線上，而遙測標記降級。
- 防護選擇和電路分級依賴於 SoraNet 目錄。供應
  簽署 `relay_directory` 快照並保留 `guard_set` 緩存以保護
  流失率保持在商定的保留窗口內。記錄的緩存指紋
  `sorafs_cli fetch` 構成了推出證據的一部分。

## 5. 降級和合規掛鉤

兩個協調器子系統有助於在無需人工干預的情況下執行策略：

- **降級修復** (`downgrade_remediation`)：監視器
  `handshake_downgrade_total` 事件，配置後的 `threshold` 為
  超出 `window_secs` 內，強製本地代理進入 `target_mode`
  （默認情況下僅元數據）。保留默認值（`threshold=3`、`window=300`、
  `cooldown=900`）除非事件審查顯示不同的模式。記錄任何
  覆蓋推出日誌並確保儀表板跟踪
  `sorafs_proxy_downgrade_state`。
- **合規政策** (`compliance`)：管轄權和清單豁免
  流經治理管理的選擇退出列表。切勿內聯臨時覆蓋
  在配置包中；相反，請求籤名更新
  `governance/compliance/soranet_opt_outs.json` 並重新部署生成的 JSON。

對於這兩個系統，保留生成的配置包並將其包含在
發布證據，以便審計人員可以追踪降檔是如何觸發的。

## 6. 遙測和儀表板

在擴大推出之前，請確認以下信號已在
目標環境：

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  金絲雀完成後應為零。
- `sorafs_orchestrator_retries_total` 和
  `sorafs_orchestrator_retry_ratio` — 期間應穩定在 10% 以下
  金絲雀並在 GA 後保持在 5% 以下。
- `sorafs_orchestrator_policy_events_total` — 驗證預期的
  推出階段處於活動狀態（`stage` 標籤）並通過 `outcome` 記錄限電。
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — 跟踪 PQ 繼電器電源
  政策預期。
- `telemetry::sorafs.fetch.*` 日誌目標 — 必須流式傳輸到共享日誌
  已保存 `status=failed` 搜索的聚合器。

從以下位置加載規範的 Grafana 儀表板
`dashboards/grafana/sorafs_fetch_observability.json`（在門戶中導出
在 **SoraFS → Fetch Observability** 下），因此區域/清單選擇器，
提供者重試熱圖、塊延遲直方圖和停頓計數器匹配
SRE 在老化期間會審查什麼。將 Alertmanager 規則連接到
`dashboards/alerts/sorafs_fetch_rules.yml` 並驗證 Prometheus 語法
與 `scripts/telemetry/test_sorafs_fetch_alerts.sh` （助手自動
在本地或 Docker 中運行 `promtool test rules`）。警報切換需要
與腳本打印相同的路由塊，以便操作員可以將證據固定到
推出票。

### 遙測預燒工作流程

路線圖項目 **SF-6e** 在翻轉之前需要進行 30 天的遙測老化
多源編排器恢復其 GA 默認值。使用存儲庫腳本
在窗口中每天捕獲可複制的工件包：

1.用燒機環境運行`ci/check_sorafs_orchestrator_adoption.sh`
   旋鈕設置。示例：

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   助手重播 `fixtures/sorafs_orchestrator/multi_peer_parity_v1`，
   寫入 `scoreboard.json`、`summary.json`、`provider_metrics.json`、
   `chunk_receipts.json` 和 `adoption_report.json` 下
   `artifacts/sorafs_orchestrator/<timestamp>/`，並強制執行最小數量
   通過 `cargo xtask sorafs-adoption-check` 列出符合資格的提供商。
2. 當存在預燒變量時，腳本也會發出
   `burn_in_note.json`，捕獲標籤、日期索引、清單 ID、遙測
   來源和人工製品摘要。將此 JSON 附加到推出日誌中，以便它是
   很明顯，在 30 天的窗口內，每天的捕獲都令人滿意。
3.導入更新的Grafana板（`dashboards/grafana/sorafs_fetch_observability.json`）
   進入暫存/生產工作區，用老化標籤對其進行標記，然後
   確認每個面板都顯示正在測試的清單/區域的樣本。
4.運行`scripts/telemetry/test_sorafs_fetch_alerts.sh`（或`promtool test rules …`）
   每當 `dashboards/alerts/sorafs_fetch_rules.yml` 更改以記錄該信息時
   警報路由與預燒期間導出的指標相匹配。
5. 歸檔生成的儀表板快照、警報測試輸出和日誌尾部
   從 `telemetry::sorafs.fetch.*` 與協調器一起搜索
   人工製品，以便治理可以重播證據，而無需從中提取指標
   實時系統。

## 7. 推出清單

1. 使用候選配置並捕獲在 CI 中重新生成記分板
   版本控制下的工件。
2. 在每個環境（實驗室、暫存、
   金絲雀，生產）並附上 `--scoreboard-out` 和 `--json-out`
   發布記錄中的工件。
3. 與值班工程師一起審查遙測儀表板，確保所有指標
   上面有活體樣本。
4. 記錄最終的配置路徑（通常通過`iroha_config`）和
   用於廣告和合規性的治理註冊表的 git commit。
5. 更新推出跟踪器並通知 SDK 團隊新的默認值，以便客戶端
   集成保持一致。

遵循本指南可以保持協調器部署的確定性和可審計性
在提供清晰的反饋循環來調整重試預算的同時，提供商
容量和隱私狀況。