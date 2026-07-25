---
id: pin-registry-plan
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Pin Registry Implementation Plan
sidebar_label: Pin Registry Plan
description: SF-4 implementation plan covering registry state machine, Torii facade, tooling, and observability.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
:::

# SoraFS 密碼註冊實施計劃 (SF-4)

SF-4 提供 Pin 註冊合同和支持服務，存儲
明確承諾，實施 pin 策略，並將 API 公開給 Torii、網關、
和協調者。本文件用具體內容擴展了驗證計劃
實現任務，涵蓋鏈上邏輯、主機端服務、固定裝置、
和操作要求。

## 範圍

1. **註冊表狀態機**：Norito 定義的清單、別名、
   後繼鏈、保留紀元和治理元數據。
2. **合約實現**：pin生命週期的確定性CRUD操作
   （`ReplicationOrder`、`Precommit`、`Completion`、驅逐）。
3. **服務門面**：由 Torii 的註冊表支持的 gRPC/REST 端點
   SDK 消耗，包括分頁和證明。
4. **工具和固定裝置**：CLI 幫助程序、測試向量和要保留的文檔
   清單、別名和治理信封同步。
5. **遙測和操作**：註冊表健康狀況的指標、警報和運行手冊。

## 數據模型

### 核心記錄 (Norito)

|結構|描述 |領域 |
|--------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` |映射別名 -> 清單 CID。 | `alias`、`manifest_cid`、`bound_at`、`expiry_epoch`。 |
| `ReplicationOrderV1` |提供者固定清單的說明。 | `order_id`、`manifest_cid`、`providers`、`redundancy`、`deadline`、`policy_hash`。 |
| `ReplicationReceiptV1` |提供商確認。 | `order_id`、`provider_id`、`status`、`timestamp`、`por_sample_digest`。 |
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

## 賽程和 CI

- 夾具目錄：`crates/iroha_core/tests/fixtures/sorafs_pin_registry/` 存儲由 `cargo run -p iroha_core --example gen_pin_snapshot` 重新生成的簽名清單/別名/訂單快照。
- CI 步驟：`ci/check_sorafs_fixtures.sh` 重新生成快照，如果出現差異則失敗，保持 CI 裝置對齊。
- 集成測試 (`crates/iroha_core/tests/pin_registry.rs`) 執行快樂路徑加上重複別名拒絕、別名批准/保留保護、不匹配的分塊器句柄、副本計數驗證和後繼保護失敗（未知/預先批准/退休/自指針）；有關承保範圍的詳細信息，請參閱 `register_manifest_rejects_*` 案例。
- 單元測試現在涵蓋 `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` 中的別名驗證、保留保護和後繼檢查；狀態機著陸後進行多跳連續檢測。
- 可觀測性管道使用的事件的黃金 JSON。

## 遙測和可觀測性

指標（Prometheus）：
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- 現有的提供商遙測（`torii_sorafs_capacity_*`、`torii_sorafs_fee_projection_nanos`）仍然在端到端儀表板的範圍內。

日誌：
- 用於治理審計的結構化 Norito 事件流（已簽名？）。

警報：
- 待處理的複制訂單超出 SLA。
- 別名到期 < 閾值。
- 違反保留規定（清單在到期前未續訂）。

儀表板：
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` 跟踪清單生命週期總數、別名覆蓋率、積壓飽和度、SLA 比率、延遲與鬆弛覆蓋以及待命審核的錯過訂單率。

## 操作手冊和文檔

- 更新 `docs/source/sorafs/migration_ledger.md` 以包括註冊表狀態更新。
- 操作員指南：`docs/source/sorafs/runbooks/pin_registry_ops.md`（現已發布），涵蓋指標、警報、部署、備份和恢復流程。
- 治理指南：描述政策參數、審批流程、爭議處理。
- 每個端點的 API 參考頁（Docusaurus 文檔）。

## 依賴關係和排序

1. 完成驗證計劃任務（ManifestValidator 集成）。
2. 最終確定 Norito 架構 + 策略默認值。
3.實行合同+服務，有線遙測。
4. 重新生成裝置，運行集成套件。
5. 更新文檔/操作手冊並將路線圖項目標記為完成。

SF-4 下的每個路線圖清單項目在取得進展時都應參考該計劃。
REST 外觀現在附帶經過驗證的列表端點：

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` 和 `GET /v1/sorafs/replication` 暴露活動
  別名目錄和復制訂單積壓具有一致的分頁和
  狀態過濾器。

CLI 包裝這些調用（`iroha app sorafs pin list`、`pin show`、`alias list`、
`replication list`），因此操作員可以編寫註冊表審核腳本而無需接觸
較低級別的 API。