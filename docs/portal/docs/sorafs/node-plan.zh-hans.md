---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/node-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3852a0f039b664344f9cbce7d2514172cfe97cd838b68755f764d4fe183b22cc
source_last_modified: "2026-01-05T09:28:11.898207+00:00"
translation_last_reviewed: 2026-02-07
id: node-plan
title: SoraFS Node Implementation Plan
sidebar_label: Node Implementation Plan
description: Translate the SF-3 storage roadmap into actionable engineering work with milestones, tasks, and test coverage.
translator: machine-google-reviewed
---

:::注意规范来源
:::

SF-3 提供了第一个可运行的 `sorafs-node` 包，可将 Iroha/Torii 进程转变为 SoraFS 存储提供程序。在对可交付成果进行排序时，请将此计划与[节点存储指南](node-storage.md)、[提供商准入政策](provider-admission-policy.md) 和[存储容量市场路线图](storage-capacity-marketplace.md) 一起使用。

## 目标范围（里程碑 M1）

1. **块存储集成。** 使用持久后端包装 `sorafs_car::ChunkStore`，该后端在配置的数据目录中存储块字节、清单和 PoR 树。
2. **网关端点。** 公开 Norito HTTP 端点，用于 Torii 进程中的 pin 提交、块获取、PoR 采样和存储遥测。
3. **配置管道。** 添加通过 `iroha_config`、`iroha_core` 和 `iroha_torii` 连接的 `SoraFsStorage` 配置结构（启用标志、容量、目录、并发限制）。
4. **配额/调度。** 通过背压强制执行操作员定义的磁盘/并行度限制和队列请求。
5. **遥测。** 发出 pin 成功、块获取延迟、容量利用率和 PoR 采样结果的指标/日志。

## 工作分解

### A. 板条箱和模块结构

|任务|所有者 |笔记|
|------|----------|--------|
|使用以下模块创建 `crates/sorafs_node`：`config`、`store`、`gateway`、`scheduler`、`telemetry`。 |存储团队|重新导出可重用类型以进行 Torii 集成。 |
|实现从 `SoraFsStorage` 映射的 `StorageConfig`（用户 → 实际 → 默认值）。 |存储团队/配置工作组 |确保 Norito/`iroha_config` 层保持确定性。 |
|提供 `NodeHandle` 外观 Torii 用于提交 pin/fetch。 |存储团队|封装存储内部结构和异步管道。 |

### B. 持久块存储

|任务|所有者 |笔记|
|------|----------|--------|
|使用磁盘清单索引 (`sled`/`sqlite`) 构建磁盘后端包装 `sorafs_car::ChunkStore`。 |存储团队|确定性布局：`<data_dir>/<manifest_cid>/chunk_{idx}.bin`。 |
|使用 `ChunkStore::sample_leaves` 维护 PoR 元数据（64KiB/4KiB 树）。 |存储团队|支持重启后回放；腐败问题快速失败。 |
|在启动时实施完整性重放（重新散列清单、修剪不完整的引脚）。 |存储团队|阻止 Torii 启动，直到重播完成。 |

### C. 网关端点

|端点 |行为 |任务 |
|----------|------------|--------|
| `POST /sorafs/pin` |接受 `PinProposalV1`，验证清单，队列摄取，使用清单 CID 进行响应。 |验证块配置文件、强制配额、通过块存储传输数据。 |
| `GET /sorafs/chunks/{cid}` + 范围查询 |使用 `Content-Chunker` 标头提供块字节；遵守范围能力规范。 |使用调度程序 + 流预算（与 SF-2d 范围功能相关）。 |
| `POST /sorafs/por/sample` |对清单和退货证明包运行 PoR 采样。 |重用块存储采样，使用 Norito JSON 有效负载进行响应。 |
| `GET /sorafs/telemetry` |摘要：容量、PoR 成功、获取错误计数。 |为仪表板/操作员提供数据。 |

运行时管道通过 `sorafs_node::por` 线程 PoR 交互：跟踪器记录每个 `PorChallengeV1`、`PorProofV1` 和 `AuditVerdictV1`，因此 `CapacityMeter` 指标反映治理结论，无需定制 Torii逻辑.【crates/sorafs_node/src/scheduler.rs#L147】

实施注意事项：

- 将 Torii 的 Axum 堆栈与 `norito::json` 有效负载结合使用。
- 添加 Norito 响应模式（`PinResultV1`、`FetchErrorV1`、遥测结构）。

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` 现在公开积压深度以及最旧的纪元/截止日期和
  每个提供商最近的成功/失败时间戳，由
  `sorafs_node::NodeHandle::por_ingestion_status` 和 Torii 记录
  `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` 仪表仪表板.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. 调度程序和配额执行

|任务|详情 |
|------|---------|
|磁盘配额 |跟踪磁盘上的字节；当超过 `max_capacity_bytes` 时拒绝新的引脚。为未来的政策提供驱逐钩子。 |
|获取并发|全局信号量 (`max_parallel_fetches`) 加上来自 SF-2d 范围上限的每个提供商的预算。 |
|引脚队列 |限制未完成的摄取作业；公开队列深度的 Norito 状态端点。 |
| PoR 节奏 |由 `por_sample_interval_secs` 驱动的后台工作者。 |

### E. 遥测和记录

指标（Prometheus）：

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds`（带有 `result` 标签的直方图）
- `torii_sorafs_storage_bytes_used`、`torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`、`torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`、`torii_sorafs_storage_por_samples_failed_total`

日志/事件：

- 用于治理摄取的结构化 Norito 遥测 (`StorageTelemetryV1`)。
- 当利用率 >90% 或 PoR 故障连续超过阈值时发出警报。

### F. 测试策略

1. **单元测试。** 块存储持久性、配额计算、调度程序不变量（请参阅 `crates/sorafs_node/src/scheduler.rs`）。  
2. **集成测试** (`crates/sorafs_node/tests`)。 Pin → 取回往返、重启恢复、配额拒绝、PoR 采样证明验证。  
3. **Torii 集成测试。** 在启用存储的情况下运行 Torii，通过 `assert_cmd` 执行 HTTP 端点。  
4. **混乱路线图。** 未来的演练将模拟磁盘耗尽、IO 缓慢、提供程序删除。

## 依赖关系

- SF-2b 准入策略 — 确保节点在发布广告之前验证准入信封。  
- SF-2c 容量市场 — 将遥测技术与容量声明联系起来。  
- SF-2d 广告扩展 — 消耗范围能力 + 可用的流预算。

## 里程碑退出标准

- `cargo run -p sorafs_node --example pin_fetch` 适用于本地装置。  
- Torii 使用 `--features sorafs-storage` 构建并通过集成测试。  
- 使用默认配置+ CLI 示例更新了文档（[节点存储指南](node-storage.md)）；提供操作手册。  
- 遥测在暂存仪表板中可见；针对容量饱和和 PoR 故障配置的警报。

## 文档和运营交付成果

- 使用默认配置、CLI 用法和故障排除步骤更新[节点存储参考](node-storage.md)。  
- 随着 SF-3 的发展，保持[节点操作运行手册](node-operations.md) 与实施保持一致。  
- 在开发人员门户内发布 `/sorafs/*` 端点的 API 参考，并在 Torii 处理程序登陆后将它们连接到 OpenAPI 清单中。