---
lang: zh-hans
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

# SoraFS 容量市场验证清单

**审核时间：** 2026-03-18 → 2026-03-24  
**计划所有者：** 存储团队 (`@storage-wg`)、治理委员会 (`@council`)、财政协会 (`@treasury`)  
**范围：** SF-2c GA 所需的提供商入职管道、争议裁决流程和财务协调流程。

在为外部运营商启用市场之前，必须检查以下清单。每行都链接到审核员可以重播的确定性证据（测试、固定装置或文档）。

## 验收清单

### 提供商入职

|检查 |验证 |证据|
|--------|------------|----------|
|注册管理机构接受规范的容量声明 |集成测试通过应用程序 API 练习 `/v1/sorafs/capacity/declare`，验证签名处理、元数据捕获以及移交给节点注册表。 | `crates/iroha_torii/src/routing.rs:7654` |
|智能合约拒绝不匹配的有效负载 |单元测试确保提供者 ID 和提交的 GiB 字段在持久化之前与签名的声明匹配。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI 发出规范的入门工件 | CLI 工具写入确定性 Norito/JSON/Base64 输出并验证往返，以便操作员可以离线进行声明。 | `crates/sorafs_car/tests/capacity_cli.rs:17` |
|操作员指南捕获准入工作流程和治理护栏 |文档列举了理事会的声明模式、默认策略和审查步骤。 | `../storage-capacity-marketplace.md` |

### 争议解决

|检查 |验证 |证据|
|--------|------------|----------|
|争议记录与规范有效负载摘要保持一致 |单元测试注册争议，解码存储的有效负载，并断言待处理状态以保证账本的确定性。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI 争议生成器匹配规范模式 | CLI 测试涵盖 Base64/Norito 输出和 `CapacityDisputeV1` 的 JSON 摘要，确保证据包确定性地散列。 | `crates/sorafs_car/tests/capacity_cli.rs:455` |
|重播测试证明争议/处罚决定论 |重播两次的失败证明遥测会产生相同的账本、信用和争议快照，因此斜线在同行之间是确定性的。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook 记录升级和撤销流程 |操作指南涵盖了理事会工作流程、证据要求和回滚程序。 | `../dispute-revocation-runbook.md` |

### 国库对账

|检查 |验证 |证据|
|--------|------------|----------|
|账本应计与 30 天浸泡预测相匹配 |浸泡测试涵盖 5 个提供商的 30 个结算窗口，将账本条目与预期支付参考进行比较。 | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
|每晚记录账本出口对账 | `capacity_reconcile.py` 将费用账本预期与执行的 XOR 转账导出进行比较，发出 Prometheus 指标，并通过 Alertmanager 获得财务部批准。 | `scripts/telemetry/capacity_reconcile.py:1`，`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`，`dashboards/alerts/sorafs_capacity_rules.yml:100` |
|计费仪表板表面处罚和应计遥测| Grafana 导入绘制 GiB·小时累计、执行计数器和保税抵押品以实现随叫随到的可见性。 | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
|已发布的报告存档浸泡方法和重放命令 |报告详细介绍了审计人员的浸泡范围、执行命令和可观察性挂钩。 | `./sf2c-capacity-soak.md` |

## 执行注意事项

在签核之前重新运行验证套件：

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

运营商应使用 `sorafs_manifest_stub capacity {declaration,dispute}` 重新生成加入/争议请求有效负载，并将生成的 JSON/Norito 字节与治理票证一起存档。

## 签核工件

|文物|路径|布莱克2b-256 |
|----------|------|-------------|
|提供商入职审批包 | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
|争议解决审批包| `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
|国库调节批准包| `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

将这些工件的签名副本与发布包一起存储，并将它们链接到治理变更记录中。

## 批准

- 存储团队负责人 — @storage-tl (2026-03-24)  
- 治理委员会秘书 — @council-sec (2026-03-24)  
- 资金运营主管 — @treasury-ops (2026-03-24)