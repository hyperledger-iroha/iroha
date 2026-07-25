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
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` |映射别名 -> 清单 CID。 | `alias`、`manifest_cid`、`bound_at`、`expiry_epoch`。 |
| `ReplicationOrderV1` |提供者固定清单的说明。 | `order_id`、`manifest_cid`、`providers`、`redundancy`、`deadline`、`policy_hash`。 |
| `ReplicationReceiptV1` |提供商确认。 | `order_id`、`provider_id`、`status`、`timestamp`、`por_sample_digest`。 |
| `ManifestPolicyV1` |治理政策快照。 | `min_replicas`、`max_retention_epochs`、`allowed_profiles`、`pin_fee_basis_points`。 |

Implementation reference: the authoritative manifest lifecycle and finalized
read schemas live in `crates/iroha_data_model/src/sorafs/pin_registry.rs`.
Supporting alias, replication, and policy envelopes live in
`crates/sorafs_manifest/src/pin_registry.rs`. Consensus admission derives and
validates the stored commitments; Torii and operator tooling consume the exact
native finalized record rather than maintaining a second pin-record format.

Status:
- The native `PinManifestRecord` and `PinManifestFinalizedRecordV1` are the V1
  manifest-registry surface used by core, Torii, fixtures, and reference
  validators.
- Rust code generation uses Norito derives; SDK parity follows the normal guard
  lanes whenever the native schema changes.
- Architecture, manifest-pipeline, CLI, OpenAPI, status, and roadmap documents
  describe the shared validation path and endpoint behavior.

## Contract Implementation

| Task | Owner(s) | Notes |
|------|----------|-------|
| Registry storage and smart-contract state. | Core Infra / Smart Contract Team | Implemented in Iroha world state (`pin_manifests`, `manifest_aliases`, `replication_orders`) with deterministic Norito payload hashing and integer-only policy arithmetic. |
| Entry points: `RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`, `BindManifestAlias`, `IssueReplicationOrder`, `CompleteReplicationOrder`, `ExpireReplicationOrder`. | Core Infra | Registration carries the complete canonical manifest, resource-bounds and validates it in consensus, and derives all stored commitments. Core execution also validates aliases, council envelopes, governance permissions, canonical replication payloads, completion, and deadline-bound expiration. |
| State transitions: enforce succession (manifest A -> B), retention epochs, alias uniqueness, and replication status changes. | Governance Council / Core Infra | `ensure_successor_chain` enforces approved, non-retired, acyclic multi-hop lineage; alias uniqueness, retention, and replication issue/complete bookkeeping are covered by unit tests. |
| Governed parameters: load `ManifestPolicyV1` from config/governance state. | Governance Council | Runtime config maps pin-policy constraints into the shared validator. Live policy-change ceremonies are rollout governance evidence, not missing local contract code. |
| Registry telemetry and audit surface. | Observability | Torii exports registry metrics and attested REST snapshots. Additional signed event archives can be layered over those snapshots if governance requires them. |

Coverage:
- Unit tests cover registration, approval, retirement, alias binding, replication
  order issue/complete, permissions, duplicate rejection, and side-effect-free
  failure paths.
- Successor tests cover self references, unknown/pending/retired predecessors,
  cycle closure, and malformed existing predecessor cycles.
- `ci/check_sorafs_fixtures.sh` regenerates chunker, provider-admission, and pin
  registry fixtures and runs the parity checks that keep the canonical schema
  surface stable.

## Service Facade (Torii/SDK Integration)

| Component | Task | Owner(s) |
|-----------|------|----------|
| Torii Service | Ships `/v1/sorafs/pin`, `/v1/sorafs/pin/{digest_hex}`, `/v1/sorafs/aliases`, and `/v1/sorafs/replication`. The manifest-detail route returns exact native `PinManifestFinalizedRecordV1` JSON and accepts only the optional paired expected finalized height/hash precondition; pagination and filters remain on list routes. | Networking TL / Core Infra |
| Finality binding | Listing responses retain their listing attestation. A manifest-detail response carries the native `finalized_cursor` beside the authoritative `PinManifestRecord`; a stale requested cursor fails with HTTP 409. | Core Infra |
| CLI | `iroha app sorafs pin register`, `pin list`, `pin show`, `alias list`, and `replication list` wrap the REST and ISI surfaces for operator audits. | Tooling WG |
| SDK | Rust request builders and the JavaScript, Python, Swift, and C# guard lanes mirror the manifest payload and pin-register validation surface. | SDK Teams |

Operations:
- List endpoints use attested snapshots, deterministic pagination, and the cache
  behavior documented in the alias policy where alias proofs are involved.
- `GET /v1/sorafs/pin/{digest_hex}` returns only `finalized_cursor` and the
  native `manifest`. The retired `limit`, attestation, embedded alias/order
  arrays, counts, and truncation fields are absent; callers use
  `/v1/sorafs/aliases` and `/v1/sorafs/replication` for bounded list queries.
- Mutating operations go through ISI/governance permissions; REST handling keeps
  the same Torii auth and resource-guard model as the surrounding SoraFS APIs.

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

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` 和 `GET /v1/sorafs/replication` 暴露活动
  别名目录和复制订单积压具有一致的分页和
  状态过滤器。

CLI 包装这些调用（`iroha app sorafs pin list`、`pin show`、`alias list`、
`replication list`），因此操作员可以编写注册表审核脚本而无需接触
较低级别的 API。