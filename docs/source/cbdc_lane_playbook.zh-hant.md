---
lang: zh-hant
direction: ltr
source: docs/source/cbdc_lane_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b97d0dd24de65ba37cba0fa4d404235b63a771f49ac6ee8f07e4814d2a6db814
source_last_modified: "2026-01-22T16:26:46.564417+00:00"
translation_last_reviewed: 2026-02-07
title: CBDC Lane Playbook
sidebar_label: CBDC Lane Playbook
description: Reference configuration, whitelist flow, and compliance evidence for permissioned CBDC lanes on SORA Nexus.
translator: machine-google-reviewed
---

# CBDC 私人通道劇本 (NX-6)

> **路線圖鏈接：** NX-6（CBDC 專用通道模板和白名單流程）和 NX-14（Nexus 運行手冊）。  
> **所有者：** 金融服務工作組、Nexus 核心工作組、合規工作組。  
> **狀態：** 起草 — `crates/iroha_data_model::nexus`、`crates/iroha_core::governance::manifest` 和 `integration_tests/tests/nexus/lane_registry.rs` 中存在實現掛鉤，但缺少 CBDC 特定的清單、白名單和操作員運行手冊。本手冊記錄了參考配置和入門工作流程，以便 CBDC 部署可以確定地進行。

## 範圍和角色

- **中央銀行通道（“CBDC 通道”）：** 許可的驗證器、託管結算緩衝區和可編程貨幣政策。作為受限數據空間+通道對運行，具​​有自己的治理清單。
- **批發/零售銀行數據空間：** 持有 UAID、接收能力清單的參與者 DS，並且可能會被列入具有 CBDC 通道的原子 AXT 白名單。
- **可編程貨幣 dApp：** 一旦列入白名單，通過 `ComposabilityGroup` 路由消耗 CBDC 流的外部 DS。
- **治理與合規性：** 議會（或同等模塊）批准車道清單、能力清單和白名單更改；合規性將證據包與 Norito 清單一起存儲。

**依賴關係**

1. 巷目錄+數據空間目錄接線（`docs/source/nexus_lanes.md`、`defaults/nexus/config.toml`）。
2. 通道清單強制執行（`crates/iroha_core/src/governance/manifest.rs`，`crates/iroha_core/src/queue.rs` 中的隊列門控）。
3. 能力清單 + UAID (`crates/iroha_data_model/src/nexus/manifest.rs`)。
4.調度程序TEU配額+指標（`integration_tests/tests/scheduler_teu.rs`、`docs/source/telemetry.md`）。

## 1. 參考車道佈局

### 1.1 車道目錄和數據空間條目

將專用條目添加到 `[[nexus.lane_catalog]]` 和 `[[nexus.dataspace_catalog]]`。下面的示例使用 CBDC 通道擴展 `defaults/nexus/config.toml`，該通道為每個時隙預留 1500 個 TEU，並將飢餓限制為 6 個時隙，並為批發銀行和零售錢包添加匹配的數據空間別名。

```toml
lane_count = 5

[[nexus.lane_catalog]]
index = 3
alias = "cbdc"
description = "Central bank CBDC lane"
dataspace = "cbdc.core"
visibility = "restricted"
lane_type = "cbdc_private"
governance = "central_bank_multisig"
settlement = "xor_dual_fund"
metadata.scheduler.teu_capacity = "1500"
metadata.scheduler.starvation_bound_slots = "6"
metadata.settlement.buffer_account = "buffer::cbdc_treasury"
metadata.settlement.buffer_asset = "61CtjvNd9T3THAR65GsMVHr82Bjc"
metadata.settlement.buffer_capacity_micro = "1500000000"
metadata.telemetry.contact = "ops@cb.example"

[[nexus.dataspace_catalog]]
alias = "cbdc.core"
id = 10
description = "CBDC issuance dataspace"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "cbdc.bank.wholesale"
id = 11
description = "Wholesale bank onboarding lane"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "cbdc.dapp.retail"
id = 12
description = "Retail wallets and programmable-money dApps"
fault_tolerance = 1

[nexus.routing_policy]
default_lane = 0
default_dataspace = "universal"

[[nexus.routing_policy.rules]]
lane = 3
dataspace = "cbdc.core"
[nexus.routing_policy.rules.matcher]
instruction = "cbdc::*"
description = "Route CBDC contracts to the restricted lane"
```

