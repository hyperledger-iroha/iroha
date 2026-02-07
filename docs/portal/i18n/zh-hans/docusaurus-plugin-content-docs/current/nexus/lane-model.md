---
id: nexus-lane-model
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/lane-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus lane model
description: Logical lane taxonomy, configuration geometry, and world-state merge rules for Sora Nexus.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Nexus 通道模型和 WSV 分区

> **状态：** NX-1 可交付成果 — 通道分类、配置几何结构和存储布局已准备好实施。  
> **所有者：** Nexus 核心工作组、治理工作组  
> **路线图参考：** `roadmap.md` 中的 NX-1

此门户页面反映了规范的 `docs/source/nexus_lanes.md` 简介，因此 Sora
Nexus 操作员、SDK 所有者和审阅者无需阅读车道指南即可
深入研究 mono-repo 树。目标架构保持世界状态
确定性，同时允许单独的数据空间（通道）公共运行或
具有隔离工作负载的私有验证器集。

## 概念

- **Lane:** Nexus 账本的逻辑分片，具有自己的验证器集和
  执行积压。由稳定的 `LaneId` 标识。
- **数据空间：** 治理桶分组一个或多个共享通道
  合规性、路由和结算政策。
- **车道清单：** 描述验证器、DA 的治理控制元数据
  策略、gas 代币、结算规则和路由权限。
- **全球承诺：** 由车道发出的总结新状态根的证明，
  结算数据和可选的跨车道传输。全球 NPoS 环
  订单承诺。

## 泳道分类

车道类型规范地描述了它们的可见性、治理面和
结算挂钩。配置几何 (`LaneConfig`) 捕获这些
属性，以便节点、SDK 和工具可以推断布局，而无需
定制逻辑。

|车道类型|能见度|验证者会员资格 | WSV 暴露 |违约治理|落户政策 |典型用途|
|------------|------------|----------------------|------------------------|--------------------|--------------------|-------------|
| `default_public` |公共|无需许可（全球股权）|完整状态副本 | SORA 议会 | `xor_global` |基线公共分类账 |
| `public_custom` |公共|无需许可或股权门禁 |完整状态副本 |股权加权模块 | `xor_lane_weighted` |高通量公共应用|
| `private_permissioned` |限制 |固定验证器集（政府批准）|承诺及证明|联邦理事会| `xor_hosted_custody` | CBDC、联盟工作负载 |
| `hybrid_confidential` |限制 |混合会员资格；包装 ZK 样张 |承诺+选择性披露|可编程货币模块| `xor_dual_fund` |保护隐私的可编程货币|

所有车道类型必须声明：

- 数据空间别名 — 绑定合规策略的人类可读分组。
- 治理句柄 — 通过 `Nexus.governance.modules` 解析的标识符。
- 结算句柄 — 结算路由器用于借方 XOR 的标识符
  缓冲区。
- 可选的遥测元数据（描述、联系人、业务领域）浮出水面
  通过 `/status` 和仪表板。

## 车道配置几何形状 (`LaneConfig`)

`LaneConfig` 是从经过验证的通道目录派生的运行时几何图形。它
**不**取代治理清单；相反，它提供了确定性
每个配置通道的存储标识符和遥测提示。

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- 无论何时配置，`LaneConfig::from_catalog` 都会重新计算几何形状
  已加载（`State::set_nexus`）。
- 别名被清理为小写字母；连续的非字母数字
  字符折叠成 `_`。如果别名产生一个空的 slug，我们就会回退
  至 `lane{id}`。
- `shard_id` 源自目录元数据密钥 `da_shard_id`（默认
  到 `lane_id`）并驱动持久分片游标日志以保持 DA 重播
  重新启动/重新分片时具有确定性。
- 密钥前缀确保 WSV 保持每通道密钥范围不相交，即使
  共享相同的后端。
- Kura 段名称在主机之间具有确定性；审核员可以交叉检查
  无需定制工具即可对目录和清单进行分段。
- 合并段（`lane_{id:03}_merge`）保存最新的合并提示根和
  全球国家对该车道的承诺。

## 世界状态划分

- 逻辑 Nexus 世界状态是每通道状态空间的并集。公共
  车道保持满状态；私人/机密通道导出默克尔/承诺
  根到合并分类账。
