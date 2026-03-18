---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b80573de9799c783b62fe4babb553de4dd0778b028cd6d6ad58eb3094f7284eb
source_last_modified: "2026-01-04T08:19:26.497389+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Provider Advert Rollout Plan"
translator: machine-google-reviewed
---

> 改編自 [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md)。

# SoraFS 提供商廣告推出計劃

該計劃協調從允許的提供商廣告到
多源塊所需的完全控制的 `ProviderAdvertV1` 表面
檢索。它側重於三個可交付成果：

- **操作員指南。 ** 存儲提供商必須完成的分步操作
  在每個門翻轉之前。
- **遙測覆蓋範圍。 ** Observability 和 Ops 使用的儀表板和警報
  確認網絡只接受合規的廣告。
此次推出與 [SoraFS 遷移中的 SF-2b/2c 里程碑保持一致
路線圖](./migration-roadmap) 並假設
[提供商准入政策](./provider-admission-policy) 已在
效果。

## 目前的要求

SoraFS 僅接受治理封裝的 `ProviderAdvertV1` 有效負載。的
入學時必須執行以下要求：

- `profile_id=sorafs.sf1@1.0.0` 與規範 `profile_aliases` 存在。
- 多源必須包含 `chunk_range_fetch` 功能有效負載
  檢索。
- `signature_strict=true`，廣告上附有理事會簽名
  信封。
- `allow_unknown_capabilities` 僅在顯式 GREASE 鑽探期間允許
  並且必須被記錄。

## 操作員清單

1. **庫存廣告。 ** 列出每個已發布的廣告和記錄：
   - 控制包絡路徑（`defaults/nexus/sorafs_admission/...` 或同等產品）。
   - 廣告 `profile_id` 和 `profile_aliases`。
   - 能力列表（預計至少為 `torii_gateway` 和 `chunk_range_fetch`）。
   - `allow_unknown_capabilities` 標誌（當存在供應商保留的 TLV 時需要）。
2. **使用提供者工具重新生成。 **
   - 與您的提供商廣告發布商重建有效負載，確保：
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` 與已定義的 `max_span`
     - 當存在 GREASE TLV 時，`allow_unknown_capabilities=<true|false>`
   - 通過 `/v1/sorafs/providers` 和 `sorafs_fetch` 驗證；關於未知的警告
     必須對能力進行分類。
3. **驗證多源準備情況。 **
   -用`--provider-advert=<path>`執行`sorafs_fetch`； CLI 現在失敗
     當 `chunk_range_fetch` 丟失並打印忽略未知的警告時
     能力。捕獲 JSON 報告並將其與操作日誌一起存檔。
4. **階段更新。 **
   - 至少提前 30 天提交 `ProviderAdmissionRenewalV1` 信封
     到期。續訂必須保留規範的句柄和功能集；
     只有權益、端點或元數據應該改變。
5. **與依賴團隊溝通。 **
   - SDK 所有者必鬚髮布向操作員發出警告的版本
     廣告被拒絕。
   - DevRel 公佈每個階段的轉變；包括儀表板鏈接和
     下面的閾值邏輯。
6. **安裝儀表板和警報。 **
   - 導入 Grafana 導出並將其放在 **SoraFS / Provider 下
     推出**，儀表板 UID `sorafs-provider-admission`。
   - 確保警報規則指向共享的 `sorafs-advert-rollout`
     暫存和生產中的通知渠道。

## 遙測和儀表板

以下指標已通過 `iroha_telemetry` 公開：

- `torii_sorafs_admission_total{result,reason}` — 接受、拒絕的計數，
  和警告結果。原因包括 `missing_envelope`、`unknown_capability`、
  `stale` 和 `policy_violation`。

Grafana 導出：[`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json)。
將文件導入共享儀表板存儲庫 (`observability/dashboards`)
並在發布前僅更新數據源 UID。

該板在 Grafana 文件夾 **SoraFS / Provider Rollout** 下發布
穩定的 UID `sorafs-provider-admission`。警報規則
`sorafs-admission-warn`（警告）和 `sorafs-admission-reject`（嚴重）是
預先配置為使用 `sorafs-advert-rollout` 通知策略；調整
如果目的地列表發生更改，則該聯繫點而不是編輯
儀表板 JSON。

