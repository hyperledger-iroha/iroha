---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7cc63e7549adebfe3ab539eca608e2fc88830361b3fe53b165491e36ecb83177
source_last_modified: "2026-01-22T14:35:36.748626+00:00"
translation_last_reviewed: 2026-02-07
id: pin-registry-plan
title: SoraFS Pin Registry Implementation Plan
sidebar_label: Pin Registry Plan
description: SF-4 implementation plan covering registry state machine, Torii facade, tooling, and observability.
translator: machine-google-reviewed
---

:::注意规范来源
:::

# SoraFS 密码注册实施计划 (SF-4)

SF-4 提供 Pin 注册合同和支持服务，存储
明确承诺，实施 pin 策略，并将 API 公开给 Torii、网关、
和协调者。本文件用具体内容扩展了验证计划
实现任务，涵盖链上逻辑、主机端服务、固定装置、
和操作要求。

## 范围

1. **注册表状态机**：Norito 定义的清单、别名、
   后继链、保留纪元和治理元数据。
2. **合约实现**：pin生命周期的确定性CRUD操作
   （`ReplicationOrder`、`Precommit`、`Completion`、驱逐）。
3. **服务门面**：由 Torii 的注册表支持的 gRPC/REST 端点
   SDK 消耗，包括分页和证明。
4. **工具和固定装置**：CLI 帮助程序、测试向量和要保留的文档
   清单、别名和治理信封同步。
5. **遥测和操作**：注册表健康状况的指标、警报和运行手册。

## 数据模型

### 核心记录 (Norito)

|结构|描述 |领域 |
|--------|-------------|--------|
| `PinRecordV1` |规范清单条目。 | `manifest_cid`、`chunk_plan_digest`、`por_root`、`profile_handle`、`approved_at`、`retention_epoch`、`pin_policy`、`successor_of`、 `governance_envelope_hash`。 |
| `AliasBindingV1` |映射别名 -> 清单 CID。 | `alias`、`manifest_cid`、`bound_at`、`expiry_epoch`。 |
| `ReplicationOrderV1` |提供者固定清单的说明。 | `order_id`、`manifest_cid`、`providers`、`redundancy`、`deadline`、`policy_hash`。 |
| `ReplicationReceiptV1` |提供商确认。 | `order_id`、`provider_id`、`status`、`timestamp`、`por_sample_digest`。 |
| `ManifestPolicyV1` |治理政策快照。 | `min_replicas`、`max_retention_epochs`、`allowed_profiles`、`pin_fee_basis_points`。 |

实现参考：参见 `crates/sorafs_manifest/src/pin_registry.rs`
Rust Norito 模式和支持这些记录的验证助手。验证
镜像清单工具（分块注册表查找、引脚策略门控），因此
契约、Torii 外观和 CLI 共享相同的不变量。

任务：
- 最终确定 `crates/sorafs_manifest/src/pin_registry.rs` 中的 Norito 架构。
- 使用 Norito 宏生成代码（Rust + 其他 SDK）。
- 一旦架构落地，更新文档（`sorafs_architecture_rfc.md`）。

## 合同执行

|任务|所有者 |笔记|
|------|----------|--------|
|实施注册表存储（sled/sqlite/链下）或智能合约模块。 |核心基础设施/智能合约团队|提供确定性哈希，避免浮点。 |
|入口点：`submit_manifest`、`approve_manifest`、`bind_alias`、`issue_replication_order`、`complete_replication`、`evict_manifest`。 |核心基础设施|利用验证计划中的 `ManifestValidator`。别名绑定现在通过 `RegisterPinManifest`（Torii DTO 表面处理）流动，而专用 `bind_alias` 仍计划进行连续更新。 |
|状态转换：强制继承（清单 A -> B）、保留纪元、别名唯一性。 |治理委员会/核心基础设施|别名唯一性、保留限制和前任批准/停用检查现在位于 `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` 中；多跳连续检测和复制簿记保持开放。 |
|治理参数：从配置/治理状态加载 `ManifestPolicyV1`；允许通过治理事件进行更新。 |治理委员会|提供用于策略更新的 CLI。 |
|事件发射：发射 Norito 事件以进行遥测（`ManifestApproved`、`ReplicationOrderIssued`、`AliasBound`）。 |可观察性|定义事件模式+日志记录。 |