**註釋**

- `metadata.scheduler.teu_capacity` 和 `metadata.scheduler.starvation_bound_slots` 為 `integration_tests/tests/scheduler_teu.rs` 所使用的 TEU 規格提供數據。操作員必須使其與驗收結果保持同步，以便 `nexus_scheduler_lane_teu_capacity` 與模板匹配。
- 上述每個數據空間別名必須出現在治理清單和功能清單中（見下文），以便准入自動拒絕漂移。

### 1.2 車道清單框架

Lane 清單位於通過 `nexus.registry.manifest_directory` 配置的目錄下（請參閱 `crates/iroha_config/src/parameters/actual.rs`）。文件名應與通道別名匹配 (`cbdc.manifest.json`)。該架構反映了 `integration_tests/tests/nexus/lane_registry.rs` 中的治理清單測試。

```json
{
  "lane": "cbdc",
  "dataspace": "cbdc.core",
  "version": 1,
  "governance": "central_bank_multisig",
  "validators": [
    "i105...",
    "i105...",
    "i105...",
    "i105..."
  ],
  "quorum": 3,
  "protected_namespaces": [
    "cbdc.core",
    "cbdc.policy",
    "cbdc.settlement"
  ],
  "hooks": {
    "runtime_upgrade": {
      "allow": true,
      "require_metadata": true,
      "metadata_key": "cbdc_upgrade_id",
      "allowed_ids": [
        "upgrade-issuance-v1",
        "upgrade-pilot-retail"
      ]
    }
  },
  "composability_group": {
    "group_id_hex": "7ab3f9b3b2777e9f8b3d6fae50264a1e0ffab7c74542ff10d67fbdd073d55710",
    "activation_epoch": 2048,
    "whitelist": [
      "ds::cbdc.bank.wholesale",
      "ds::cbdc.dapp.retail"
    ],
    "quotas": {
      "group_teu_share_max": 500,
      "per_ds_teu_max": 250
    }
  }
}
```

關鍵要求：- 驗證器**必須**是目錄中存在的規範 I105 帳戶 ID（無 `@domain`；僅附加 `@domain` 作為顯式路由提示）。將 `quorum` 設置為多重簽名閾值 (≥2)。
- 受保護的命名空間由 `Queue::push` 強制執行（請參閱 `crates/iroha_core/src/queue.rs`），因此所有 CBDC 合約必須指定 `gov_namespace` + `gov_contract_id`。
- `composability_group` 字段遵循 `docs/source/nexus.md` §8.6 中描述的模式；所有者（CBDC 通道）提供白名單和配額。白名單 DS 清單僅指定 `group_id_hex` + `activation_epoch`。
- 複製清單後，運行 `cargo test -p integration_tests nexus::lane_registry -- --nocapture` 以確認 `LaneManifestRegistry::from_config` 加載它。

### 1.3 能力清單（UAID策略）

能力清單（`crates/iroha_data_model/src/nexus/manifest.rs` 中的 `AssetPermissionManifest`）將 `UniversalAccountId` 綁定到確定性限額。通過 Space Directory 發布它們，以便銀行和 dApp 可以獲取簽名的策略。

```json
{
  "version": 1,
  "uaid": "uaid:5f77b4fcb89cb03a0ab8f46d98a72d585e3b115a55b6bdb2e893d3f49d9342f1",
  "dataspace": 11,
  "issued_ms": 1762723200000,
  "activation_epoch": 2050,
  "expiry_epoch": 2300,
  "entries": [
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.transfer",
        "method": "transfer",
        "asset": "CBDC#centralbank",
        "role": "Initiator"
      },
      "effect": {
        "Allow": {
          "max_amount": "1000000000",
          "window": "PerDay"
        }
      },
      "notes": "Wholesale transfer allowance (per UAID)"
    },
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.kit",
        "method": "withdraw"
      },
      "effect": {
        "Deny": {
          "reason": "Withdrawals disabled for this UAID"
        }
      }
    }
  ]
}
```

