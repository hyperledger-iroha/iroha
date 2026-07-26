---
lang: zh-hans
direction: ltr
source: docs/examples/sns/arbitration_transparency_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 305a3f3b253a013825d4dd798d2282e111913ec777fe0fbf5b02a92c7172b92a
source_last_modified: "2025-12-29T18:16:35.076964+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
# SNS 仲裁透明度报告 — <YYYY 月>

- **后缀：** `<.sora / .nexus / .dao>`
- **报告窗口：** `<ISO start>` → `<ISO end>`
- **编制者：** `<Council liaison>`
- **源工件：** `cases.ndjson` SHA256 `<hash>`，仪表板导出 `<filename>.json`

## 1. 执行摘要

- 新病例总数：`<count>`
- 本期结案案件：`<count>`
- SLA 合规性：`<ack %>` 确认/`<resolution %>` 决定
- 监护人覆盖已发出：`<count>`
- 转账/退款执行：`<count>`

## 2. 案例组合

|争议类型|新案例 |已结案件 |中值分辨率（天）|
|--------------|------------|--------------|----------------------------|
|所有权| 0 | 0 | 0 |
|违反政策 | 0 | 0 | 0 |
|滥用 | 0 | 0 | 0 |
|计费| 0 | 0 | 0 |
|其他| 0 | 0 | 0 |

## 3.SLA 性能

|优先|确认 SLA |已达成 | SLA | 决议已达成 |违规|
|----------|-----------------|----------|----------------|----------|----------|
|紧急| ≤ 2 小时 | 0% | ≤ 72 小时 | 0% | 0 |
|高| ≤ 8 小时 | 0% | ≤10天| 0% | 0 |
|标准| ≤ 24 小时 | 0% | ≤ 21 天 | 0% | 0 |
|信息 | ≤ 3d | 0% | ≤ 30 天 | 0% | 0 |

描述任何违规行为的根本原因并链接到补救通知单。

## 4. 案例登记

|案例编号 |选择器|优先|状态 |结果|笔记|
|--------|----------|----------|--------|---------|--------|
| SNS-YYYY-NNNNN | `label.suffix` |标准|关闭 |维持| `<summary>` |

提供引用匿名事实或公众投票链接的一行注释。密封件
在需要时并提及应用的修订。

## 5. 行动与补救措施

- **冻结/释放：** `<counts + case ids>`
- **转账：** `<counts + assets moved>`
- **计费调整：** `<credits/debits>`
- **政策跟进：** `<tickets or RFCs opened>`

## 6. 上诉和监护人优先

总结所有升级到监护委员会的上诉，包括时间戳和
决定（批准/拒绝）。链接至 `sns governance appeal` 记录或理事会
投票。

## 7. 未完成项目

- `<Action item>` — 所有者 `<name>`，预计到达时间 `<date>`
- `<Action item>` — 所有者 `<name>`，预计到达时间 `<date>`

附上本报告中引用的 NDJSON、Grafana 导出和 CLI 日志。