测试：
- 每个入口点的单元测试（正面+拒绝）。
- 继承链的属性测试（无循环，单调时期）。
- 通过生成随机清单（有界）进行模糊验证。

## 服务外观（Torii/SDK 集成）

|组件|任务|所有者 |
|------------|------|----------|
| Torii 服务 |公开 `/v2/sorafs/pin`（提交）、`/v2/sorafs/pin/{cid}`（查找）、`/v2/sorafs/aliases`（列表/绑定）、`/v2/sorafs/replication`（订单/收据）。提供分页+过滤。 |网络 TL/核心基础设施 |
|认证|在响应中包含注册表高度/哈希值；添加 SDK 使用的 Norito 证明结构。 |核心基础设施|
|命令行|使用 `pin submit`、`alias bind`、`order issue`、`registry export` 扩展 `sorafs_manifest_stub` 或新的 `sorafs_pin` CLI。 |工具工作组 |
| SDK |从 Norito 模式生成客户端绑定 (Rust/Go/TS)；添加集成测试。 | SDK 团队 |

操作：
- 为 GET 端点添加缓存层/ETag。
- 提供与 Torii 策略一致的速率限制/身份验证。

## 赛程和 CI

- 夹具目录：`crates/iroha_core/tests/fixtures/sorafs_pin_registry/` 存储由 `cargo run -p iroha_core --example gen_pin_snapshot` 重新生成的签名清单/别名/订单快照。
- CI 步骤：`ci/check_sorafs_fixtures.sh` 重新生成快照，如果出现差异则失败，保持 CI 装置对齐。
- 集成测试 (`crates/iroha_core/tests/pin_registry.rs`) 执行快乐路径加上重复别名拒绝、别名批准/保留保护、不匹配的分块器句柄、副本计数验证和后继保护失败（未知/预先批准/退休/自指针）；有关承保范围的详细信息，请参阅 `register_manifest_rejects_*` 案例。
- 单元测试现在涵盖 `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` 中的别名验证、保留保护和后继检查；状态机着陆后进行多跳连续检测。
- 可观测性管道使用的事件的黄金 JSON。

## 遥测和可观测性

指标（Prometheus）：
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- 现有的提供商遥测（`torii_sorafs_capacity_*`、`torii_sorafs_fee_projection_nanos`）仍然在端到端仪表板的范围内。

日志：
- 用于治理审计的结构化 Norito 事件流（已签名？）。

警报：
- 待处理的复制订单超出 SLA。
- 别名到期 < 阈值。
- 违反保留规定（清单在到期前未续订）。

仪表板：
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` 跟踪清单生命周期总数、别名覆盖率、积压饱和度、SLA 比率、延迟与松弛覆盖以及待命审核的错过订单率。

## 操作手册和文档

- 更新 `docs/source/sorafs/migration_ledger.md` 以包括注册表状态更新。
- 操作员指南：`docs/source/sorafs/runbooks/pin_registry_ops.md`（现已发布），涵盖指标、警报、部署、备份和恢复流程。
- 治理指南：描述政策参数、审批流程、争议处理。
- 每个端点的 API 参考页（Docusaurus 文档）。

## 依赖关系和排序

1. 完成验证计划任务（ManifestValidator 集成）。
2. 最终确定 Norito 架构 + 策略默认值。
3.实行合同+服务，有线遥测。
4. 重新生成装置，运行集成套件。
5. 更新文档/操作手册并将路线图项目标记为完成。

SF-4 下的每个路线图清单项目在取得进展时都应参考该计划。
REST 外观现在附带经过验证的列表端点：

- `GET /v2/sorafs/pin` 和 `GET /v2/sorafs/pin/{digest}` 返回清单
  别名绑定、复制顺序和从派生的证明对象
  最新的块哈希。
- `GET /v2/sorafs/aliases` 和 `GET /v2/sorafs/replication` 暴露活动
  别名目录和复制订单积压具有一致的分页和
  状态过滤器。

CLI 包装这些调用（`iroha app sorafs pin list`、`pin show`、`alias list`、
`replication list`），因此操作员可以编写注册表审核脚本而无需接触
较低级别的 API。