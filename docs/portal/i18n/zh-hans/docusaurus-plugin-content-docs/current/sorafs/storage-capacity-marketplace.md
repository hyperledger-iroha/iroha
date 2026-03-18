---
id: storage-capacity-marketplace
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Storage Capacity Marketplace
sidebar_label: Capacity Marketplace
description: SF-2c plan for the capacity marketplace, replication orders, telemetry, and governance hooks.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
:::

# SoraFS 存储容量市场（SF-2c 草案）

SF-2c 路线图项目引入了一个受监管的市场，其中存储
提供商声明承诺容量、接收复制订单并赚取费用
与交付的可用性成正比。本文档涵盖了可交付成果
首次发布所需的内容并将其分解为可操作的轨道。

## 目标

- Express 提供商容量承诺（总字节数、每通道限制、到期时间）
  以可供治理、SoraNet 传输和 Torii 使用的可验证形式。
- 根据声明的容量、股权和数量在提供商之间分配 pin
  政策约束，同时保持确定性行为。
- 计量存储交付（复制成功、正常运行时间、完整性证明）以及
  导出遥测数据以进行费用分配。
- 提供撤销和争议流程，以便不诚实的提供商可以
  受到处罚或除名。

## 领域概念

|概念 |描述 |初始交付 |
|--------|-------------|---------------------|
| `CapacityDeclarationV1` | Norito 有效负载描述提供商 ID、分块配置文件支持、承诺 GiB、通道特定限制、定价提示、质押承诺和到期日。 | `sorafs_manifest::capacity` 中的架构 + 验证器。 |
| `ReplicationOrder` |治理发布的指令将清单 CID 分配给一个或多个提供商，包括冗余级别和 SLA 指标。 | Norito 架构与 Torii + 智能合约 API 共享。 |
| `CapacityLedger` |链上/链下注册表跟踪活动容量声明、复制订单、性能指标和费用应计。 |具有确定性快照的智能合约模块或链下服务存根。 |
| `MarketplacePolicy` |定义最低股权、审计要求和惩罚曲线的治理政策。 | `sorafs_manifest` + 治理文档中的配置结构。 |

### 已实施的架构（状态）

## 工作分解

### 1. 架构和注册表层

|任务|所有者 |笔记|
|------|----------|--------|
|定义 `CapacityDeclarationV1`、`ReplicationOrderV1`、`CapacityTelemetryV1`。 |存储团队/治理|使用 Norito；包括语义版本控制和功能参考。 |
|在 `sorafs_manifest` 中实现解析器 + 验证器模块。 |存储团队|强制执行单调 ID、容量限制、权益要求。 |
|每个配置文件使用 `min_capacity_gib` 扩展分块器注册表元数据。 |工具工作组 |帮助客户强制执行每个配置文件的最低硬件要求。 |
| `MarketplacePolicy` 草案文件包含入场护栏和处罚表。 |治理委员会|在文档中与默认策略一起发布。 |

#### 架构定义（已实现）