推薦的 Grafana 面板：

|面板|查詢 |筆記|
|--------|--------|--------|
| **錄取結果率** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` |堆棧圖可視化接受、警告、拒絕。當警告 > 0.05 * 總計（警告）或拒絕 > 0（嚴重）時發出警報。 |
| **警戒率** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` |滿足尋呼機閾值的單行時間序列（5% 警告率滾動 15 分鐘）。 |
| **拒絕原因** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` |推動運行手冊分類；附加緩解步驟的鏈接。 |
| **刷新債務** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` |表示提供商錯過了刷新截止日期；與發現緩存日誌的交叉引用。 |

手動儀表板的 CLI 工件：

- `sorafs_fetch --provider-metrics-out` 寫入 `failures`、`successes`，以及
  每個提供商的 `disabled` 計數器。導入臨時儀表板進行監控
  在切換生產提供商之前，orchestrator 會進行試運行。
- JSON 報告的 `chunk_retry_rate` 和 `provider_failure_rate` 字段
  突出顯示通常在進入之前出現的節流或失效有效負載症狀
  拒絕。

### Grafana 儀表板佈局

Observability 發布了專門的委員會 — **SoraFS 提供商准入
推出** (`sorafs-provider-admission`) — 在 **SoraFS/提供商推出**下
具有以下規範面板 ID：

- 第 1 組 — *入院結果率*（堆疊面積，單位“操作/分鐘”）。
- 面板 2 — *警告率*（單個系列），發出表達式
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- 第 3 版 — *拒絕原因*（按 `reason` 分組的時間序列），排序依據
  `rate(...[5m])`。
- 第 4 部分 — *刷新債務*（統計數據），反映上表中的查詢和
  註釋有從遷移分類賬中提取的廣告刷新截止日期。

在基礎設施儀表板存儲庫中復制（或創建）JSON 框架，網址為
`observability/dashboards/sorafs_provider_admission.json`，然後僅更新
數據源UID；面板 ID 和警報規則由 Runbook 引用
下面，因此請避免在不修改本文檔的情況下對它們重新編號。

為了方便起見，存儲庫現在在以下位置提供了參考儀表板定義：
`docs/source/grafana_sorafs_admission.json`；將其複製到您的 Grafana 文件夾中，如果
您需要一個本地測試的起點。

### Prometheus 警報規則

將以下規則組添加到 `observability/prometheus/sorafs_admission.rules.yml`
（如果這是第一個 SoraFS 規則組，則創建該文件）並將其包含在
您的 Prometheus 配置。將 `<pagerduty>` 替換為實際路由
隨叫隨到輪換的標籤。

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

運行 `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
在推送更改之前確保語法通過 `promtool check rules`。

## 錄取結果

- 缺少 `chunk_range_fetch` 功能 → 使用 `reason="missing_capability"` 拒絕。
- 不帶 `allow_unknown_capabilities=true` 的未知功能 TLV → 拒絕
  `reason="unknown_capability"`。
- `signature_strict=false` → 拒絕（保留用於隔離診斷）。
- 過期 `refresh_deadline` → 拒絕。

## 溝通和事件處理

- **每週狀態郵件。 ** DevRel 分發入學簡要摘要
  指標、未解決的警告和即將到來的截止日期。
- **事件響應。 ** 如果 `reject` 發出火災警報，待命工程師：
  1. 通過 Torii 發現 (`/v1/sorafs/providers`) 獲取違規廣告。
  2. 在提供商管道中重新運行廣告驗證並與
     `/v1/sorafs/providers` 重現該錯誤。
  3. 與提供商協調，在下次刷新之前輪播廣告
     截止日期。
- **更改凍結。 ** 在 R1/R2 期間，功能模式不會發生更改，除非
  推出委員會簽字同意； GREASE 試驗必須安排在
  每周維護窗口並記錄在遷移分類賬中。

## 參考文獻

- [SoraFS 節點/客戶端協議](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [提供商准入政策](./provider-admission-policy)
- [遷移路線圖](./migration-roadmap)
- [提供商廣告多源擴展](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)