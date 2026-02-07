---
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd986adf52d15dfb82f4396cfa6891efd54c78f528d7621c355dd6d8624f0a02
source_last_modified: "2025-12-29T18:16:35.145965+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-overview
title: Sora Nexus overview
description: High-level summary of the Iroha 3 (Sora Nexus) architecture with pointers to the canonical mono-repo docs.
translator: machine-google-reviewed
---

Nexus (Iroha 3) 通过多通道执行、治理范围扩展了 Iroha 2
数据空间以及跨每个 SDK 的共享工具。此页面反映了新的
`docs/source/nexus_overview.md` 在 mono-repo 中进行了简要介绍，以便门户读者可以
快速了解架构各个部分如何组合在一起。

## 发布线

- **Iroha 2** – 用于联盟或专用网络的自托管部署。
- **Iroha 3 / Sora Nexus** – 运营商所在的多车道公共网络
  注册数据空间（DS）并继承共享治理、结算和
  可观察性工具。
- 两条线均从同一工作区（IVM + Kotodama 工具链）编译，因此 SDK
  修复、ABI 更新和 Norito 装置仍然可移植。运营商下载
  `iroha3-<version>-<os>.tar.zst` 捆绑包加入 Nexus；参考
  `docs/source/sora_nexus_operator_onboarding.md` 用于全屏清单。

## 构建块

|组件|总结|门户挂钩|
|------------|---------|--------------|
|数据空间（DS）|治理定义的执行/存储域，拥有一个或多个通道，声明验证器集、隐私类别、费用 + DA 策略。 |有关清单架构，请参阅 [Nexus 规范](./nexus-spec)。 |
|车道 |执行的确定性分片；发出全球 NPoS 环令的承诺。通道类别包括 `default_public`、`public_custom`、`private_permissioned` 和 `hybrid_confidential`。 | [车道模型](./nexus-lane-model) 捕获几何图形、存储前缀和保留。 |
|过渡计划|占位符标识符、路由阶段和双配置文件打包跟踪单通道部署如何演变为 Nexus。 | [转换说明](./nexus-transition-notes) 记录每个迁移阶段。 |
|空间目录|存储 DS 清单 + 版本的注册表合同。操作员在加入之前根据此目录协调目录条目。 |清单差异跟踪器位于 `docs/source/project_tracker/nexus_config_deltas/` 下。 |
|巷目录| `[nexus]` 配置部分，将通道 ID 映射到别名、路由策略和 DA 阈值。 `irohad --sora --config … --trace-config` 打印已解析的目录以供审核。 |使用 `docs/source/sora_nexus_operator_onboarding.md` 进行 CLI 演练。 |
|结算路由器|连接私人 CBDC 通道与公共流动性通道的 XOR 传输协调器。 | `docs/source/cbdc_lane_playbook.md` 详细说明了策略旋钮和遥测门。 |
|遥测/SLO | `dashboards/grafana/nexus_*.json` 下的仪表板 + 警报捕获通道高度、DA 积压、结算延迟和治理队列深度。 | [遥测修复计划](./nexus-telemetry-remediation) 详细说明了仪表板、警报和审计证据。 |

## 推出快照

|相|焦点 |退出标准|
|--------|---------|----------------|
| N0 – 内测 |理事会管理的注册商 (`.sora`)、手动操作员入职、静态车道目录。 |签署的 DS 清单 + 排练的治理交接。 |
| N1 – 公开发布 |新增`.nexus`后缀、拍卖、自助注册、异或结算接线。 |解析器/网关同步测试、计费协调仪表板、争议桌面演习。 |
| N2 – 扩展 |推出 `.dao`、经销商 API、分析、争议门户、管理员记分卡。 |合规工件版本化、在线政策陪审团工具包、财务透明度报告。 |
| NX-12/13/14门|合规引擎、遥测仪表板和文档必须在合作伙伴试点之前一起交付。 | [Nexus 概述](./nexus-overview) + [Nexus 操作](./nexus-operations) 已发布，仪表板已连接，策略引擎已合并。 |

## 操作员职责

1. **配置卫生** – 保持 `config/config.toml` 与已发布的通道同步 &
   数据空间目录；将 `--trace-config` 输出与每个发行票一起存档。
2. **清单跟踪** – 使目录条目与最新的空间保持一致
   加入或升级节点之前的目录捆绑。
3. **遥测覆盖** – 公开 `nexus_lanes.json`、`nexus_settlement.json`、
   以及相关的 SDK 仪表板；将警报发送至 PagerDuty 并根据遥测修复计划进行季度审查。
4. **事件报告** – 遵循严重性矩阵
   [Nexus 操作](./nexus-operations) 并在五个工作日内提交 RCA。
5. **治理准备** – 参加影响您的车道的 Nexus 理事会投票
   每季度排练回滚指令（通过跟踪
   `docs/source/project_tracker/nexus_config_deltas/`）。

## 另请参阅

- 规范概述：`docs/source/nexus_overview.md`
- 详细规格：[./nexus-spec](./nexus-spec)
- 车道几何形状：[./nexus-lane-model](./nexus-lane-model)
- 过渡计划：[./nexus-transition-notes](./nexus-transition-notes)
- 遥测修复计划：[./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- 操作手册：[./nexus-operations](./nexus-operations)
- 操作员入门指南：`docs/source/sora_nexus_operator_onboarding.md`