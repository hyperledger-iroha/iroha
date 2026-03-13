---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cac03d504c6a7dcfacaa4b298e14f0a71ccbcb5ec58f1977b5bf124300c8ec61
source_last_modified: "2026-01-05T09:28:11.865094+00:00"
translation_last_reviewed: 2026-02-07
id: developer-deployment
title: SoraFS Deployment Notes
sidebar_label: Deployment Notes
description: Checklist for promoting the SoraFS pipeline from CI to production.
translator: machine-google-reviewed
---

:::注意规范来源
:::

# 部署注意事项

SoraFS 打包工作流程强化了确定性，因此从 CI 转向
生产主要需要操作护栏。使用此清单时
将工具推广到真正的网关和存储提供商。

## 飞行前

- **注册表对齐** — 确认分块配置文件和清单引用
  相同的 `namespace.name@semver` 元组 (`docs/source/sorafs/chunker_registry.md`)。
- **准入政策** — 审查已签署的提供商广告和别名证明
  `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`) 需要。
- **Pin 注册表操作手册** — 保留 `docs/source/sorafs/runbooks/pin_registry_ops.md`
  对于恢复场景（别名轮换、复制失败）很方便。

## 环境配置

- 网关必须启用证明流端点 (`POST /v2/sorafs/proof/stream`)
  因此 CLI 可以发出遥测摘要。
- 使用中的默认值配置 `sorafs_alias_cache` 策略
  `iroha_config` 或 CLI 帮助程序 (`sorafs_cli manifest submit --alias-*`)。
- 通过安全秘密管理器提供流令牌（或 Torii 凭证）。
- 启用遥测导出器（`torii_sorafs_proof_stream_*`，
  `torii_sorafs_chunk_range_*`）并将它们运送到您的 Prometheus/OTel 堆栈。

## 推出策略

1. **蓝/绿舱单**
   - 使用 `manifest submit --summary-out` 存档每次部署的响应。
   - 密切关注 `torii_sorafs_gateway_refusals_total` 以捕获功能
     早期不匹配。
2. **证明验证**
   - 将 `sorafs_cli proof stream` 中的故障视为部署阻碍因素；延迟
     峰值通常表明提供商受到限制或层配置错误。
   - `proof verify` 应成为引脚后冒烟测试的一部分，以确保 CAR
     由提供商托管的仍然与清单摘要匹配。
3. **遥测仪表板**
   - 将 `docs/examples/sorafs_proof_streaming_dashboard.json` 导入 Grafana。
   - 为引脚注册表健康分层附加面板
     (`docs/source/sorafs/runbooks/pin_registry_ops.md`) 和块范围统计信息。
4. **多源支持**
   - 按照中的分阶段推出步骤进行操作
     开机时 `docs/source/sorafs/runbooks/multi_source_rollout.md`
     编排器，并将记分板/遥测工件存档以供审核。

## 事件处理

- 按照 `docs/source/sorafs/runbooks/` 中的升级路径进行操作：
  - `sorafs_gateway_operator_playbook.md` 用于网关中断和流令牌
    精疲力尽。
  - 当发生复制争议时，`dispute_revocation_runbook.md`。
  - `sorafs_node_ops.md` 用于节点级维护。
  - `multi_source_rollout.md` 用于协调器覆盖、对等黑名单和
    分阶段推出。
- 通过现有的GovernanceLog记录证明失败和延迟异常
  PoR 跟踪器 API，以便治理可以评估提供商的绩效。

## 后续步骤

- 一旦集成协调器自动化（`sorafs_car::multi_fetch`）
  多源获取协调器 (SF-6b) 登陆。
- 跟踪 SF-13/SF-14 下的 PDP/PoTR 升级； CLI 和文档将演变为
  一旦这些证据稳定下来，就会出现表面的截止日期和等级选择。

通过将这些部署说明与快速入门和 CI 配方相结合，团队
可以从本地实验转移到生产级 SoraFS 管道
可重复、可观察的过程。