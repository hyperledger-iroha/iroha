---
lang: zh-hant
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
| `PinRecordV1` |規范清單條目。 | `manifest_cid`、`chunk_plan_digest`、`por_root`、`profile_handle`、`approved_at`、`retention_epoch`、`pin_policy`、`successor_of`、 `governance_envelope_hash`。 |
| `AliasBindingV1` |映射別名 -> 清單 CID。 | `alias`、`manifest_cid`、`bound_at`、`expiry_epoch`。 |
| `ReplicationOrderV1` |提供者固定清單的說明。 | `order_id`、`manifest_cid`、`providers`、`redundancy`、`deadline`、`policy_hash`。 |
| `ReplicationReceiptV1` |提供商確認。 | `order_id`、`provider_id`、`status`、`timestamp`、`por_sample_digest`。 |
| `ManifestPolicyV1` |治理政策快照。 | `min_replicas`、`max_retention_epochs`、`allowed_profiles`、`pin_fee_basis_points`。 |

實現參考：參見 `crates/sorafs_manifest/src/pin_registry.rs`
Rust Norito 模式和支持這些記錄的驗證助手。驗證
鏡像清單工具（分塊註冊表查找、引腳策略門控），因此
契約、Torii 外觀和 CLI 共享相同的不變量。

任務：
- 最終確定 `crates/sorafs_manifest/src/pin_registry.rs` 中的 Norito 架構。
- 使用 Norito 宏生成代碼（Rust + 其他 SDK）。
- 一旦架構落地，更新文檔（`sorafs_architecture_rfc.md`）。

## 合同執行

|任務|所有者 |筆記|
|------|----------|--------|
|實施註冊表存儲（sled/sqlite/鏈下）或智能合約模塊。 |核心基礎設施/智能合約團隊|提供確定性哈希，避免浮點。 |
|入口點：`submit_manifest`、`approve_manifest`、`bind_alias`、`issue_replication_order`、`complete_replication`、`evict_manifest`。 |核心基礎設施|利用驗證計劃中的 `ManifestValidator`。別名綁定現在通過 `RegisterPinManifest`（Torii DTO 表面處理）流動，而專用 `bind_alias` 仍計劃進行連續更新。 |
|狀態轉換：強制繼承（清單 A -> B）、保留紀元、別名唯一性。 |治理委員會/核心基礎設施|別名唯一性、保留限制和前任批准/停用檢查現在位於 `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` 中；多跳連續檢測和復制簿記保持開放。 |
|治理參數：從配置/治理狀態加載 `ManifestPolicyV1`；允許通過治理事件進行更新。 |治理委員會|提供用於策略更新的 CLI。 |
|事件發射：發射 Norito 事件以進行遙測（`ManifestApproved`、`ReplicationOrderIssued`、`AliasBound`）。 |可觀察性|定義事件模式+日誌記錄。 |

測試：
- 每個入口點的單元測試（正面+拒絕）。
- 繼承鏈的屬性測試（無循環，單調時期）。
- 通過生成隨機清單（有界）進行模糊驗證。

## 服務外觀（Torii/SDK 集成）

|組件|任務|所有者 |
|------------|------|----------|
| Torii 服務 |公開 `/v1/sorafs/pin`（提交）、`/v1/sorafs/pin/{cid}`（查找）、`/v1/sorafs/aliases`（列表/綁定）、`/v1/sorafs/replication`（訂單/收據）。提供分頁+過濾。 |網絡 TL/核心基礎設施 |
|認證|在響應中包含註冊表高度/哈希值；添加 SDK 使用的 Norito 證明結構。 |核心基礎設施|
|命令行|使用 `pin submit`、`alias bind`、`order issue`、`registry export` 擴展 `sorafs_manifest_stub` 或新的 `sorafs_pin` CLI。 |工具工作組 |
| SDK |從 Norito 模式生成客戶端綁定 (Rust/Go/TS)；添加集成測試。 | SDK 團隊 |

操作：
- 為 GET 端點添加緩存層/ETag。
- 提供與 Torii 策略一致的速率限制/身份驗證。

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

- `GET /v1/sorafs/pin` 和 `GET /v1/sorafs/pin/{digest}` 返回清單
  別名綁定、複製順序和從派生的證明對象
  最新的塊哈希。
- `GET /v1/sorafs/aliases` 和 `GET /v1/sorafs/replication` 暴露活動
  別名目錄和復制訂單積壓具有一致的分頁和
  狀態過濾器。

CLI 包裝這些調用（`iroha app sorafs pin list`、`pin show`、`alias list`、
`replication list`），因此操作員可以編寫註冊表審核腳本而無需接觸
較低級別的 API。