- 即使允許規則匹配 (`ManifestVerdict::Denied`)，拒絕規則也會獲勝，因此將所有顯式拒絕放在相關允許之後。
- 使用 `AllowanceWindow::PerSlot` 進行原子支付處理，使用 `PerMinute`/`PerDay` 進行滾動客戶限額。
- 每個 UAID/數據空間一個清單就足夠了；激活和到期強制策略輪換節奏。
- 一旦達到 `expiry_epoch`，通道運行時間現在會自動過期，
  因此運營團隊只需監控 `SpaceDirectoryEvent::ManifestExpired`，
  存檔 `nexus_space_directory_revision_total` 增量，並驗證 Torii 顯示
  `status = "Expired"`。 CLI `manifest expire` 命令仍然可用
  手動覆蓋或證據回填。

## 2. 銀行入職和白名單工作流程

|相|所有者 |行動|證據|
|--------|----------|---------|----------|
| 0. 攝入量 | CBDC 項目辦公室 |收集 KYC 檔案、技術 DS 清單、驗證者列表、UAID 映射。 |接收票、已簽署的 DS 艙單草稿。 |
| 1.治理審批|議會/合規|檢查攝入包，會簽 `cbdc.manifest.json`，批准 `AssetPermissionManifest`。 |簽署的治理分鐘，清單提交哈希。 |
| 2、能力發布| CBDC 通道操作 |通過 `norito::json::to_string_pretty` 對清單進行編碼，存儲在空間目錄下，通知操作員。 |清單 JSON + Norito `.to` 文件，BLAKE3 摘要。 |
| 3.白名單激活| CBDC 通道操作 |將 DSID 附加到 `composability_group.whitelist`，碰撞 `activation_epoch`，分發清單；如果需要，更新數據空間路由。 |艙單差異、`kagami config diff` 輸出、治理批准 ID。 |
| 4. 推出驗證 | QA 協會/運營 |運行集成測試、TEU 負載測試和可編程貨幣重放（見下文）。 | `cargo test` 日誌、TEU 儀表板、可編程貨幣夾具結果。 |
| 5.證據檔案|合規工作組 |捆綁清單、批准、功能摘要、測試輸出以及 `artifacts/nexus/cbdc_<stamp>/` 下的 Prometheus 抓取。 |證據壓縮包、校驗和文件、委員會簽字。 |

### 審計包助手使用空間目錄中的 `iroha app space-directory manifest audit-bundle` 幫助程序
在提交證據包之前對每個功能清單進行快照的劇本。
提供清單 JSON（或 `.to` 有效負載）和數據空間配置文件：

```bash
iroha app space-directory manifest audit-bundle \
  --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
  --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
  --out-dir artifacts/nexus/cbdc/2026-02-01T00-00Z/cbdc_wholesale \
  --notes "CBDC wholesale refresh"
```

該命令會發出規範的 JSON/Norito/hash 副本
`audit_bundle.json`，記錄UAID、數據空間id、激活/過期
紀元、清單哈希和配置文件審核掛鉤，同時強制執行所需的要求
`SpaceDirectoryEvent` 訂閱。將包裹放入證據內
目錄，以便審計人員和監管人員可以稍後重放確切的字節。

### 2.1 命令和驗證

1. **車道清單：** `cargo test -p integration_tests nexus::lane_registry -- --nocapture lane_manifest_registry_loads_fixture_manifests`。
2. **調度程序配額：** `cargo test -p integration_tests scheduler_teu -- queue_teu_backlog_matches_metering queue_routes_transactions_across_configured_lanes`。
3. **手動煙霧：** `irohad --sora --config configs/soranexus/nexus/config.toml --chain 0000…`，清單目錄指向 CBDC 文件，然後點擊 `/v1/sumeragi/status` 並驗證 CBDC 通道的 `lane_governance.manifest_ready=true`。
4. **白名單一致性測試：** `cargo test -p integration_tests nexus::cbdc_whitelist -- --nocapture` 練習 `integration_tests/tests/nexus/cbdc_whitelist.rs`，解析 `fixtures/space_directory/profile/cbdc_lane_profile.json` 和引用的能力清單，以確保每個白名單條目的 UAID、數據空間、激活紀元和許可列表與 `fixtures/space_directory/capability/` 下的 Norito 清單匹配。每當白名單或清單發生更改時，將測試日誌附加到 NX-6 證據包。

