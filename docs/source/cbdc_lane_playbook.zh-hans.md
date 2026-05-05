---
lang: zh-hans
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

# CBDC 私人通道剧本 (NX-6)

> **路线图链接：** NX-6（CBDC 专用通道模板和白名单流程）和 NX-14（Nexus 运行手册）。  
> **所有者：** 金融服务工作组、Nexus 核心工作组、合规工作组。  
> **状态：** 起草 — `crates/iroha_data_model::nexus`、`crates/iroha_core::governance::manifest` 和 `integration_tests/tests/nexus/lane_registry.rs` 中存在实现挂钩，但缺少 CBDC 特定的清单、白名单和操作员运行手册。本手册记录了参考配置和入门工作流程，以便 CBDC 部署可以确定地进行。

## 范围和角色

- **中央银行通道（“CBDC 通道”）：** 许可的验证器、托管结算缓冲区和可编程货币政策。作为受限数据空间+通道对运行，具​​有自己的治理清单。
- **批发/零售银行数据空间：** 持有 UAID、接收能力清单的参与者 DS，并且可能会被列入具有 CBDC 通道的原子 AXT 白名单。
- **可编程货币 dApp：** 一旦列入白名单，通过 `ComposabilityGroup` 路由消耗 CBDC 流的外部 DS。
- **治理与合规性：** 议会（或同等模块）批准车道清单、能力清单和白名单更改；合规性将证据包与 Norito 清单一起存储。

**依赖关系**

1. 巷目录+数据空间目录接线（`docs/source/nexus_lanes.md`、`defaults/nexus/config.toml`）。
2. 通道清单强制执行（`crates/iroha_core/src/governance/manifest.rs`，`crates/iroha_core/src/queue.rs` 中的队列门控）。
3. 能力清单 + UAID (`crates/iroha_data_model/src/nexus/manifest.rs`)。
4.调度程序TEU配额+指标（`integration_tests/tests/scheduler_teu.rs`、`docs/source/telemetry.md`）。

## 1. 参考车道布局

### 1.1 车道目录和数据空间条目

将专用条目添加到 `[[nexus.lane_catalog]]` 和 `[[nexus.dataspace_catalog]]`。下面的示例使用 CBDC 通道扩展 `defaults/nexus/config.toml`，该通道为每个时隙预留 1500 个 TEU，并将饥饿限制为 6 个时隙，并为批发银行和零售钱包添加匹配的数据空间别名。

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

**注释**

- `metadata.scheduler.teu_capacity` 和 `metadata.scheduler.starvation_bound_slots` 为 `integration_tests/tests/scheduler_teu.rs` 所使用的 TEU 规格提供数据。操作员必须使其与验收结果保持同步，以便 `nexus_scheduler_lane_teu_capacity` 与模板匹配。
- 上述每个数据空间别名必须出现在治理清单和功能清单中（见下文），以便准入自动拒绝漂移。

### 1.2 车道清单框架

Lane 清单位于通过 `nexus.registry.manifest_directory` 配置的目录下（请参阅 `crates/iroha_config/src/parameters/actual.rs`）。文件名应与通道别名匹配 (`cbdc.manifest.json`)。该架构反映了 `integration_tests/tests/nexus/lane_registry.rs` 中的治理清单测试。

