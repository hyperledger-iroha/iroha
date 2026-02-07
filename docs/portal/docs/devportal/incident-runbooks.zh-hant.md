---
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8599dbc1a8e4fe846965eed90af128deb5950f83dc61838fea583b326b92a011
source_last_modified: "2025-12-29T18:16:35.104300+00:00"
translation_last_reviewed: 2026-02-07
id: incident-runbooks
title: Incident Runbooks & Rollback Drills
sidebar_label: Incident Runbooks
description: Response guides for failed portal deployments, SoraFS replication degradation, analytics outages, and the quarterly rehearsal cadence required by DOCS-9.
translator: machine-google-reviewed
---

## 目的

路線圖項目 **DOCS-9** 需要可操作的劇本以及排練計劃，以便
門戶網站運營商無需猜測即可從運輸失敗中恢復。此註
涵蓋三個高信號事件——部署失敗、複製
退化和分析中斷，並記錄季度演練
證明別名回滾和綜合驗證仍然可以端到端地工作。

### 相關材料

- [`devportal/deploy-guide`](./deploy-guide) — 打包、簽名和別名
  促銷工作流程。
- [`devportal/observability`](./observability) — 發布標籤、分析和
  下面引用的探針。
- `docs/source/sorafs_node_client_protocol.md`
  和 [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — 註冊表遙測和升級閾值。
- `docs/portal/scripts/sorafs-pin-release.sh` 和 `npm run probe:*` 幫助程序
  在整個清單中引用。

### 共享遙測和工具

|信號/工具|目的|
| ------------- | -------- |
| `torii_sorafs_replication_sla_total`（已滿足/錯過/待處理）|檢測複製停滯和 SLA 違規。 |
| `torii_sorafs_replication_backlog_total`，`torii_sorafs_replication_completion_latency_epochs` |量化分類的積壓深度和完成延遲。 |
| `torii_sorafs_gateway_refusals_total`、`torii_sorafs_manifest_submit_total{status="error"}` |顯示錯誤部署後經常出現的網關端故障。 |
| `npm run probe:portal` / `npm run probe:tryit-proxy` |用於控制釋放和驗證回流的合成探針。 |
| `npm run check:links` |斷鍊門；每次緩解後使用。 |
| `sorafs_cli manifest submit … --alias-*`（由 `scripts/sorafs-pin-release.sh` 包裹）|別名提升/恢復機制。 |
| `Docs Portal Publishing` Grafana 板 (`dashboards/grafana/docs_portal.json`) |聚合拒絕/別名/TLS/複製遙測。 PagerDuty 警報引用這些面板來獲取證據。 |

## Runbook — 部署失敗或不良工件

### 觸發條件

- 預覽/生產探針失敗 (`npm run probe:portal -- --expect-release=…`)。
- `torii_sorafs_gateway_refusals_total` 上的 Grafana 警報或
  推出後的 `torii_sorafs_manifest_submit_total{status="error"}`。
- 手動 QA 立即註意到損壞的路由或 Try-It 代理故障
  別名推廣。

### 立即收容

1. **凍結部署：** 使用 `DEPLOY_FREEZE=1` 標記 CI 管道 (GitHub
   工作流輸入）或暫停 Jenkins 作業，這樣就不會出現其他工件。
2. **捕獲工件：** 下載失敗構建的 `build/checksums.sha256`，
   `portal.manifest*.{json,to,bundle,sig}`，並探測輸出，以便回滾可以
   參考準確的摘要。
3. **通知利益相關者：** 存儲 SRE、Docs/DevRel 領導和治理
   值班人員提高意識（特別是當 `docs.sora` 受到影響時）。

### 回滾過程

1. 識別最後已知良好 (LKG) 清單。生產流程存儲
   它們位於 `artifacts/devportal/<release>/sorafs/portal.manifest.to` 下。
2. 使用傳送助手將別名重新綁定到該清單：

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. 將回滾摘要與 LKG 一起記錄在事件單中，並
   清單摘要失敗。

### 驗證

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`。
2. `npm run check:links`。
3. `sorafs_cli manifest verify-signature …` 和 `sorafs_cli proof verify …`
   （請參閱部署指南）確認重新升級的清單仍然匹配
   存檔的 CAR。
4. `npm run probe:tryit-proxy` 確保 Try-It 臨時代理返回。

### 事件發生後

1. 僅在了解根本原因後才重新啟用部署管道。
2. 回填 [`devportal/deploy-guide`](./deploy-guide)“經驗教訓”
   帶有新陷阱的條目（如果有）。
3. 記錄失敗測試套件的缺陷（探針、鏈接檢查器等）。

## Runbook — 複製降級

### 觸發條件

- 警報：`sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  Clip_min(sum(torii_sorafs_replication_sla_total{結果=~"遇見|錯過"}), 1) <
  0.95` 10 分鐘。
- `torii_sorafs_replication_backlog_total > 10` 10 分鐘（參見
  `pin-registry-ops.md`）。
- 治理報告發布後別名可用性緩慢。

### 分類

1. 檢查 [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) 儀表板以確認
   待辦事項是否局限於存儲類或提供商隊列。
2. 交叉檢查 Torii 日誌中是否有 `sorafs_registry::submit_manifest` 警告
   確定提交本身是否失敗。
3. 通過 `sorafs_cli manifest status --manifest …` 獲取副本運行狀況樣本（列出
   每個提供商的複制結果）。

### 緩解措施

1. 使用更高的副本計數 (`--pin-min-replicas 7`) 重新發出清單
   `scripts/sorafs-pin-release.sh` 因此調度程序將負載分散到更大的
   提供商集。在事件日誌中記錄新的清單摘要。
2. 如果待辦事項與單個提供商相關聯，請通過以下方式暫時禁用它：
   複製調度程序（記錄在 `pin-registry-ops.md` 中）並提交新的
   清單強制其他提供者刷新別名。
3. 當別名新鮮度比複製奇偶校驗更重要時，重新綁定
   為已上演的熱清單 (`docs-preview`) 建立別名，然後發布
   一旦 SRE 清除了積壓的工作，就會有後續清單。

### 恢復和關閉

1. 監控`torii_sorafs_replication_sla_total{outcome="missed"}`以確保
   計算平台期。
2. 捕獲 `sorafs_cli manifest status` 輸出作為每個副本都已存在的證據
   恢復合規。
3. 歸檔或更新復制積壓事後分析以及後續步驟
   （提供者擴展、分塊器調整等）。

## Runbook — 分析或遙測中斷

### 觸發條件

- `npm run probe:portal` 成功，但儀表板停止攝取
  `AnalyticsTracker` 事件持續 >15 分鐘。
- 隱私審查標記了丟失事件的意外增加。
- `npm run probe:tryit-proxy` 在 `/probe/analytics` 路徑上失敗。

### 回應

1. 驗證構建時輸入：`DOCS_ANALYTICS_ENDPOINT` 和
   失敗的發布工件 (`build/release.json`) 中的 `DOCS_ANALYTICS_SAMPLE_RATE`。
2. 重新運行 `npm run probe:portal`，其中 `DOCS_ANALYTICS_ENDPOINT` 指向
   暫存收集器以確認跟踪器仍然發出有效負載。
3. 如果收集器已關閉，請設置 `DOCS_ANALYTICS_ENDPOINT=""` 並重建，以便
   跟踪器短路；在事件時間線中記錄中斷窗口。
4.驗證`scripts/check-links.mjs`仍然是指紋`checksums.sha256`
   （分析中斷不得*不*阻止站點地圖驗證）。
5. 一旦收集器恢復，運行 `npm run test:widgets` 來執行
   重新發布之前進行分析助手單元測試。

### 事件發生後

1. 使用任何新收集器更新 [`devportal/observability`](./observability)
   限製或抽樣要求。
2. 如果任何分析數據在外部被刪除或編輯，則文件治理通知
   政策。

## 每季度彈性訓練

在**每個季度的第一個星期二**（一月/四月/七月/十月）進行兩次演習
或在任何重大基礎設施變更後立即進行。將工件存儲在
`artifacts/devportal/drills/<YYYYMMDD>/`。

|鑽|步驟|證據|
| -----| -----| -------- |
| Alias 回滾演練 | 1. 使用最新的生產清單重播“部署失敗”回滾。 <br/>2.一旦探針通過，重新綁定到生產環境。 <br/>3.在鑽取文件夾中記錄 `portal.manifest.submit.summary.json` 和探測日誌。 | `rollback.submit.json`，探測輸出，以及排練的釋放標籤。 |
|綜合驗證審核| 1. 針對生產和登台運行 `npm run probe:portal` 和 `npm run probe:tryit-proxy`。 <br/>2.運行 `npm run check:links` 並存檔 `build/link-report.json`。 <br/>3.附上 Grafana 面板的屏幕截圖/導出，確認探測成功。 |探測日誌 + `link-report.json` 引用清單指紋。 |

將錯過的演習上報給 Docs/DevRel 經理和 SRE 治理審查，
因為路線圖需要確定性的季度證據，表明兩者都別名
回滾和門戶探針保持健康。

## PagerDuty 和待命協調

- PagerDuty 服務 **Docs Portal Publishing** 擁有從以下位置生成的警報
  `dashboards/grafana/docs_portal.json`。規則 `DocsPortal/GatewayRefusals`，
  `DocsPortal/AliasCache` 和 `DocsPortal/TLSExpiry` 頁面 Docs/DevRel
  主存儲 SRE 作為輔助存儲。
- 尋呼時，請附上 `DOCS_RELEASE_TAG`，並附上受影響的屏幕截圖
  Grafana 面板，以及之前事件註釋中的鏈路探測/鏈路檢查輸出
  緩解措施開始。
- 緩解（回滾或重新部署）後，重新運行 `npm run probe:portal`，
  `npm run check:links`，並捕獲顯示指標的新 Grafana 快照
  回到閾值內。附上 PagerDuty 事件之前的所有證據
  解決它。
- 如果兩個警報同時觸發（例如 TLS 到期加上積壓），則進行分類
  先拒絕（停止發布），執行回滾程序，然後清除
  橋接器上具有存儲 SRE 的 TLS/積壓項目。