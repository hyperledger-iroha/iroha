---
lang: zh-hant
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6120b882618e0f9b6113948d3b12d97e0152a5fc5d4350681ba30aaf114e99d3
source_last_modified: "2026-01-22T14:45:01.354580+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-default-lane-quickstart
title: Default lane quickstart (NX-5)
sidebar_label: Default Lane Quickstart
description: Configure and verify the Nexus default lane fallback so Torii and SDKs can omit lane_id in public lanes.
translator: machine-google-reviewed
---

:::注意規範來源
此頁面鏡像 `docs/source/quickstart/default_lane.md`。保留兩份副本
對齊，直到定位掃描落在門戶中。
:::

# 默認車道快速入門 (NX-5)

> **路線圖背景：** NX-5 — 默認公共車道集成。現在的運行時間
> 公開 `nexus.routing_policy.default_lane` 回退，因此 Torii REST/gRPC
> 當流量所屬時，端點和每個 SDK 都可以安全地省略 `lane_id`
> 在規範的公共車道上。本指南引導操作員完成配置
> 目錄，驗證 `/status` 中的回退，並鍛煉客戶端
> 端到端的行為。

## 先決條件

- `irohad` 的 Sora/Nexus 版本（與 `irohad --sora --config ...` 一起運行）。
- 訪問配置存儲庫，以便您可以編輯 `nexus.*` 部分。
- `iroha_cli` 配置為與目標集群通信。
- `curl`/`jq`（或等效項），用於檢查 Torii `/status` 有效負載。

## 1. 描述通道和數據空間目錄

聲明網絡上應存在的通道和數據空間。片段
下面（從 `defaults/nexus/config.toml` 修剪）註冊了三個公共車道
加上匹配的數據空間別名：

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

每個 `index` 必須是唯一且連續的。數據空間 id 是 64 位值；
為了清楚起見，上面的示例使用與車道索引相同的數值。

## 2. 設置路由默認值和可選覆蓋

`nexus.routing_policy` 部分控制後備通道並讓您
覆蓋特定指令或帳戶前綴的路由。如果沒有規則
匹配，調度程序將事務路由到配置的 `default_lane`
和 `default_dataspace`。路由器邏輯位於
`crates/iroha_core/src/queue/router.rs` 並將策略透明地應用於
Torii REST/gRPC 表面。

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

當您稍後添加新車道時，請先更新目錄，然後擴展路線
規則。後備車道應繼續指向公共車道

## 3. 啟動應用了策略的節點

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

節點在啟動期間記錄派生的路由策略。任何驗證錯誤
（缺少索引、重複的別名、無效的數據空間 ID）在之前出現
八卦開始了。

## 4.確認車道治理狀態

節點上線後，使用 CLI 幫助程序驗證默認通道是否為
密封（已裝載艙單）並準備運輸。摘要視圖打印一行
每車道：

```bash
iroha_cli app nexus lane-report --summary
```

輸出示例：

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

如果默認通道顯示 `sealed`，請按照之前的通道治理操作手冊進行操作
允許外部流量。 `--fail-on-sealed` 標誌對於 CI 來說很方便。

## 5. 檢查 Torii 狀態負載

`/status` 響應公開了路由策略和每通道調度程序
快照。使用 `curl`/`jq` 確認配置的默認值並檢查
後備通道正在生成遙測數據：

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

示例輸出：

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

要檢查通道 `0` 的實時調度程序計數器：

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

這確認了 TEU 快照、別名元數據和清單標誌對齊
與配置。 Grafana 面板使用相同的有效負載
車道攝取儀表板。

## 6. 執行客戶端默認設置

- **Rust/CLI.** `iroha_cli` 和 Rust 客戶端包省略了 `lane_id` 字段
  當您未通過 `--lane-id` / `LaneSelector` 時。因此隊列路由器
  回落至 `default_lane`。使用顯式 `--lane-id`/`--dataspace-id` 標誌
  僅當定位非默認車道時。
- **JS/Swift/Android。 **最新的 SDK 版本將 `laneId`/`lane_id` 視為可選
  並回退到 `/status` 所公佈的值。保留路由策略
  跨階段和生產同步，因此移動應用程序不需要緊急情況
  重新配置。
- **管道/SSE 測試。 ** 交易事件過濾器接受
  `tx_lane_id == <u32>` 謂詞（請參閱 `docs/source/pipeline.md`）。訂閱
  `/v2/pipeline/events/transactions` 使用該過濾器來證明寫入已發送
  沒有明確的車道到達後備車道 ID 下。

## 7. 可觀察性和治理掛鉤

- `/status` 還發布了 `nexus_lane_governance_sealed_total` 和
  `nexus_lane_governance_sealed_aliases` 因此 Alertmanager 可以在任何時候發出警告
  車道失去其清單。即使對於開發網絡也保持啟用這些警報。
- 調度程序遙測地圖和車道治理儀表板
  (`dashboards/grafana/nexus_lanes.json`) 期望別名/slug 字段來自
  目錄。如果重命名別名，請重新標記相應的 Kura 目錄，以便
  審計員保持確定性路徑（在 NX-1 下跟踪）。
- 議會對默認車道的批准應包括回滾計劃。記錄
  清單哈希和治理證據以及本快速入門
  操作員運行手冊，以便將來的輪換不會猜測所需的狀態。

一旦這些檢查通過，您就可以將 `nexus.routing_policy.default_lane` 視為
網絡上的代碼路徑。