---
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 51971e1dc4e763ac7017f76c7239eef943bc21151e49e827988b61972fa58245
source_last_modified: "2025-12-29T18:16:35.107308+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w1-log
title: W1 feedback & telemetry log
sidebar_label: W1 feedback log
description: Aggregate roster, telemetry checkpoints, and reviewer notes for the first partner preview wave.
translator: machine-google-reviewed
---

此日志保留邀请名册、遥测检查点和审阅者反馈
**W1 合作伙伴预览**，伴随接受任务
[`preview-feedback/w1/plan.md`](./plan.md) 和波形跟踪器条目
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md)。每当收到邀请时更新它
发送、记录遥测快照或对反馈项目进行分类，以便治理审核者可以重播
无需追寻外部票据即可获得证据。

## 队列名单

|合作伙伴 ID |索取门票 |收到 NDA |邀请已发送 (UTC) |确认/首次登录 (UTC) |状态 |笔记|
| ---| ---| ---| ---| ---| ---| ---|
|合作伙伴-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ 2025-04-26 完成 |索拉夫-op-01；专注于协调器文档平价证据。 |
|合作伙伴-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ 2025-04-26 完成 |索拉夫-op-02；已验证 Norito/遥测交叉链接。 |
|合作伙伴-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ 2025-04-26 完成 |索拉夫-op-03；进行多源故障转移演练。 |
|合作伙伴-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ 2025-04-26 完成 |鸟居-int-01； Torii `/v1/pipeline` + 尝试食谱评论。 |
|合作伙伴-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ 2025-04-26 完成 |鸟居-int-02；配对尝试屏幕截图更新 (docs-preview/w1 #2)。 |
|合作伙伴-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ 2025-04-26 完成 | SDK-合作伙伴-01； JS/Swift 食谱反馈 + ISO 桥完整性检查。 |
|合作伙伴-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ 2025-04-26 完成 | SDK-合作伙伴-02；合规性于 2025 年 4 月 11 日通过，重点关注连接/遥测注释。 |
|合作伙伴-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ 2025-04-26 完成 |网关-ops-01；审核网关操作指南 + 匿名尝试代理流程。 |

发出出站电子邮件后，立即填充 **发送邀请** 和 **确认** 时间戳。
将时间锚定到 W1 计划中定义的 UTC 时间表。

## 遥测检查点

|时间戳 (UTC) |仪表板/探头|业主|结果 |文物|
| ---| ---| ---| ---| ---|
| 2025-04-06 18:05 | `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals` |文档/开发版本 + 操作 | ✅ 全绿色 | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | `npm run manage:tryit-proxy -- --stage preview-w1` 成绩单 |行动| ✅ 上演 | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | `docs.preview.integrity`、`TryItProxyErrors`、`DocsPortal/GatewayRefusals`、`probe:portal` |文档/开发版本 + 操作 | ✅ 预邀请快照，无回归 | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 |上面的仪表板 + 尝试代理延迟差异 |文档/DevRel 领导 | ✅ 中点检查已通过（0 个警报；尝试一下延迟 p95=410ms）| `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 |上面的仪表板 + 出口探针 |文档/DevRel + 治理联络 | ✅ 退出快照，零未完成警报 | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

每日办公时间样本 (2025-04-13 → 2025-04-25) 捆绑为 NDJSON + PNG 导出，位于
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` 与文件名
`docs-preview-integrity-<date>.json` 和相应的屏幕截图。

## 反馈和问题日志

使用此表总结审稿人提交的发现。将每个条目链接到 GitHub/讨论
票证加上通过捕获的结构化表格
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)。

|参考|严重性 |业主|状态 |笔记|
| ---| ---| ---| ---| ---|
| `docs-preview/w1 #1` |低|文档核心-02 | ✅ 2025-04-18 解决 |澄清“尝试一下”导航措辞 + 侧边栏锚点（`docs/source/sorafs/tryit.md` 使用新标签更新）。 |
| `docs-preview/w1 #2` |低|文档核心-03 | ✅ 2025-04-19 解决 |根据审阅者请求刷新尝试屏幕截图 + 标题；人工制品 `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`。 |
| — |信息 |文档/DevRel 领导 | 🟢 已关闭 |其余评论仅限问答；在 `artifacts/docs_preview/W1/preview-2025-04-12/feedback/` 下每个合作伙伴的反馈表中获取。 |

## 知识检查和调查跟踪

1.记录每位审稿人的测验分数（目标≥90%）；将导出的 CSV 附加到
   邀请文物。
2. 收集通过反馈表模板捕获的定性调查答案并进行镜像
   在 `artifacts/docs_preview/W1/preview-2025-04-12/surveys/` 下。
3. 为得分低于阈值的任何人安排补救电话，并将其记录在此文件中。

所有八位审稿人在知识检查中得分均≥94%（CSV：
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`）。没有接到任何补救电话
被要求；调查每个合作伙伴的出口情况
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`。

## 文物清单

- 预览描述符/校验和包：`artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- 探测 + 链路检查摘要：`artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- 尝试代理更改日志：`artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- 遥测导出：`artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- 每日办公时间遥测捆绑包：`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- 反馈 + 调查导出：将审阅者特定的文件夹放置在
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- 知识检查 CSV 和摘要：`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

保持库存与跟踪器问题同步。将工件复制到时附加哈希值
治理票证，以便审计员无需 shell 访问即可验证文件。