### 2.2 CLI 片段

- 通過 `cargo run -p iroha_cli -- manifest cbdc --uaid <hash> --dataspace 11 --template cbdc_wholesale` 生成 UAID + 清單框架。
- 使用 `iroha app space-directory manifest publish --manifest cbdc_wholesale.manifest.to`（或 `--manifest-json cbdc_wholesale.manifest.json`）將能力清單發佈到 Torii（空間目錄）；提交帳戶必須持有 CBDC 數據空間的 `CanPublishSpaceDirectoryManifest`。
- 如果操作台正在運行遠程自動化，則通過 HTTP 發布：

  ```bash
  curl -X POST https://torii.soranexus/v1/space-directory/manifests \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "i105...",
            "private_key": "ed25519:CiC7…",
            "manifest": '"'"'$(cat fixtures/space_directory/capability/cbdc_wholesale.manifest.json)'"'"',
            "reason": "CBDC onboarding wave 4"
          }'
  ```

  一旦發布事務排隊，Torii返回`202 Accepted`；一樣的
  CIDR/API 令牌門適用，並且鏈上權限要求與
  CLI 工作流程。
- 可以通過 POST 至 Torii 遠程發出緊急撤銷：

  ```bash
  curl -X POST https://torii.soranexus/v1/space-directory/manifests/revoke \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "i105...",
            "private_key": "ed25519:CiC7…",
            "uaid": "uaid:0f4d…ab11",
            "dataspace": 11,
            "revoked_epoch": 9216,
            "reason": "Fraud investigation #NX-16-R05"
          }'
  ```

  一旦撤銷交易排隊，Torii返回`202 Accepted`；一樣的
  CIDR/API 令牌門適用於其他應用程序端點，並且 `CanPublishSpaceDirectoryManifest`
  仍然需要上鍊。
- 輪換白名單成員資格：編輯 `cbdc.manifest.json`、更改 `activation_epoch`，並通過安全副本重新部署到所有驗證器； `LaneManifestRegistry` 按照配置的輪詢間隔進行熱重載。

## 3. 合規證據包

將工件存儲在 `artifacts/nexus/cbdc_rollouts/<YYYYMMDDThhmmZ>/` 下，並將摘要附加到治理票證中。

|文件|描述 |
|------|-------------|
| `cbdc.manifest.json` |帶有白名單差異的簽名通道清單（之前/之後）。 |
| `capability/<uaid>.manifest.json` 和 `.to` | Norito + 每個 UAID 的 JSON 功能清單。 |
| `compliance/kyc_<bank>.pdf` |面向監管機構的 KYC 認證。 |
| `metrics/nexus_scheduler_lane_teu_capacity.prom` | Prometheus 刮擦驗證 TEU 淨空。 |
| `tests/cargo_test_nexus_lane_registry.log` |從清單測試運行記錄。 |
| `tests/cargo_test_scheduler_teu.log` |記錄證明 TEU 路由通過。 |
| `programmable_money/axt_replay.json` |重放演示可編程貨幣互操作性的記錄（參見第 4 節）。 |
| `approvals/governance_minutes.md` |簽署的批准分鐘引用清單哈希 + 激活紀元。 |**驗證腳本：**運行 `ci/check_cbdc_rollout.sh` 以斷言證據包完整，然後再將其附加到治理票證。幫助程序掃描每個 `cbdc.manifest.json` 的 `artifacts/nexus/cbdc_rollouts/`（或 `CBDC_ROLLOUT_BUNDLE=<path>`），解析清單/可組合性組，驗證每個功能清單是否具有匹配的 `.to` 文件，檢查 `kyc_*.pdf` 證明，確認 TEU 指標抓取加`cargo test` 記錄、驗證可編程貨幣重放 JSON，並確保批准會議記錄引用激活紀元和清單哈希。最新版本還強制執行 NX-6 中提出的安全規範：法定人數不能超過聲明的驗證器集，受保護的命名空間必須是非空字符串，並且功能清單必須聲明單調遞增的 `activation_epoch`/`expiry_epoch` 對以及格式良好的 `Allow`/`Deny` 效果。 `fixtures/nexus/cbdc_rollouts/` 下的示例證據包由集成測試 `integration_tests/tests/nexus/cbdc_rollout_bundle.rs` 執行，使驗證器連接到 CI。

