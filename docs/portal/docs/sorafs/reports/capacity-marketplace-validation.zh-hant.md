---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7ffbb145e1e0aa9dc71bdb6896c4f8be69eb6226194c5c165905af1ac243cc9
source_last_modified: "2025-12-29T18:16:35.199832+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Capacity Marketplace Validation
tags: [SF-2c, acceptance, checklist]
summary: Acceptance checklist covering provider onboarding, dispute workflows, and treasury reconciliation gating the SoraFS capacity marketplace general availability.
translator: machine-google-reviewed
---

# SoraFS 容量市場驗證清單

**審核時間：** 2026-03-18 → 2026-03-24  
**計劃所有者：** 存儲團隊 (`@storage-wg`)、治理委員會 (`@council`)、財政協會 (`@treasury`)  
**範圍：** SF-2c GA 所需的提供商入職管道、爭議裁決流程和財務協調流程。

在為外部運營商啟用市場之前，必須檢查以下清單。每行都鏈接到審核員可以重播的確定性證據（測試、固定裝置或文檔）。

## 驗收清單

### 提供商入職

|檢查 |驗證 |證據|
|--------|------------|----------|
|註冊管理機構接受規範的容量聲明 |集成測試通過應用程序 API 練習 `/v1/sorafs/capacity/declare`，驗證簽名處理、元數據捕獲以及移交給節點註冊表。 | `crates/iroha_torii/src/routing.rs:7654` |
|智能合約拒絕不匹配的有效負載 |單元測試確保提供者 ID 和提交的 GiB 字段在持久化之前與簽名的聲明匹配。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI 發出規範的入門工件 | CLI 工具寫入確定性 Norito/JSON/Base64 輸出並驗證往返，以便操作員可以離線進行聲明。 | `crates/sorafs_car/tests/capacity_cli.rs:17` |
|操作員指南捕獲准入工作流程和治理護欄 |文檔列舉了理事會的聲明模式、默認策略和審查步驟。 | `../storage-capacity-marketplace.md` |

### 爭議解決

|檢查 |驗證 |證據|
|--------|------------|----------|
|爭議記錄與規範有效負載摘要保持一致 |單元測試註冊爭議，解碼存儲的有效負載，並斷言待處理狀態以保證賬本的確定性。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI 爭議生成器匹配規範模式 | CLI 測試涵蓋 Base64/Norito 輸出和 `CapacityDisputeV1` 的 JSON 摘要，確保證據包確定性地散列。 | `crates/sorafs_car/tests/capacity_cli.rs:455` |
|重播測試證明爭議/處罰決定論 |重播兩次的失敗證明遙測會產生相同的賬本、信用和爭議快照，因此斜線在同行之間是確定性的。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook 記錄升級和撤銷流程 |操作指南涵蓋了理事會工作流程、證據要求和回滾程序。 | `../dispute-revocation-runbook.md` |

### 國庫對賬

|檢查 |驗證 |證據|
|--------|------------|----------|
|賬本應計與 30 天浸泡預測相匹配 |浸泡測試涵蓋 5 個提供商的 30 個結算窗口，將賬本條目與預期支付參考進行比較。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
|每晚記錄賬本出口對賬 | `capacity_reconcile.py` 將費用賬本預期與執行的 XOR 轉賬導出進行比較，發出 Prometheus 指標，並通過 Alertmanager 獲得財務部批准。 | `scripts/telemetry/capacity_reconcile.py:1`，`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`，`dashboards/alerts/sorafs_capacity_rules.yml:100` |
|計費儀表板表面處罰和應計遙測| Grafana 導入繪製 GiB·小時累計、執行計數器和保稅抵押品以實現隨叫隨到的可見性。 | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
|已發布的報告存檔浸泡方法和重放命令 |報告詳細介紹了審計人員的浸泡範圍、執行命令和可觀察性掛鉤。 | `./sf2c-capacity-soak.md` |

## 執行注意事項

在簽核之前重新運行驗證套件：

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

運營商應使用 `sorafs_manifest_stub capacity {declaration,dispute}` 重新生成加入/爭議請求有效負載，並將生成的 JSON/Norito 字節與治理票證一起存檔。

## 簽核工件

|文物|路徑|布萊克2b-256 |
|----------|------|-------------|
|提供商入職審批包 | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
|爭議解決審批包| `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
|國庫調節批准包| `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

將這些工件的簽名副本與發布包一起存儲，並將它們鏈接到治理變更記錄中。

## 批准

- 存儲團隊負責人 — @storage-tl (2026-03-24)  
- 治理委員會秘書 — @council-sec (2026-03-24)  
- 資金運營主管 — @treasury-ops (2026-03-24)