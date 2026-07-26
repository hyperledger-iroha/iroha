---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: "SoraFS Migration Ledger"
description: "Canonical change log tracking every migration milestone, owners, and required follow-ups."
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> 改编自 [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md)。

# SoraFS 迁移账本

该分类帐镜像了 SoraFS 中捕获的迁移更改日志
架构 RFC。条目按里程碑分组并列出有效的
窗口、受影响的团队以及所需的行动。迁移计划的更新
必须修改此页面和 RFC (`docs/source/sorafs_architecture_rfc.md`)
使下游消费者保持一致。

|里程碑|有效窗口|变更摘要|受影响的团队 |行动项目|状态 |
|------------|------------------|----------------|----------------|------------------------|--------|
| M1 |第 7-12 周 | CI 强制执行确定性固定装置；暂存中可用的别名证明；工具公开了明确的期望标志。 |文档、存储、治理 |确保装置保持签名，在暂存注册表中注册别名，通过 `--car-digest/--root-cid` 强制更新发布清单。 | ⏳ 待定 |

引用这些里程碑的治理控制平面会议记录如下
`docs/source/sorafs/`。团队应在每行下方添加注明日期的要点
当发生值得注意的事件时（例如，新的别名注册、注册表事件
回顾）以提供可审计的书面记录。

## 最近更新

- 2025-11-01 — 向治理委员会分发 `migration_roadmap.md` 并
  供审查的操作员名单；等待下届理事会会议批准
  （参考：`docs/source/sorafs/council_minutes_2025-10-29.md` 后续）。
- 2025 年 11 月 2 日 — Pin 注册表寄存器 ISI 现在强制执行共享分块器/策略
  通过 `sorafs_manifest` 助手进行验证，保持链上路径对齐
  带有 Torii 支票。
- 2026-02-13 — 将提供商广告推出阶段 (R0–R3) 添加到分类账中，并
  发布了相关的仪表板和操作员指南
  （`provider_advert_rollout.md`、`grafana_sorafs_admission.json`）。