```json
{
  "lane": "cbdc",
  "dataspace": "cbdc.core",
  "version": 1,
  "governance": "central_bank_multisig",
  "validators": [
    { "validator": "<i105-account-id>", "peer_id": "<peer-id>" },
    { "validator": "<i105-account-id>", "peer_id": "<peer-id>" },
    { "validator": "<i105-account-id>", "peer_id": "<peer-id>" },
    { "validator": "<i105-account-id>", "peer_id": "<peer-id>" }
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

关键要求：

- Validators **must** be declared as explicit bindings with a canonical I105
  authority account plus a concrete `peer_id`. Legacy string-only validator
  arrays are rejected.
- Each manifest `peer_id` must resolve to a registered runtime peer with a live
  consensus key that is present in the current commit topology; Torii routes
  only to those authoritative peer bindings and fails closed when the runtime
  truth disagrees with the manifest.
- Validator accounts should remain stable governance identities even if the
  underlying host or peer keys rotate; update the manifest `peer_id` binding
  when the serving peer changes. Set `quorum` to the multisig threshold (≥2).
- 受保护的命名空间由 `Queue::push` 强制执行（请参阅 `crates/iroha_core/src/queue.rs`），因此所有 CBDC 合约必须指定 `gov_namespace` + `gov_contract_id`。
- `composability_group` 字段遵循 `docs/source/nexus.md` §8.6 中描述的模式；所有者（CBDC 通道）提供白名单和配额。白名单 DS 清单仅指定 `group_id_hex` + `activation_epoch`。
- 复制清单后，运行 `cargo test -p integration_tests nexus::lane_registry -- --nocapture` 以确认 `LaneManifestRegistry::from_config` 加载它。

### 1.3 能力清单（UAID策略）

能力清单（`crates/iroha_data_model/src/nexus/manifest.rs` 中的 `AssetPermissionManifest`）将 `UniversalAccountId` 绑定到确定性限额。通过 Space Directory 发布它们，以便银行和 dApp 可以获取签名的策略。

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

- 即使允许规则匹配 (`ManifestVerdict::Denied`)，拒绝规则也会获胜，因此将所有显式拒绝放在相关允许之后。
- 使用 `AllowanceWindow::PerSlot` 进行原子支付处理，使用 `PerMinute`/`PerDay` 进行滚动客户限额。
- 每个 UAID/数据空间一个清单就足够了；激活和到期强制策略轮换节奏。
- 一旦达到 `expiry_epoch`，通道运行时间现在会自动过期，
  因此运营团队只需监控 `SpaceDirectoryEvent::ManifestExpired`，
  存档 `nexus_space_directory_revision_total` 增量，并验证 Torii 显示
  `status = "Expired"`。 CLI `manifest expire` 命令仍然可用
  手动覆盖或证据回填。

## 2. 银行入职和白名单工作流程

|相|所有者 |行动|证据|
|--------|----------|---------|----------|
| 0. 摄入量 | CBDC 项目办公室 |收集 KYC 档案、技术 DS 清单、验证者列表、UAID 映射。 |接收票、已签署的 DS 舱单草稿。 |
| 1.治理审批|议会/合规|检查摄入包，会签 `cbdc.manifest.json`，批准 `AssetPermissionManifest`。 |签署的治理分钟，清单提交哈希。 |
| 2、能力发布| CBDC 通道操作 |通过 `norito::json::to_string_pretty` 对清单进行编码，存储在空间目录下，通知操作员。 |清单 JSON + Norito `.to` 文件，BLAKE3 摘要。 |
| 3.白名单激活| CBDC 通道操作 |将 DSID 附加到 `composability_group.whitelist`，碰撞 `activation_epoch`，分发清单；如果需要，更新数据空间路由。 |舱单差异、`kagami config diff` 输出、治理批准 ID。 |
| 4. 推出验证 | QA 协会/运营 |运行集成测试、TEU 负载测试和可编程货币重放（见下文）。 | `cargo test` 日志、TEU 仪表板、可编程货币夹具结果。 |
| 5.证据档案|合规工作组 |捆绑清单、批准、功能摘要、测试输出以及 `artifacts/nexus/cbdc_<stamp>/` 下的 Prometheus 抓取。 |证据压缩包、校验和文件、委员会签字。 |

### 审计包助手使用空间目录中的 `iroha app space-directory manifest audit-bundle` 帮助程序
在提交证据包之前对每个功能清单进行快照的剧本。
提供清单 JSON（或 `.to` 有效负载）和数据空间配置文件：

```bash
iroha app space-directory manifest audit-bundle \
  --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
  --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
  --out-dir artifacts/nexus/cbdc/2026-02-01T00-00Z/cbdc_wholesale \
  --notes "CBDC wholesale refresh"
