---
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w3/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 76a4303fa2657476a3f983f1aa5597c9ddb478f670d233b0a7cf4e3791419a72
source_last_modified: "2025-12-29T18:16:35.110857+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w3-summary
title: W3 beta feedback & status
sidebar_label: W3 summary
description: Live digest for the 2026 beta preview wave (finance, observability, SDK, and ecosystem cohorts).
translator: machine-google-reviewed
---

|项目 |详情 |
| ---| ---|
|波| W3 — Beta 群组（财务 + 运营 + SDK 合作伙伴 + 生态系统倡导者）|
|邀请窗口 | 2026-02-18 → 2026-02-28 |
|文物标签| `preview-20260218` |
|追踪器问题 | `DOCS-SORA-Preview-W3` |
|参与者 |财务-beta-01、可观察性-ops-02、合作伙伴-sdk-03、生态系统-倡导者-04 |

## 亮点

1. **端到端证据管道。** `npm run preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01 --notes "Finance/observability beta wave"` 生成每波摘要 (`artifacts/docs_portal_preview/preview-20260218-summary.json`)、摘要 (`preview-20260218-digest.md`) 并刷新 `docs/portal/src/data/previewFeedbackSummary.json`，以便治理审核人员可以依赖单个命令。
2. **遥测+治理覆盖范围。** 所有四位审阅者均承认校验和门控访问，提交反馈，并按时撤销；该摘要引用了反馈问题（`docs-preview/20260218` 集 + `DOCS-SORA-Preview-20260218`）以及在 Wave 期间收集的 Grafana 运行。
3. **门户呈现。** 刷新后的门户表现在显示已关闭的 W3 波以及延迟和响应率指标，下面的新日志页面反映了不提取原始 JSON 日志的审核员的事件时间线。

## 行动项目

|身份证 |描述 |业主|状态 |
| ---| ---| ---| ---|
| W3-A1 |捕获预览摘要并附加到跟踪器。 |文档/DevRel 领导 | ✅ 2026-02-28 完成 |
| W3-A2 |将邀请/摘要证据镜像到门户+路线图/状态中。 |文档/DevRel 领导 | ✅ 2026-02-28 完成 |

## 退出总结 (2026-02-28)

- 2026 年 2 月 18 日发出邀请，并在几分钟后记录确认；最终遥测检查通过后，预览访问权限于 2026 年 2 月 28 日被撤销。
- 在 `artifacts/docs_portal_preview/` 下捕获的摘要 + 摘要，原始日志由 `artifacts/docs_portal_preview/feedback_log.json` 锚定以实现可重玩性。
- 向治理跟踪器 `DOCS-SORA-Preview-20260218` 提交 `docs-preview/20260218` 下的问题跟进； CSP/Try it 注释路由至可观察性/财务所有者并从摘要链接。
- 跟踪器行更新为🈴已完成，门户反馈表反映了已关闭的浪潮，完成了剩余的 DOCS-SORA beta 准备任务。