---
id: preview-feedback-w2-plan
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: W2 community intake plan
sidebar_label: W2 plan
description: Intake, approvals, and evidence checklist for the community preview cohort.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

|项目 |详情 |
| ---| ---|
|波| W2 — 社区审阅者 |
|目标窗口| 2025 年第 3 季度第 1 周（暂定）|
|人工制品标签（计划中）| `preview-2025-06-15` |
|追踪器问题 | `DOCS-SORA-Preview-W2` |

## 目标

1. 定义社区接纳标准和审查工作流程。
2. 获得治理部门对拟议名册和可接受用途附录的批准。
3. 在新窗口中刷新经过校验和验证的预览工件和遥测包。
4. 在邀请发送之前暂存 Try it 代理 + 仪表板。

## 任务分解

|身份证 |任务|业主|到期 |状态 |笔记|
| ---| ---| ---| ---| ---| ---|
| W2-P1 |起草社区接纳标准（资格、最大名额、CoC 要求）并分发给治理 |文档/DevRel 领导 | 2025-05-15 | ✅ 已完成 |接收政策合并到 `DOCS-SORA-Preview-W2` 中，并在 2025 年 5 月 20 日理事会会议上批准。 |
| W2-P2 |使用社区特定问题（动机、可用性、本地化需求）更新请求模板 |文档核心-01 | 2025-05-18 | ✅ 已完成 | `docs/examples/docs_preview_request_template.md` 现在包括社区部分，在接收表格中引用。 |
| W2-P3 |确保接收计划得到治理批准（会议投票 + 记录会议记录）|治理联络| 2025-05-22 | ✅ 已完成 |投票于2025年5月20日一致通过；分钟 + 点名链接在 `DOCS-SORA-Preview-W2` 中。 |
| W2-P4 |计划尝试 W2 窗口的代理暂存 + 遥测捕获 (`preview-2025-06-15`) |文档/开发版本 + 操作 | 2025-06-05 | ✅ 已完成 |变更票 `OPS-TRYIT-188` 批准并执行 2025-06-09 02:00–04:00UTC； Grafana 屏幕截图与票证一起存档。 |
| W2-P5 |构建/验证新的预览工件标签 (`preview-2025-06-15`) 和存档描述符/校验和/探测日志 |门户网站 TL | 2025-06-07 | ✅ 已完成 | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` 运行于 2025-06-10；输出存储在 `artifacts/docs_preview/W2/preview-2025-06-15/` 下。 |
| W2-P6 |组建社区邀请名册（≤25 名审稿人，分批），并提供经政府批准的联系信息 |社区经理 | 2025-06-10 | ✅ 已完成 |第一批 8 名社区评审员获得批准；跟踪器中记录的请求 ID `DOCS-SORA-Preview-REQ-C01…C08`。 |

## 证据清单

- [x] 治理批准记录（会议记录 + 投票链接）附于 `DOCS-SORA-Preview-W2`。
- [x] 更新了在 `docs/examples/` 下提交的请求模板。
- [x] `preview-2025-06-15` 描述符、校验和日志、探测输出、链接报告以及存储在 `artifacts/docs_preview/W2/` 下的 Try it 代理记录。
- [x] Grafana 为 W2 预检窗口捕获的屏幕截图（`docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals`）。
- [x] 邀请名册表，其中包含审阅者 ID、请求票证和在发送前填充的批准时间戳（请参阅跟踪器 W2 部分）。

保持该计划的更新；跟踪器会引用它，以便 DOCS-SORA 路线图可以准确地看到 W2 邀请发出之前剩余的内容。