- MV 存储为每个键添加 4 字节通道前缀
  `LaneConfigEntry::key_prefix`，产生诸如`[00 00 00 01] ++之类的键
  打包密钥`。
- 共享表（账户、资产、触发器、治理记录）因此存储
  条目按通道前缀分组，保持范围扫描的确定性。
- 合并账本元数据镜像相同的布局：每个通道写入合并提示
  根并将全局状态根减少为 `lane_{id:03}_merge`，从而允许
  当车道退役时有针对性的保留或驱逐。
- 跨通道索引（账户别名、资产注册表、治理清单）
  存储明确的车道前缀，以便操作员可以快速协调条目。
- **保留政策** — 公共车道保留完整的街区主体；仅限承诺
  检查站后车道可能会挤满较旧的尸体，因为承诺是
  权威的。机密通道将密文日志保存在专用的通道中
  段以避免阻塞其他工作负载。
- **工具** — 维护实用程序（`kagami`、CLI 管理命令）应该
  在公开指标、Prometheus 标签时引用 slugged 命名空间，或者
  归档 Kura 片段。

## 路由和 API

- Torii REST/gRPC 端点接受可选的 `lane_id`；缺席意味着
  `lane_default`。
- SDK 使用通道选择器将用户友好的别名映射到 `LaneId`
  车道目录。
- 路由规则在经过验证的目录上运行，并且可以选择车道和
  数据空间。 `LaneConfig` 为仪表板和
  日志。

## 结算及费用

- 每个通道向全局验证器集支付异或费用。车道可能会收集原生
  天然气代币，但必须与承诺一起托管异或等价物。
- 结算证明包括金额、转换元数据和托管证明
  （例如，转账至全球费用库）。
- 统一结算路由器（NX-3）使用同一通道借记缓冲区
  前缀，因此沉降遥测与存储几何结构一致。

## 治理

- 通道通过目录声明其治理模块。 `LaneConfigEntry`
  携带原始别名和 slug 以保留遥测和审计跟踪
  可读。
- Nexus 注册表分发签名的通道清单，其中包括
  `LaneId`，数据空间绑定，治理句柄，结算句柄，以及
  元数据。
- 运行时升级挂钩继续执行治理策略
  （默认情况下为 `gov_upgrade_id`）并通过遥测桥记录差异
  （`nexus.config.diff` 事件）。

## 遥测和状态

- `/status` 公开通道别名、数据空间绑定、治理句柄和
  沉降概况，源自目录和 `LaneConfig`。
- 调度程序指标 (`nexus_scheduler_lane_teu_*`) 渲染通道别名/slugs
  运营商可以快速绘制积压订单和 TEU 压力图。
- `nexus_lane_configured_total` 计算派生车道条目的数量，并且
  配置更改时重新计算。每当以下情况时，遥测都会发出签名差异
  车道几何形状发生变化。
- 数据空间积压量表包括别名/描述元数据以提供帮助
  运营商将队列压力与业务领域联系起来。

## 配置和 Norito 类型

- `LaneCatalog`、`LaneConfig` 和 `DataSpaceCatalog` 实时存在
  `iroha_data_model::nexus` 并提供 Norito 格式结构
  清单和 SDK。
- `LaneConfig` 存在于 `iroha_config::parameters::actual::Nexus` 中并派生
  自动从目录中；它不需要 Norito 编码，因为它
  是一个内部运行时助手。
- 面向用户的配置 (`iroha_config::parameters::user::Nexus`)
  继续接受声明性通道和数据空间描述符；现在解析
  导出几何图形并拒绝无效别名或重复的车道 ID。

## 出色的工作

- 将结算路由器更新 (NX-3) 与新几何图形集成，以便 XOR 缓冲区
  借方和收据均由通道标记标记。
- 扩展管理工具以列出列族、紧凑退役通道以及
  使用 slugged 命名空间检查每个通道的块日志。
- 最终确定合并算法（排序、修剪、冲突检测）并
  附加回归装置以进行跨车道重播。
- 添加白名单/黑名单和可编程货币的合规挂钩
  政策（在 NX-12 下跟踪）。

---

*本页面将继续跟踪 NX-1 的后续行动，如 NX-2 至 NX-18 着陆。
请在 `roadmap.md` 或治理跟踪器中提出未解决的问题，以便
门户与规范文档保持一致。*