## 4. 可編程貨幣互操作性

一旦銀行（數據空間 11）和零售 dApp（數據空間 12）都被列入同一 `ComposabilityGroupId` 中的白名單，可編程資金流將遵循 `docs/source/nexus.md` §8.6 中的 AXT 模式：

1. Retail dApp 請求綁定到其 UAID + AXT 摘要的資產句柄。 CBDC 通道通過 `AssetPermissionManifest::evaluate` 驗證句柄（拒絕勝利，強制執行津貼）。
2. 兩個 DS 聲明相同的可組合性組，因此路由將它們折疊到 CBDC 通道中以實現原子包含（相互列入白名單時，`LaneRoutingPolicy` 使用 `group_id`）。
3. 在執行過程中，CBDC DS 在其電路內部強制執行 AML/KYC 證明（`nexus.md` 中的 `use_asset_handle` 偽代碼），而 dApp DS 僅在 CBDC 片段成功後更新本地業務狀態。
4. 證明材料（FASTPQ + DA 承諾）僅限於 CBDC 通道；合併賬本條目保持全局狀態的確定性，而不會洩漏私人數據。

可編程貨幣重放檔案應包括：

- AXT 描述符 + 處理請求/響應。
- Norito 編碼的交易信封。
- 結果收據（快樂路徑、拒絕路徑）。
- `telemetry::fastpq.execution_mode`、`nexus_scheduler_lane_teu_slot_committed` 和 `lane_commitments` 的遙測片段。

## 5. 可觀察性和運行手冊

- **指標：** 監控 `nexus_scheduler_lane_teu_capacity`、`nexus_scheduler_lane_teu_slot_committed`、`nexus_scheduler_lane_teu_deferral_total{reason}`、`governance_manifest_admission_total` 和 `lane_governance_sealed_total`（請參閱 `docs/source/telemetry.md`）。
- **儀表板：** 使用 CBDC 通道行擴展 `docs/source/grafana_scheduler_teu.json`；添加白名單流失面板（註釋每個激活時期）和功能到期管道。
- **警報：** 當 `rate(nexus_scheduler_lane_teu_deferral_total{reason="cap_exceeded"}[5m]) > 0` 持續 15 分鐘或當 `lane_governance.manifest_ready=false` 持續超過一個輪詢間隔時觸發。
- **Runbook 指針：** 鏈接到 `docs/source/governance_api.md` 中的受保護命名空間指南和 `docs/source/nexus.md` 中的可編程貨幣故障排除。

## 6. 驗收清單- [ ] `nexus.lane_catalog` 中聲明的 CBDC 通道，其 TEU 元數據與 TEU 測試匹配。
- [ ] 已簽名的 `cbdc.manifest.json` 存在於清單目錄中，通過 `cargo test -p integration_tests nexus::lane_registry` 進行驗證。
- [ ]為每個UAID發布並存儲在空間目錄中的能力清單；通過單元測試驗證拒絕/允許優先級 (`crates/iroha_data_model/src/nexus/manifest.rs`)。
- [ ] 白名單激活記錄有治理批准 ID、`activation_epoch` 和 Prometheus 證據。
- [ ] 可編程貨幣重放已存檔，演示了處理髮行、拒絕和允許流程。
- [ ] 證據包隨加密摘要一起上傳，一旦 NX-6 從🈯畢業到🈺，就從治理票證和 `status.md` 鏈接。

遵循本手冊可以滿足 NX-6 的文檔交付，並通過為 CBDC 通道配置、白名單加入和可編程貨幣互操作性提供確定性模板來解鎖未來路線圖項目 (NX-12/NX-15)。