- `CapacityDeclarationV1` 捕获每个提供商签署的容量承诺，包括规范分块器句柄、功能参考、可选通道上限、定价提示、有效性窗口和元数据。验证确保非零权益、规范句柄、重复数据删除别名、声明总数内的每通道上限以及单调 GiB 会计。【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` 将清单绑定到具有冗余目标、SLA 阈值和每个分配保证的治理发布的分配；验证器在 Torii 或注册中心接收订单之前强制执行规范的分块句柄、唯一提供者和截止日期约束。【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` 表示提供费用分配的纪元快照（声明的与使用的 GiB、复制计数器、正常运行时间/PoR 百分比）。边界检查将利用率保持在声明范围内，百分比保持在 0 – 100% 范围内。【crates/sorafs_manifest/src/capacity.rs:476】
- 共享助手（`CapacityMetadataEntry`、`PricingScheduleV1`、通道/分配/SLA 验证器）提供 CI 和下游工具可以重用的确定性密钥验证和错误报告。【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` 现在通过 `/v1/sorafs/capacity/state` 呈现链上快照，结合确定性 Norito JSON 后面的提供商声明和费用分类帐条目。【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- 验证覆盖范围执行规范句柄执行、重复检测、每通道边界、复制分配保护和遥测范围检查，以便回归立即在 CI 中显现。【crates/sorafs_manifest/src/capacity.rs:792】
- 操作员工具：`sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` 将人类可读的规范转换为规范的 Norito 有效负载、base64 blob 和 JSON 摘要，以便操作员可以使用本地暂存 `/v1/sorafs/capacity/declare`、`/v1/sorafs/capacity/telemetry` 和复制顺序固定装置验证。【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】参考夹具位于 `fixtures/sorafs_manifest/replication_order/`（`order_v1.json`、`order_v1.to`）中，并通过 `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order` 生成。

### 2. 控制平面集成

|任务|所有者 |笔记|
|------|----------|--------|
|添加具有 Norito JSON 负载的 `/v1/sorafs/capacity/declare`、`/v1/sorafs/capacity/telemetry`、`/v1/sorafs/capacity/orders` Torii 处理程序。 | Torii 团队 |镜像验证器逻辑；重用 Norito JSON 帮助程序。 |
|将 `CapacityDeclarationV1` 快照传播到 Orchestrator 记分板元数据和网关获取计划中。 |工具工作组/协调器团队|使用容量参考扩展 `provider_metadata`，以便多源评分遵守通道限制。 |
|将复制订单输入编排器/网关客户端以驱动分配和故障转移提示。 |网络 TL/网关团队 |记分板构建器使用治理签名的复制订单。 |
| CLI 工具：使用 `capacity declare`、`capacity telemetry`、`capacity orders import` 扩展 `sorafs_cli`。 |工具工作组 |提供确定性 JSON + 记分板输出。 |

### 3. 市场政策与治理

|任务|所有者 |笔记|
|------|----------|--------|
|批准 `MarketplacePolicy`（最低股权、处罚乘数、审计节奏）。 |治理委员会|在文档中发布，捕获修订历史记录。 |
|添加治理挂钩，以便议会可以批准、更新和撤销声明。 |治理委员会/智能合约团队|使用 Norito 事件 + 清单摄取。 |
|实施与遥测 SLA 违规行为相关的处罚计划（费用减少、保证金削减）。 |治理委员会/财政部|与 `DealEngine` 结算输出对齐。 |
|记录争议流程和升级矩阵。 |文档/治理|指向争议 Runbook + CLI 帮助程序的链接。 |

### 4. 计量和费用分配

|任务|所有者 |笔记|
|------|----------|--------|
|展开 Torii 计量摄取以接受 `CapacityTelemetryV1`。 | Torii 团队 |验证 GiB 小时、PoR 成功、正常运行时间。 |
|更新 `sorafs_node` 计量管道以报告每订单利用率 + SLA 统计数据。 |存储团队|与复制顺序和分块器句柄对齐。 |
|结算管道：将遥测+复制数据转换为以异或计价的支出，生成可供治理的摘要，并记录账本状态。 |财务/存储团队|电汇至交易引擎/金库出口。 |
|导出仪表板/警报以测量运行状况（摄取积压、过时的遥测）。 |可观察性|扩展 SF-6/SF-7 引用的 Grafana 包。 |

- Torii 现在公开 `/v1/sorafs/capacity/telemetry` 和 `/v1/sorafs/capacity/state` (JSON + Norito)，以便操作员可以提交纪元遥测快照，检查员可以检索规范账本以进行审计或证据包装。【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- `PinProviderRegistry` 集成确保可通过同一端点访问复制订单； CLI 助手 (`sorafs_cli capacity telemetry --from-file telemetry.json`) 现在通过确定性哈希和别名解析来验证/发布来自自动化运行的遥测数据。
- 计量快照生成固定到 `metering` 快照的 `CapacityTelemetrySnapshot` 条目，Prometheus 导出为 `docs/source/grafana_sorafs_metering.json` 处的准备导入 Grafana 板提供数据，以便计费团队可以监控预计的 GiB·小时累积情况nano-SORA费用，以及实时SLA合规性。【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- 启用计量平滑后，快照包括 `smoothed_gib_hours` 和 `smoothed_por_success_bps`，因此运营商可以将 EMA 趋势值与治理用于支付的原始计数器进行比较。【crates/sorafs_node/src/metering.rs:401】

### 5. 争议和撤销处理

|任务|所有者 |笔记|
|------|----------|--------|
|定义 `CapacityDisputeV1` 有效负载（投诉人、证据、目标提供商）。 |治理委员会| Norito 架构 + 验证器。 |
| CLI 支持提交争议并作出回应（带有证据附件）。 |工具工作组 |确保证据包的确定性散列。 |
|添加对重复违反 SLA 的自动检查（自动升级为争议）。 |可观察性|警报阈值和治理挂钩。 |
|文档撤销手册（宽限期、撤出固定数据）。 |文档/存储团队 |政策文档和操作手册的链接。 |

## 测试和 CI 要求- 所有新模式验证器的单元测试 (`sorafs_manifest`)。
- 模拟集成测试：声明→复制顺序→计量→支付。
- CI 工作流程，用于重新生成样本容量声明/遥测并确保签名保持同步（扩展 `ci/check_sorafs_fixtures.sh`）。
- 注册表 API 的负载测试（模拟 10k 个提供商、100k 个订单）。

## 遥测和仪表板

- 仪表板面板：
  - 每个提供商声明的容量与使用的容量。
  - 复制订单积压和平均分配延迟。
  - SLA 合规性（正常运行时间百分比、PoR 成功率）。
  - 每个周期的费用累积和处罚。
- 警报：
  - 提供商低于最低承诺容量。
  - 复制顺序卡住 > SLA。
  - 计量管道故障。

## 文档交付成果

- 操作员指南，用于声明容量、更新承诺和监控利用率。
- 审批申报、发布命令、处理纠纷的治理指南。
- 容量端点和复制顺序格式的 API 参考。
- 开发人员的市场常见问题解答。

## GA 准备清单

路线图项目**SF-2c**根据会计领域的具体证据进行生产推广，
争议处理和入职。使用以下工件来保持验收标准
与实施同步。

### 每晚记账和异或对账
- 导出同一窗口的容量状态快照和异或账本导出，然后运行：
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  帮助程序因缺少/多付的和解或罚款而退出非零，并发出 Prometheus
  文本文件摘要。
- 警报 `SoraFSCapacityReconciliationMismatch`（在 `dashboards/alerts/sorafs_capacity_rules.yml` 中）
  每当调节指标报告差距时就会触发；仪表板位于下面
  `dashboards/grafana/sorafs_capacity_penalties.json`。
- 将 JSON 摘要和哈希值存档在 `docs/examples/sorafs_capacity_marketplace_validation/` 下
  与治理包一起。

### 争议和削减证据
- 通过 `sorafs_manifest_stub capacity dispute` 提出争议（测试：
  `cargo test -p sorafs_car --test capacity_cli`），因此有效负载保持规范。
- 运行 `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` 和惩罚
  套件（`record_capacity_telemetry_penalises_persistent_under_delivery`）来证明争议和
  斜杠确定性地重播。
- 遵循 `docs/source/sorafs/dispute_revocation_runbook.md` 进行证据捕获和升级；
  将罢工批准链接回验证报告。

### 提供商入职和退出冒烟测试
- 使用 `sorafs_manifest_stub capacity ...` 重新生成声明/遥测工件并重播
  提交前进行 CLI 测试 (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`)。
- 通过 Torii (`/v1/sorafs/capacity/declare`) 提交，然后捕获 `/v1/sorafs/capacity/state` 加
  Grafana 屏幕截图。按照 `docs/source/sorafs/capacity_onboarding_runbook.md` 中的退出流程进行操作。
- 将签名的工件和对账输出存档在内部
  `docs/examples/sorafs_capacity_marketplace_validation/`。

## 依赖关系和排序

1. 完成 SF-2b（准入政策）——市场依赖于经过审查的提供商。
2. 在Torii集成之前实现模式+注册表层（本文档）。
3. 在启用支付之前完成计量管道。
4. 最后一步：一旦计量数据在暂存阶段得到验证，即可启用治理控制的费用分配。

应在路线图中参考本文档来跟踪进展情况。一旦每个主要部分（架构、控制平面、集成、计量、争议处理）达到功能完成状态，就更新路线图。