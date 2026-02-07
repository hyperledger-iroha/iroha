---
lang: zh-hans
direction: ltr
source: docs/source/ministry/red_team_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f4a2e50e18749f64212dee5186bfd3a3d034ac906d1a506d420cdcb1b5117517
source_last_modified: "2025-12-29T18:16:35.979926+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Red-Team Status (MINFO-9)
summary: Snapshot of the chaos drill program covering upcoming runs, last completed scenario, and remediation items.
translator: machine-google-reviewed
---

# 事工红队状态

本页是对[审核红队计划](moderation_red_team_plan.md) 的补充
通过跟踪近期演练日历、证据包和补救措施
状态。每次运行后与捕获的文物一起更新它
`artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`。

## 即将进行的演习

|日期 (UTC) |场景 |所有者 |证据准备|笔记|
|------------|---------|----------|----------------|--------|
| 2026-11-12 | **蒙眼行动** — Taikai 混合模式走私演习与网关降级尝试 |安全工程（Miyu Sato），部门行动（Liam O’Connor）| `scripts/ministry/scaffold_red_team_drill.py` 捆绑包 `docs/source/ministry/reports/red_team/2026-11-operation-blindfold.md` + 暂存目录 `artifacts/ministry/red-team/2026-11/operation-blindfold/` |练习 GAR/Taikai 重叠以及 DNS 故障转移；需要在启动前将 Merkle 快照列入拒绝列表，并在捕获仪表板后运行 `export_red_team_evidence.py`。 |

## 最后一次演练快照

|日期 (UTC) |场景 |证据包|补救和跟进|
|------------|---------|-----------------|----------------------------------------|
| 2026-08-18 | **海玻璃行动** — 网关走私、治理重播和警报限电演练 | `artifacts/ministry/red-team/2026-08/operation-seaglass/`（Grafana 导出、Alertmanager 日志、`seaglass_evidence_manifest.json`）| **开放：** 重放密封自动化（`MINFO-RT-17`，所有者：Governance Ops，截止日期为 2026 年 9 月 5 日）； pin 仪表板冻结为 SoraFS（`MINFO-RT-18`，可观察性，2026 年 8 月 25 日到期）。 **已关闭：** 日志模板已更新以携带 Norito 清单哈希值。 |

## 跟踪和工具

- 使用`scripts/ministry/moderation_payload_tool.py`封装注射剂
  每个场景的有效负载和拒绝列表补丁。
- 通过 `scripts/ministry/export_red_team_evidence.py` 记录仪表板/日志捕获
  每次演习后立即进行，以便证据清单包含签名的哈希值。
- CI 防护 `ci/check_ministry_red_team.sh` 强制执行已提交的演练报告
  不包含占位符文本并且引用的工件之前存在
  合并。

请参阅 `status.md`（§ *部委红队状态*）以获取引用的实时摘要
在每周的协调电话中。