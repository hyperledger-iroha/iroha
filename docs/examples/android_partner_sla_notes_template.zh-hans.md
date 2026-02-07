---
lang: zh-hans
direction: ltr
source: docs/examples/android_partner_sla_notes_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ca51ec624ebbb4b3760d5f2265d31047cd3b6492e21bdb10a3aa61655ccca69
source_last_modified: "2025-12-29T18:16:35.069181+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android 合作伙伴 SLA 发现说明 — 模板

将此模板用于每个 AND8 SLA 发现会话。保存填写好的副本
在 `docs/source/sdk/android/partner_sla_sessions/<partner>/<date>/minutes.md` 下
并附上支持性资料（调查问卷答复、致谢、
附件）在同一目录中。

```
Partner: <Name>                      Date: <YYYY-MM-DD>  Time: <UTC>
Primary contact(s): <names, roles, email>
Android attendees: <Program Lead / Partner Eng / Support Eng / Compliance>
Meeting link / ticket: <URL or ID>
```

## 1. 议程和背景

- 会议的目的（试点范围、发布窗口、遥测期望）。
- 在电话会议前共享的参考文档（支持手册、发布日历、
  遥测仪表板）。

## 2. 工作负载概述

|主题 |笔记|
|--------|--------|
|目标工作负载/链 | |
|预计交易量 | |
|关键业务窗口/停电期 | |
|监管制度（GDPR、MAS、FISC 等）| |
|所需语言/本地化 | |

## 3.SLA 讨论

| SLA 等级 |合作伙伴期望 |相对于基线的增量？ |需要采取行动|
|----------|--------------------|----------------------|-----------------|
|关键修复（48 小时）| |是/否 | |
|高严重性（5 个工作日）| |是/否 | |
|维护（30 天）| |是/否 | |
|割接通知（60 天）| |是/否 | |
|事件通讯节奏 | |是/否 | |

记录合作伙伴要求的任何附加 SLA 条款（例如专用
电话桥，额外的遥测出口）。

## 4. 遥测和访问要求

- Grafana / Prometheus 访问需求：
- 日志/跟踪导出要求：
- 离线证据或档案期望：

## 5. 合规性和法律说明

- 管辖区通知要求（法规+时间）。
- 事件更新所需的法律联系人。
- 数据驻留限制/存储要求。

## 6. 决策和行动项目

|项目 |业主|到期|笔记|
|------|--------|-----|--------|
| | | | |

## 7.致谢

- 合作伙伴承认基线 SLA？ （是/否）
- 后续确认方式（电子邮件/票据/签名）：
- 关闭前将确认电子邮件或会议纪要附加到此目录。