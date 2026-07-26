---
lang: zh-hant
direction: ltr
source: docs/portal/docs/da/replication-policy.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

標題：數據可用性複制策略
sidebar_label：複製策略
描述：適用於所有 DA 攝取提交的治理強制保留配置文件。
---

:::注意規範來源
:::

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

Taikai 路由清單 (`taikai.trm`) 聲明 `availability_class`
（`hot`、`warm` 或 `cold`）。 Torii 在分塊之前強制執行匹配策略
因此操作員可以擴展每個流的副本數量，而無需編輯全局
表。默認值：

|可用性等級 |熱保持|保冷|所需副本 |存儲類|治理標籤|
|--------------------|-------------|----------------|--------------------------------|----------------|----------------|
| `hot` | 24小時| 14 天 | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 小時 | 30 天 | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1小時| 180 天 | 3 | `cold` | `da.taikai.archive` |

缺失提示默認為 `hot`，因此直播保留最強策略。
通過覆蓋默認值
`torii.da_ingest.replication_policy.taikai_availability` 如果您的網絡使用
不同的目標。

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
編輯 `default_retention`。

Taikai 可用性類可以通過獨立覆蓋
`torii.da_ingest.replication_policy.taikai_availability`：

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## 強制語義

- Torii 使用強製配置文件替換用戶提供的 `RetentionPolicy`
  在分塊或顯式發射之前。
- 聲明不匹配保留配置文件的預構建清單將被拒絕
  與 `400 schema mismatch` 因此過時的客戶不能削弱合同。
- 記錄每個覆蓋事件（`blob_class`，已提交與預期策略）
  在推出期間顯示不合規的呼叫者。

請參閱[數據可用性攝取計劃](ingest-plan.md)（驗證清單）了解更新後的門
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
   `docs/examples/da_manifest_review_template.md`
   議會投票的數據包。
3. **觸發重新復制。 ** 當審計報告不足時，發出新的
   `ReplicationOrderV1` 通過中描述的治理工具
   [SoraFS存儲容量市場](../sorafs/storage-capacity-marketplace.md)並重新運行審核
   直到副本集收斂。對於緊急覆蓋，請將 CLI 輸出配對
   與 `iroha app da prove-availability` 以便 SRE 可以引用相同的摘要
   和 PDP 證據。

回歸覆蓋範圍位於 `integration_tests/tests/da/replication_policy.rs` 中；
該套件向 `/v1/da/ingest` 提交不匹配的保留策略並驗證
獲取的清單公開強製配置文件而不是調用者
意圖。