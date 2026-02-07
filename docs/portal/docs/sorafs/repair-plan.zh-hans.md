---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/repair-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 302b74b4022656e57c2b876a8f15bf5301a593030a18ad1b93780061e5d783ef
source_last_modified: "2026-01-21T19:17:13.232211+00:00"
translation_last_reviewed: 2026-02-07
id: repair-plan
title: SoraFS Repair Automation & Auditor API
sidebar_label: Repair Automation
description: Governance policy, escalation lifecycle, and API expectations for SoraFS repair automation.
translator: machine-google-reviewed
---

:::注意规范来源
镜子 `docs/source/sorafs_repair_plan.md`。保持两个版本同步，直到 Sphinx 集退役。
:::

## 治理决策生命周期
1. 升级修复创建斜杠提案草案并打开争议窗口。
2. 治理投票者在争议窗口期间提交批准/拒绝投票。
3. 在 `escalated_at_unix + dispute_window_secs` 中，决策是确定性计算的：最少投票者、批准超过拒绝，并且批准率满足法定人数阈值。
4. 批准的决定打开上诉窗口； `approved_at_unix + appeal_window_secs` 之前记录的上诉将决定标记为已上诉。
5. 处罚上限适用于所有提案；超过上限的提交将被拒绝。

## 治理升级政策
升级策略源自 `iroha_config` 中的 `governance.sorafs_repair_escalation`，并对每个修复削减提案强制执行。

|设置|默认|意义|
|---------|---------|---------|
| `quorum_bps` | 6667 |计票中的最低支持率（基点）。 |
| `minimum_voters` | 3 |做出决定所需的最小数量的不同选民。 |
| `dispute_window_secs` | 86400 |升级后到投票最终确定之前的时间（秒）。 |
| `appeal_window_secs` | 604800 |批准后接受申诉的时间（秒）。 |
| `max_penalty_nano` | 1,000,000,000 |修复升级允许的最大斜线惩罚（纳米异或）。 |

- 调度程序生成的提案上限为 `max_penalty_nano`；审核员提交的超出上限的意见将被拒绝。
- 投票记录以确定性排序（`voter_id` 排序）存储在 `repair_state.to` 中，因此所有节点都得出相同的决策时间戳和结果。