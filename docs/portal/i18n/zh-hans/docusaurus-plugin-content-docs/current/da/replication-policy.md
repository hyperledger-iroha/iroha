---
lang: zh-hans
direction: ltr
source: docs/portal/docs/da/replication-policy.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

标题：数据可用性复制策略
sidebar_label：复制策略
描述：适用于所有 DA 摄取提交的治理强制保留配置文件。
---

:::注意规范来源
:::

# 数据可用性复制策略 (DA-4)

_状态：进行中 — 所有者：核心协议工作组/存储团队/SRE_

DA 摄取管道现在强制执行确定性保留目标
`roadmap.md`（工作流 DA-4）中描述的每个 Blob 类。 Torii 拒绝
保留调用者提供的与配置不匹配的保留信封
策略，保证每个验证器/存储节点保留所需的
纪元和副本的数量，而不依赖于提交者的意图。

## 默认策略

|斑点类|热保持|保冷|所需副本 |存储类|治理标签|
|------------------------|--------------|----------------|--------------------------------|----------------|----------------|
| `taikai_segment` | 24小时| 14 天 | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 小时 | 7 天 | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 小时 | 180 天 | 3 | `cold` | `da.governance` |
| _默认（所有其他类）_ | 6 小时 | 30 天 | 3 | `warm` | `da.default` |

这些值嵌入在 `torii.da_ingest.replication_policy` 中并应用于
所有 `/v1/da/ingest` 提交内容。 Torii 使用强制重写清单
保留配置文件并在调用者提供不匹配的值时发出警告，以便
运营商可以检测过时的 SDK。

### Taikai 可用性课程

Taikai 路由清单 (`taikai.trm`) 声明 `availability_class`
（`hot`、`warm` 或 `cold`）。 Torii 在分块之前强制执行匹配策略
因此操作员可以扩展每个流的副本数量，而无需编辑全局
表。默认值：

|可用性等级 |热保持|保冷|所需副本 |存储类|治理标签|
|--------------------|-------------|----------------|--------------------------------|----------------|----------------|
| `hot` | 24小时| 14 天 | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 小时 | 30 天 | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1小时| 180 天 | 3 | `cold` | `da.taikai.archive` |

缺失提示默认为 `hot`，因此直播保留最强策略。
通过覆盖默认值
`torii.da_ingest.replication_policy.taikai_availability` 如果您的网络使用
不同的目标。

## 配置

该保单位于 `torii.da_ingest.replication_policy` 下，并公开了
*默认*模板加上每个类覆盖的数组。类标识符是
不区分大小写并接受 `taikai_segment`、`nexus_lane_sidecar`、
`governance_artifact` 或 `custom:<u16>` 用于治理批准的扩展。
存储类别接受 `hot`、`warm` 或 `cold`。

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

保持块不变，以使用上面列出的默认值运行。拧紧一个
类，更新匹配的覆盖；更改新类的基线，
编辑 `default_retention`。

Taikai 可用性类可以通过独立覆盖
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

## 强制语义

- Torii 使用强制配置文件替换用户提供的 `RetentionPolicy`
  在分块或显式发射之前。
- 声明不匹配保留配置文件的预构建清单将被拒绝
  与 `400 schema mismatch` 因此过时的客户不能削弱合同。
- 记录每个覆盖事件（`blob_class`，已提交与预期策略）
  在推出期间显示不合规的呼叫者。

请参阅[数据可用性摄取计划](ingest-plan.md)（验证清单）了解更新后的门
涵盖保留执行。

## 重新复制工作流程（DA-4 后续）

保留强制执行只是第一步。运营商还必须证明
实时清单和复制订单与配置的策略保持一致，因此
SoraFS 可以自动重新复制不合规的 blob。

1. **注意漂移。** Torii 发出
   `overriding DA retention policy to match configured network baseline` 每当
   调用者提交过时的保留值。将该日志与
   `torii_sorafs_replication_*` 遥测发现副本短缺或延迟
   重新部署。
2. **意图与实时副本的区别。** 使用新的审计助手：

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   该命令从提供的加载 `torii.da_ingest.replication_policy`
   配置，解码每个清单（JSON 或 Norito），并可选择匹配任何
   `ReplicationOrderV1` 清单摘要的有效负载。摘要标记了两个
   条件：

   - `policy_mismatch` – 清单保留配置文件与强制执行的不同
     策略（除非 Torii 配置错误，否则这种情况永远不会发生）。
   - `replica_shortfall` – 实时复制订单请求的副本数少于
     `RetentionPolicy.required_replicas` 或提供比其更少的分配
     目标。

   非零退出状态表示存在活跃短缺，因此 CI/on-call 自动化
   可以立即寻呼。将 JSON 报告附加到
   `docs/examples/da_manifest_review_template.md`
   议会投票的数据包。
3. **触发重新复制。** 当审计报告不足时，发出新的
   `ReplicationOrderV1` 通过中描述的治理工具
   [SoraFS存储容量市场](../sorafs/storage-capacity-marketplace.md)并重新运行审核
   直到副本集收敛。对于紧急覆盖，请将 CLI 输出配对
   与 `iroha app da prove-availability` 以便 SRE 可以引用相同的摘要
   和 PDP 证据。

回归覆盖范围位于 `integration_tests/tests/da/replication_policy.rs` 中；
该套件向 `/v1/da/ingest` 提交不匹配的保留策略并验证
获取的清单公开强制配置文件而不是调用者
意图。