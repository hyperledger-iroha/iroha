---
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ed6531e0088cf68b6e50d5b3356d1ef96ed6986ff9b46629a2632fa0cc81da65
source_last_modified: "2025-12-29T18:16:35.108666+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w1-summary
title: W1 partner feedback & exit summary
sidebar_label: W1 summary
description: Findings, actions, and exit evidence for the partner/Torii integrator preview wave.
translator: machine-google-reviewed
---

|项目 |详情 |
| ---| ---|
|波| W1 — 合作伙伴和 Torii 集成商 |
|邀请窗口 | 2025-04-12 → 2025-04-26 |
|文物标签| `preview-2025-04-12` |
|追踪器问题 | `DOCS-SORA-Preview-W1` |
|参与者 | sorafs-op-01…03、torii-int-01…02、sdk-partner-01…02、gateway-ops-01 |

## 亮点

1. **校验和工作流程** — 所有审阅者都通过 `scripts/preview_verify.sh` 验证描述符/存档；日志与邀请确认一起存储。
2. **遥测** — `docs.preview.integrity`、`TryItProxyErrors` 和 `DocsPortal/GatewayRefusals` 仪表板在整个波次中保持绿色；没有触发任何事件或警报页面。
3. **文档反馈 (`docs-preview/w1`)** — 提交了两个小问题：
   - `docs-preview/w1 #1`：澄清“尝试”部分中的导航措辞（已解决）。
   - `docs-preview/w1 #2`：更新尝试屏幕截图（已解决）。
4. **Runbook 奇偶校验** — SoraFS 运营商确认 `orchestrator-ops` 和 `multi-source-rollout` 之间的新交叉链接解决了他们的 W0 问题。

## 行动项目

|身份证 |描述 |业主|状态 |
| ---| ---| ---| ---|
| W1-A1 |根据 `docs-preview/w1 #1` 更新尝试导航措辞。 |文档核心-02 | ✅ 已完成（2025-04-18）。 |
| W1-A2|刷新试试 `docs-preview/w1 #2` 的屏幕截图。 |文档核心-03 | ✅ 已完成（2025-04-19）。 |
| W1-A3 |在路线图/状态中总结合作伙伴的发现+遥测证据。 |文档/DevRel 领导 | ✅ 已完成（参见 tracker + status.md）。 |

## 退出总结 (2025-04-26)

- 所有八位审阅者均在最后办公时间内确认完成，清除了当地文物，并撤销了他们的访问权限。
- 退出时遥测保持绿色；最终快照附于 `DOCS-SORA-Preview-W1`。
- 使用退出确认更新邀请日志；跟踪器将 W1 翻转到 🈴 并添加检查点条目。
- 证据包（描述符、校验和日志、探测输出、Try it 代理记录、遥测屏幕截图、反馈摘要）存档于 `artifacts/docs_preview/W1/` 下。

## 后续步骤

- 准备 W2 社区接纳计划（治理批准 + 请求模板调整）。
- 刷新 W2 波的预览工件标签，并在日期确定后重新运行预检脚本。
- 将适用的 W1 调查结果移植到路线图/状态中，以便社区 Wave 拥有最新的指导。