```

该命令会发出规范的 JSON/Norito/hash 副本
`audit_bundle.json`，记录UAID、数据空间id、激活/过期
纪元、清单哈希和配置文件审核挂钩，同时强制执行所需的要求
`SpaceDirectoryEvent` 订阅。将包裹放入证据内
目录，以便审计人员和监管人员可以稍后重放确切的字节。

### 2.1 命令和验证

1. **车道清单：** `cargo test -p integration_tests nexus::lane_registry -- --nocapture lane_manifest_registry_loads_fixture_manifests`。
2. **调度程序配额：** `cargo test -p integration_tests scheduler_teu -- queue_teu_backlog_matches_metering queue_routes_transactions_across_configured_lanes`。
3. **手动烟雾：** `irohad --sora --config configs/soranexus/nexus/config.toml --chain 0000…`，清单目录指向 CBDC 文件，然后点击 `/v1/sumeragi/status` 并验证 CBDC 通道的 `lane_governance.manifest_ready=true`。
4. **白名单一致性测试：** `cargo test -p integration_tests nexus::cbdc_whitelist -- --nocapture` 练习 `integration_tests/tests/nexus/cbdc_whitelist.rs`，解析 `fixtures/space_directory/profile/cbdc_lane_profile.json` 和引用的能力清单，以确保每个白名单条目的 UAID、数据空间、激活纪元和许可列表与 `fixtures/space_directory/capability/` 下的 Norito 清单匹配。每当白名单或清单发生更改时，将测试日志附加到 NX-6 证据包。

### 2.2 CLI 片段

- 通过 `cargo run -p iroha_cli -- manifest cbdc --uaid <hash> --dataspace 11 --template cbdc_wholesale` 生成 UAID + 清单骨架。
- 使用 `iroha app space-directory manifest publish --manifest cbdc_wholesale.manifest.to`（或 `--manifest-json cbdc_wholesale.manifest.json`）将能力清单发布到 Torii（空间目录）；提交帐户必须持有 CBDC 数据空间的 `CanPublishSpaceDirectoryManifest`。
- 如果操作台正在运行远程自动化，则通过 HTTP 发布：

  ```bash
  curl -X POST https://torii.soranexus/v1/space-directory/manifests \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "<i105-account-id>",
            "private_key": "ed25519:CiC7…",
            "manifest": '"'"'$(cat fixtures/space_directory/capability/cbdc_wholesale.manifest.json)'"'"',
            "reason": "CBDC onboarding wave 4"
          }'
  ```

  一旦发布事务排队，Torii返回`202 Accepted`；一样的
  CIDR/API 令牌门适用，并且链上权限要求与
  CLI 工作流程。
- 可以通过 POST 至 Torii 远程发出紧急撤销：

  ```bash
  curl -X POST https://torii.soranexus/v1/space-directory/manifests/revoke \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "<i105-account-id>",
            "private_key": "ed25519:CiC7…",
            "uaid": "uaid:0f4d…ab11",
            "dataspace": 11,
            "revoked_epoch": 9216,
            "reason": "Fraud investigation #NX-16-R05"
          }'
  ```

  一旦撤销交易排队，Torii返回`202 Accepted`；一样的
  CIDR/API 令牌门适用于其他应用程序端点，并且 `CanPublishSpaceDirectoryManifest`
  仍然需要上链。
- 轮换白名单成员资格：编辑 `cbdc.manifest.json`、更改 `activation_epoch`，并通过安全副本重新部署到所有验证器； `LaneManifestRegistry` 按照配置的轮询间隔进行热重载。

## 3. 合规证据包

将工件存储在 `artifacts/nexus/cbdc_rollouts/<YYYYMMDDThhmmZ>/` 下，并将摘要附加到治理票证中。

|文件 |描述 |
|------|-------------|
| `cbdc.manifest.json` |带有白名单差异的签名通道清单（之前/之后）。 |
| `capability/<uaid>.manifest.json` 和 `.to` | Norito + 每个 UAID 的 JSON 功能清单。 |
| `compliance/kyc_<bank>.pdf` |面向监管机构的 KYC 认证。 |
| `metrics/nexus_scheduler_lane_teu_capacity.prom` | Prometheus 刮擦验证 TEU 净空。 |
| `tests/cargo_test_nexus_lane_registry.log` |从清单测试运行记录。 |
| `tests/cargo_test_scheduler_teu.log` |记录证明 TEU 路由通过。 |
| `programmable_money/axt_replay.json` |重放演示可编程货币互操作性的记录（参见第 4 节）。 |
| `approvals/governance_minutes.md` |签署的批准分钟引用清单哈希 + 激活纪元。 |**验证脚本：**运行 `ci/check_cbdc_rollout.sh` 以断言证据包完整，然后再将其附加到治理票证。帮助程序扫描每个 `cbdc.manifest.json` 的 `artifacts/nexus/cbdc_rollouts/`（或 `CBDC_ROLLOUT_BUNDLE=<path>`），解析清单/可组合性组，验证每个功能清单是否具有匹配的 `.to` 文件，检查 `kyc_*.pdf` 证明，确认 TEU 指标抓取加`cargo test` 记录、验证可编程货币重放 JSON，并确保批准会议记录引用激活纪元和清单哈希。最新版本还强制执行 NX-6 中提出的安全规范：法定人数不能超过声明的验证器集，受保护的命名空间必须是非空字符串，并且功能清单必须声明单调递增的 `activation_epoch`/`expiry_epoch` 对以及格式良好的 `Allow`/`Deny` 效果。 `fixtures/nexus/cbdc_rollouts/` 下的示例证据包由集成测试 `integration_tests/tests/nexus/cbdc_rollout_bundle.rs` 执行，使验证器连接到 CI。

## 4. 可编程货币互操作性

一旦银行（数据空间 11）和零售 dApp（数据空间 12）都被列入同一 `ComposabilityGroupId` 中的白名单，可编程资金流将遵循 `docs/source/nexus.md` §8.6 中的 AXT 模式：

1. Retail dApp 请求绑定到其 UAID + AXT 摘要的资产句柄。 CBDC 通道通过 `AssetPermissionManifest::evaluate` 验证句柄（拒绝胜利，强制执行津贴）。
2. 两个 DS 声明相同的可组合性组，因此路由将它们折叠到 CBDC 通道中以实现原子包含（相互列入白名单时，`LaneRoutingPolicy` 使用 `group_id`）。
3. 在执行过程中，CBDC DS 在其电路内部强制执行 AML/KYC 证明（`nexus.md` 中的 `use_asset_handle` 伪代码），而 dApp DS 仅在 CBDC 片段成功后更新本地业务状态。
4. 证明材料（FASTPQ + DA 承诺）仅限于 CBDC 通道；合并账本条目保持全局状态的确定性，而不会泄漏私人数据。

可编程货币重放档案应包括：

- AXT 描述符 + 处理请求/响应。
- Norito 编码的交易信封。
- 结果收据（快乐路径、拒绝路径）。
- `telemetry::fastpq.execution_mode`、`nexus_scheduler_lane_teu_slot_committed` 和 `lane_commitments` 的遥测片段。

## 5. 可观察性和运行手册

- **指标：** 监控 `nexus_scheduler_lane_teu_capacity`、`nexus_scheduler_lane_teu_slot_committed`、`nexus_scheduler_lane_teu_deferral_total{reason}`、`governance_manifest_admission_total` 和 `lane_governance_sealed_total`（请参阅 `docs/source/telemetry.md`）。
- **仪表板：** 使用 CBDC 通道行扩展 `docs/source/grafana_scheduler_teu.json`；添加白名单流失面板（注释每个激活时期）和功能到期管道。
- **警报：** 当 `rate(nexus_scheduler_lane_teu_deferral_total{reason="cap_exceeded"}[5m]) > 0` 持续 15 分钟或当 `lane_governance.manifest_ready=false` 持续超过一个轮询间隔时触发。
- **Runbook 指针：** 链接到 `docs/source/governance_api.md` 中的受保护命名空间指南和 `docs/source/nexus.md` 中的可编程货币故障排除。

## 6. 验收清单- [ ] `nexus.lane_catalog` 中声明的 CBDC 通道，其 TEU 元数据与 TEU 测试匹配。
- [ ] 已签名的 `cbdc.manifest.json` 存在于清单目录中，通过 `cargo test -p integration_tests nexus::lane_registry` 进行验证。
- [ ]为每个UAID发布并存储在空间目录中的能力清单；通过单元测试验证拒绝/允许优先级 (`crates/iroha_data_model/src/nexus/manifest.rs`)。
- [ ] 白名单激活记录有治理批准 ID、`activation_epoch` 和 Prometheus 证据。
- [ ] 可编程货币重放已存档，演示了处理发行、拒绝和允许流程。
- [ ] 证据包随加密摘要一起上传，一旦 NX-6 从🈯毕业到🈺，就从治理票证和 `status.md` 链接。

遵循本手册可以满足 NX-6 的文档交付，并通过为 CBDC 通道配置、白名单加入和可编程货币互操作性提供确定性模板来解锁未来路线图项目 (NX-12/NX-15)。
