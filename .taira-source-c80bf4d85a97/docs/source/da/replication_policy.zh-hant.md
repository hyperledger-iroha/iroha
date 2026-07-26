---
lang: zh-hant
direction: ltr
source: docs/source/da/replication_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70163ed6740c80c48c78ae918c37d34e0022ab97ffabce6d451bbf85060e24b4
source_last_modified: "2026-01-22T14:35:37.691616+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 數據可用性複制策略 (DA-4)

_狀態：進行中 — 所有者：核心協議工作組/存儲團隊/SRE_

DA 攝取管道現在強制執行確定性保留目標
`roadmap.md`（工作流 DA-4）中描述的每個 Blob 類。 Torii 拒絕
保留調用者提供的與配置不匹配的保留信封
策略，保證每個驗證器/存儲節點保留所需的
紀元和副本的數量，而不依賴於提交者的意圖。

## 默認策略

|斑點類|熱保持|保冷|所需副本 |存儲類|治理標籤|
|------------------------|--------------|----------------|--------------------------------|----------------|----------------|
| `taikai_segment` | 24小時| 14 天 | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 小時 | 7 天 | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 小時 | 180 天 | 3 | `cold` | `da.governance` |
| _默認（所有其他類）_ | 6 小時 | 30 天 | 3 | `warm` | `da.default` |

這些值嵌入在 `torii.da_ingest.replication_policy` 中並應用於
所有 `/v1/da/ingest` 提交內容。 Torii 使用強制重寫清單
保留配置文件並在調用者提供不匹配的值時發出警告，以便
運營商可以檢測過時的 SDK。

### Taikai 可用性課程

Taikai 路由清單（`taikai.trm` 元數據）現在包含
`availability_class` 提示（`Hot`、`Warm` 或 `Cold`）。當存在時，Torii
從 `torii.da_ingest.replication_policy` 中選擇匹配的保留配置文件
在對有效負載進行分塊之前，允許事件操作員降級非活動狀態
無需編輯全局策略表即可進行演繹。默認值是：

|可用性等級 |熱保持|保冷|所需副本 |存儲類|治理標籤|
|--------------------|-------------|----------------|--------------------------------|----------------|----------------|
| `hot` | 24小時| 14 天 | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 小時 | 30 天 | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1小時| 180 天 | 3 | `cold` | `da.taikai.archive` |

如果清單省略 `availability_class`，則攝取路徑會回退到
`hot` 配置文件，因此實時流保留其完整的副本集。運營商可以
通過編輯新值來覆蓋這些值
配置中的 `torii.da_ingest.replication_policy.taikai_availability` 塊。

## 配置

該保單位於 `torii.da_ingest.replication_policy` 下，並公開了
*默認*模板加上每個類覆蓋的數組。類標識符是
不區分大小寫並接受 `taikai_segment`、`nexus_lane_sidecar`、
`governance_artifact` 或 `custom:<u16>` 用於治理批准的擴展。
存儲類別接受 `hot`、`warm` 或 `cold`。

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

保持塊不變，以使用上面列出的默認值運行。擰緊一個
類，更新匹配的覆蓋；更改新類的基線，
編輯 `default_retention`。要調整特定的 Taikai 可用性等級，請在下面添加條目
`torii.da_ingest.replication_policy.taikai_availability`：

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "warm"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 14400         # 4 h
cold_retention_secs = 604800       # 7 d
required_replicas = 4
storage_class = "warm"
governance_tag = "da.taikai.warm"
```

## 強制語義

- Torii 使用強製配置文件替換用戶提供的 `RetentionPolicy`
  在分塊或顯式發射之前。
- 聲明不匹配保留配置文件的預構建清單將被拒絕
  與 `400 schema mismatch` 因此過時的客戶不能削弱合同。
- 記錄每個覆蓋事件（`blob_class`，已提交與預期策略）
  在推出期間顯示不合規的呼叫者。

請參閱 `docs/source/da/ingest_plan.md`（驗證清單）了解更新的門
涵蓋保留執行。

## 重新復制工作流程（DA-4 後續）

保留強制執行只是第一步。運營商還必須證明
實時清單和復制訂單與配置的策略保持一致，因此
SoraFS 可以自動重新復制不合規的 blob。

1. **注意漂移。 ** Torii 發出
   `overriding DA retention policy to match configured network baseline` 每當
   調用者提交過時的保留值。將該日誌與
   `torii_sorafs_replication_*` 遙測發現副本短缺或延遲
   重新部署。
2. **意圖與實時副本的區別。 ** 使用新的審計助手：

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   該命令從提供的加載 `torii.da_ingest.replication_policy`
   配置，解碼每個清單（JSON 或 Norito），並可選擇匹配任何
   `ReplicationOrderV1` 清單摘要的有效負載。摘要標記了兩個
   條件：

   - `policy_mismatch` – 清單保留配置文件與強制執行的不同
     策略（除非 Torii 配置錯誤，否則這種情況永遠不會發生）。
   - `replica_shortfall` – 實時復制訂單請求的副本數少於
     `RetentionPolicy.required_replicas` 或提供比其更少的分配
     目標。

   非零退出狀態表示存在活躍短缺，因此 CI/on-call 自動化
   可以立即尋呼。將 JSON 報告附加到
   `docs/examples/da_manifest_review_template.md` 議會投票數據包。
3. **觸發重新復制。 ** 當審計報告不足時，發出新的
   `ReplicationOrderV1` 通過中描述的治理工具
   `docs/source/sorafs/storage_capacity_marketplace.md` 並重新運行審核
   直到副本集收斂。對於緊急覆蓋，請將 CLI 輸出配對
   與 `iroha app da prove-availability` 以便 SRE 可以引用相同的摘要
   和 PDP 證據。

回歸覆蓋範圍位於 `integration_tests/tests/da/replication_policy.rs` 中；
該套件向 `/v1/da/ingest` 提交不匹配的保留策略並驗證
獲取的清單公開強製配置文件而不是調用者
意圖。

## 健康證明遙測和儀表板（DA-5 橋）

路線圖項目 **DA-5** 要求 PDP/PoTR 執行結果可在
實時。 `SorafsProofHealthAlert` 事件現在驅動一組專用的
Prometheus 指標：

- `torii_sorafs_proof_health_alerts_total{provider_id,trigger,penalty}`
- `torii_sorafs_proof_health_pdp_failures{provider_id}`
- `torii_sorafs_proof_health_potr_breaches{provider_id}`
- `torii_sorafs_proof_health_penalty_nano{provider_id}`
- `torii_sorafs_proof_health_cooldown{provider_id}`
- `torii_sorafs_proof_health_window_end_epoch{provider_id}`

**SoraFS PDP 和 PoTR 健康** Grafana 板
(`dashboards/grafana/sorafs_pdp_potr_health.json`) 現在公開這些信號：- *按觸發器證明健康警報* 按觸發器/懲罰標記繪製警報率圖表，以便
  Taikai/CDN 運營商可以證明僅 PDP、僅 PoTR 或雙重攻擊是否有效
  射擊。
- *冷卻中的提供者*報告當前處於冷卻狀態的提供者的實時總數
  SorafsProofHealthAlert 冷卻時間。
- *健康證明窗口快照* 合併 PDP/PoTR 計數器、罰款金額、
  冷卻標誌，並敲擊每個提供商的窗口結束時期，以便治理審查員
  可以將該表附加到事件數據包中。

在提供 DA 執行證據時，操作手冊應鏈接這些小組；他們
將 CLI 證明流失敗直接與鏈上懲罰元數據聯繫起來，
提供路線圖中調用的可觀察性掛鉤。