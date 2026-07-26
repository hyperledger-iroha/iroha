---
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c3eddff3a7b9b5dc4eac39251c9df72d0533a6e1b5865c716d54dc3b1c5de164
source_last_modified: "2025-12-29T18:16:35.108030+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w1-plan
title: W1 partner preflight plan
sidebar_label: W1 plan
description: Tasks, owners, and evidence checklist for the partner preview cohort.
translator: machine-google-reviewed
---

|项目 |详情 |
| ---| ---|
|波| W1 — 合作伙伴和 Torii 集成商 |
|目标窗口| 2025 年第二季度第 3 周 |
|人工制品标签（计划中）| `preview-2025-04-12` |
|追踪器问题 | `DOCS-SORA-Preview-W1` |

## 目标

1. 确保合作伙伴预览条款获得法律+治理批准。
2. 暂存邀请包中使用的 Try it 代理和遥测快照。
3. 刷新经过校验和验证的预览工件和探测结果。
4. 在发送邀请之前确定合作伙伴名册 + 请求模板。

## 任务分解

|身份证 |任务|业主|到期 |状态 |笔记|
| ---| ---| ---| ---| ---| ---|
| W1-P1 |获得预览条款附录的法律批准 |文档/DevRel 负责人 → 法律 | 2025-04-05 | ✅ 已完成 |合法票据 `DOCS-SORA-Preview-W1-Legal` 于 2025-04-05 签署； PDF 附在跟踪器上。 |
| W1-P2 |捕获尝试代理暂存窗口 (2025-04-10) 并验证代理运行状况 |文档/开发版本 + 操作 | 2025-04-06 | ✅ 已完成 | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` 于 2025-04-06 执行； CLI 记录 + `.env.tryit-proxy.bak` 已存档。 |
| W1-P3 |构建预览工件 (`preview-2025-04-12`)，运行 `scripts/preview_verify.sh` + `npm run probe:portal`，存档描述符/校验和 |门户网站 TL | 2025-04-08 | ✅ 已完成 | Artefact + 验证日志存储在 `artifacts/docs_preview/W1/preview-2025-04-12/` 下；探头输出连接到跟踪器。 |
| W1-P4 |查看合作伙伴入会表格 (`DOCS-SORA-Preview-REQ-P01…P08`)，确认联系人 + 保密协议 |治理联络| 2025-04-07 | ✅ 已完成 |所有八项请求均获得批准（最后两项请求于 2025 年 4 月 11 日获得批准）；跟踪器中链接的批准。 |
| W1-P5 |起草邀请文案（基于`docs/examples/docs_preview_invite_template.md`），为每个合作伙伴设置`<preview_tag>`和`<request_ticket>` |文档/DevRel 领导 | 2025-04-08 | ✅ 已完成 |邀请草稿于 2025 年 4 月 12 日 15:00 UTC 连同工件链接一起发送。 |

## 飞行前检查表

> 提示：运行 `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` 自动执行步骤 1-5（构建、校验和验证、门户探测、链接检查器和 Try it 代理更新）。该脚本记录了一个 JSON 日志，您可以将其附加到跟踪器问题。

1. `npm run build`（与 `DOCS_RELEASE_TAG=preview-2025-04-12`）重新生成 `build/checksums.sha256` 和 `build/release.json`。
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`。
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`。
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` 和描述符旁边的存档 `build/link-report.json`。
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora`（或通过`--tryit-target`提供适当的目标）；提交更新的 `.env.tryit-proxy` 并保留 `.bak` 进行回滚。
6. 使用日志路径更新 W1 跟踪器问题（描述符校验和、探测输出、尝试代理更改、Grafana 快照）。

## 证据清单

- [x] 已签署的法律批准（PDF 或票据链接）附在 `DOCS-SORA-Preview-W1` 上。
- [x] Grafana `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` 的屏幕截图。
- [x] `preview-2025-04-12` 描述符 + 校验和日志存储在 `artifacts/docs_preview/W1/` 下。
- [x] 填充了 `invite_sent_at` 时间戳的邀请名册表（请参阅跟踪器 W1 日志）。
- [x] 反馈工件镜像在 [`preview-feedback/w1/log.md`](./log.md) 中，每个合作伙伴一行（2025 年 4 月 26 日更新，包含名册/遥测/问题数据）。

随着任务的进展更新此计划；跟踪器引用它来保留路线图
可审计。

## 反馈工作流程

1. 对于每个审阅者，将模板复制到
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   填写元数据，并将完成的副本存储在
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`。
2. 在实时日志中总结邀请、遥测检查点和未决问题
   [`preview-feedback/w1/log.md`](./log.md) 因此治理审核者可以重播整个浪潮
   无需离开存储库。
3. 当知识检查或调查导出到达时，将它们附加到日志中记录的工件路径中
   并交叉链接跟踪器问题。