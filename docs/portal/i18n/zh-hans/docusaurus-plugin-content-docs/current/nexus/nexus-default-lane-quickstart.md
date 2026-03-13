---
id: nexus-default-lane-quickstart
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Default lane quickstart (NX-5)
sidebar_label: Default Lane Quickstart
description: Configure and verify the Nexus default lane fallback so Torii and SDKs can omit lane_id in public lanes.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
此页面镜像 `docs/source/quickstart/default_lane.md`。保留两份副本
对齐，直到定位扫描落在门户中。
:::

# 默认车道快速入门 (NX-5)

> **路线图背景：** NX-5 — 默认公共车道集成。现在的运行时间
> 公开 `nexus.routing_policy.default_lane` 回退，因此 Torii REST/gRPC
> 当流量所属时，端点和每个 SDK 都可以安全地省略 `lane_id`
> 在规范的公共车道上。本指南引导操作员完成配置
> 目录，验证 `/status` 中的回退，并锻炼客户端
> 端到端的行为。

## 先决条件

- `irohad` 的 Sora/Nexus 版本（与 `irohad --sora --config ...` 一起运行）。
- 访问配置存储库，以便您可以编辑 `nexus.*` 部分。
- `iroha_cli` 配置为与目标集群通信。
- `curl`/`jq`（或等效项），用于检查 Torii `/status` 有效负载。

## 1. 描述通道和数据空间目录

声明网络上应存在的通道和数据空间。片段
下面（从 `defaults/nexus/config.toml` 修剪）注册了三个公共车道
加上匹配的数据空间别名：

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

每个 `index` 必须是唯一且连续的。数据空间 id 是 64 位值；
为了清楚起见，上面的示例使用与车道索引相同的数值。

## 2. 设置路由默认值和可选覆盖

`nexus.routing_policy` 部分控制后备通道并让您
覆盖特定指令或帐户前缀的路由。如果没有规则
匹配，调度程序将事务路由到配置的 `default_lane`
和 `default_dataspace`。路由器逻辑位于
`crates/iroha_core/src/queue/router.rs` 并将策略透明地应用于
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

当您稍后添加新车道时，请先更新目录，然后扩展路线
规则。后备车道应继续指向公共车道

## 3. 启动应用了策略的节点

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

节点在启动期间记录派生的路由策略。任何验证错误
（缺少索引、重复的别名、无效的数据空间 ID）在之前出现
八卦开始了。

## 4.确认车道治理状态

节点上线后，使用 CLI 帮助程序验证默认通道是否为
密封（已装载舱单）并准备运输。摘要视图打印一行
每车道：

```bash
iroha_cli app nexus lane-report --summary
```

输出示例：

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

如果默认通道显示 `sealed`，请按照之前的通道治理操作手册进行操作
允许外部流量。 `--fail-on-sealed` 标志对于 CI 来说很方便。

## 5. 检查 Torii 状态负载

`/status` 响应公开了路由策略和每通道调度程序
快照。使用 `curl`/`jq` 确认配置的默认值并检查
后备通道正在生成遥测数据：

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

示例输出：

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

要检查通道 `0` 的实时调度程序计数器：

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

这确认了 TEU 快照、别名元数据和清单标志对齐
与配置。 Grafana 面板使用相同的有效负载
车道摄取仪表板。

## 6. 执行客户端默认设置

- **Rust/CLI.** `iroha_cli` 和 Rust 客户端包省略了 `lane_id` 字段
  当您未通过 `--lane-id` / `LaneSelector` 时。因此队列路由器
  回落至 `default_lane`。使用显式 `--lane-id`/`--dataspace-id` 标志
  仅当定位非默认车道时。
- **JS/Swift/Android。**最新的 SDK 版本将 `laneId`/`lane_id` 视为可选
  并回退到 `/status` 所公布的值。保留路由策略
  跨阶段和生产同步，因此移动应用程序不需要紧急情况
  重新配置。
- **管道/SSE 测试。** 交易事件过滤器接受
  `tx_lane_id == <u32>` 谓词（请参阅 `docs/source/pipeline.md`）。订阅
  `/v2/pipeline/events/transactions` 使用该过滤器来证明写入已发送
  没有明确的车道到达后备车道 ID 下。

## 7. 可观察性和治理挂钩

- `/status` 还发布了 `nexus_lane_governance_sealed_total` 和
  `nexus_lane_governance_sealed_aliases` 因此 Alertmanager 可以在任何时候发出警告
  车道失去其清单。即使对于开发网络也保持启用这些警报。
- 调度程序遥测地图和车道治理仪表板
  (`dashboards/grafana/nexus_lanes.json`) 期望别名/slug 字段来自
  目录。如果重命名别名，请重新标记相应的 Kura 目录，以便
  审计员保持确定性路径（在 NX-1 下跟踪）。
- 议会对默认车道的批准应包括回滚计划。记录
  清单哈希和治理证据以及本快速入门
  操作员运行手册，以便将来的轮换不会猜测所需的状态。

一旦这些检查通过，您就可以将 `nexus.routing_policy.default_lane` 视为
网络上的代码路径。