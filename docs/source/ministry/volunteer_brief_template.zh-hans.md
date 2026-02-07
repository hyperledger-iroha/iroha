---
lang: zh-hans
direction: ltr
source: docs/source/ministry/volunteer_brief_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cae4747782524b545fdcd52e7523cce0f5b60ddb85f32c747c5f57a63f85ccdc
source_last_modified: "2025-12-29T18:16:35.984008+00:00"
translation_last_reviewed: 2026-02-07
title: Volunteer Brief Template
summary: Structured template for roadmap item MINFO-3a covering balanced briefs, fact tables, conflict disclosures, and moderation tags.
translator: machine-google-reviewed
---

# 志愿者简介模板 (MINFO-3a)

路线图参考：**MINFO-3a — 平衡的简要模板和冲突披露。**

志愿者提交的简短意见总结了公民小组希望政府在提出黑名单变更或其他部委执法动议时审查的立场。 MINFO-3a 要求每份简报都遵循确定性结构，以便透明度管道能够 (1) 呈现可比较的事实表，(2) 确认利益冲突已披露，以及 (3) 自动删除或标记偏离主题的提交内容。此页面定义了 `cargo xtask ministry-transparency` 中附带的工具所需的规范字段、CSV 样式事实表布局和审核标签。

> **Norito 架构：** `iroha_data_model::ministry::VolunteerBriefV1` 结构（版本 `1`）现在是所有提交的权威架构。工具和门户验证程序在发布摘要或在小组摘要中引用它之前会调用 `VolunteerBriefV1::validate`。

## 提交有效负载结构

|部分|领域 |要求|
|---------|--------|--------------|
| **信封** | `version` (u16) |必须是 `1`。版本保护允许该部毫无歧义地发展模式。 |
| **身份与立场** | `brief_id`（字符串，每个日历年唯一）、`proposal_id`（黑名单或政策动议的链接）、`language` (BCP-47)、`stance` （`support`/`oppose`/`context`），`submitted_at`（RFC3339）|所有字段均必填。 `stance` 提供仪表板并且必须匹配允许的词汇表。 |
| **作者信息** | `author.name`、`author.organization`（可选）、`author.contact`、`author.no_conflicts_certified`（布尔）| `author.contact` 是从公共仪表板中编辑的，但存储在原始工件中。仅当作者证明没有披露适用时才设置 `no_conflicts_certified: true`。 |
| **总结** | `summary.title`、`summary.abstract`、`summary.requested_action` |文本概述出现在事实表旁边。将 `summary.abstract` 限制为 ≤2000 个字符。 |
| **事实表** | `fact_table` 阵列（参见下一节）|即使是短内裤也需要。 CLI 和透明度摄取作业会拒绝没有事实表的提交。 |
| **披露** | `disclosures` 数组或 `author.no_conflicts_certified: true` |每个披露行必须包含 `type` (`financial`、`employment`、`governance`、`family`、`other`)、`entity`、 `relationship` 和 `details`。 |
| **审核元数据** | `moderation.off_topic`（布尔）、`moderation.tags`（枚举字符串数组）、`moderation.notes` |审稿人用来压制 astroturfing 或不相关的提交内容。离题条目不会对仪表板做出贡献。 |

## 事实表规范

每个 `fact_table` 行捕获一个机器可读的声明。将行存储为具有以下字段的 JSON 对象：|领域|描述 |
|--------|-------------|
| `claim_id` |稳定标识符（例如 `VB-2026-04-F1`）。 |
| `claim` |对事实或影响的单句陈述。 |
| `status` | `corroborated`、`disputed`、`context-only` 之一。 |
| `impact` |包含 `governance`、`technical`、`compliance`、`community` 中的一个或多个的数组。 |
| `citations` |非空字符串数组。接受 URL、Torii 案例 ID 或 CID 参考。 |
| `evidence_digest` |支持文档的可选 BLAKE3 校验和。 |

自动化注意事项：
- 摄取作业计数 `fact_rows` 和 `fact_rows_with_citation` 以构建发布记分卡。没有引用的行仍然出现在人类可读的表中，但被跟踪为缺失的证据。
- 保持声明简洁并引用治理提案中使用的相同标识符，以便交叉链接具有确定性。

## 冲突披露要求

1. 当存在财务、就业、治理或家庭关系时，至少提供一项披露条目。
2. 使用 `author.no_conflicts_certified: true` 断言“无已知冲突”。提交的内容必须包含披露条目或 `true` 认证；否则，它们会在摄取过程中被标记。
3. 只要存在公共文件（例如，公司备案、DAO 投票），请包含 `disclosures[i].evidence`。对于“无”认证，证据是可选的，但强烈建议提供。

## 审核标签和离题处理

审核者可以在进入透明度管道之前对提交内容进行标记：

- `moderation.off_topic: true` 从聚合计数中删除该条目，同时递增 `off_topic_rejections` 计数器。该行仍然可以在原始档案中用于审计。
- `moderation.tags` 接受枚举值：`duplicate`、`needs-translation`、`needs-follow-up`、`spam`、`astroturf`、`policy-escalation`。标签可帮助下游审阅者进行分类，而无需重新阅读完整的摘要。
- `moderation.notes` 存储审核决定的简短理由（≤512 个字符）。

## 提交清单

1. 使用此模板或下述帮助器 CLI 填写 JSON 负载。
2. 填充至少一个事实表行；包括每行的引用。
3. 提供披露或明确设置`author.no_conflicts_certified: true`。
4. 附加审核元数据（默认 `off_topic: false`），以便审阅者可以快速分类。
5. 上传前使用 `cargo xtask ministry-transparency ingest --volunteer <file>` 或任何 Norito 验证器验证有效负载。

## 验证 CLI (MINFO-3)

该存储库现在为志愿者简介提供了专用验证器：

```bash
cargo xtask ministry-transparency volunteer-validate \
  --input docs/examples/ministry/volunteer_brief_template.json \
  --json-output artifacts/ministry/volunteer_lint_report.json
```

关键行为：- 接受单独的 JSON 对象*或*内裤数组；多次传递 `--input` 以在一次运行中检查多个文件。
- 发出一个简短的摘要，显示错误和警告的数量；警告会突出显示空的引文列表或过长的注释，而错误则会阻止发表。
- 确保必填字段（`brief_id`、`proposal_id`、`stance`、事实表内容、披露或 `no_conflicts_certified`）与此模板匹配，并且枚举值保留在记录的词汇表内。
- 当设置 `--json-output <path>` 时，验证器会写入一个机器可读的清单，总结每个摘要（提案 ID、立场、状态、错误/警告）。门户的 `npm run generate:volunteer-lint` 命令使用此清单来显示每个提案页面旁边的 lint 状态。

将命令集成到门户工作流程或 CI 中，以确保志愿者提交的内容在到达透明度摄取作业之前符合 **MINFO-3**。

## 负载示例

请参阅 `docs/examples/ministry/volunteer_brief_template.json` 以获取完整填充的示例，包括事实表行、披露和审核标签。下游仪表板使用原始 JSON 并自动计算：

- `total_briefs`（不包括题外话）
- `fact_rows` / `fact_rows_with_citation`
- `disclosures_missing`
- `off_topic_rejections`

如果需要新字段，请在同一更改中更新此文档和摄取摘要器 (`xtask/src/ministry.rs`)，以便治理证据保持可重现。

## 出版物 SLA 和门户表面处理 (MINFO-3)

为了保持公民提交的透明度，门户现在在通过验证后以固定的节奏发布简报：

1. **T+0–6 小时：** 通过志愿者报名表或 `cargo xtask ministry-transparency ingest` 提交意见。验证器运行 `VolunteerBriefV1::validate`，拒绝格式错误的有效负载，并发出 lint 报告（缺少披露、重复的事实 ID 等）。
2. **T+6–24 小时：** 接受的简报将排队等待翻译/分类。应用审核标签（`needs-translation`、`duplicate`、`policy-escalation`，...），偏离主题的条目将被存档，但从聚合计数中排除。
3. **T+24–48 小时：** 门户网站在相应的提案页面旁边发布简报。现在，每个已发布的提案都链接到“志愿者意见”，因此审阅者可以在不打开原始 JSON 的情况下阅读支持/反对/背景摘要。

如果提交标记为 `policy-escalation` 或 `astroturf`，SLA 会收紧至 **12 小时**，以便治理可以快速响应。操作员可以通过文档门户 (`docs/portal/docs/ministry/volunteer-briefs.md`) 中的 **志愿者简介** 页面审核 SLA，其中列出了最新的发布窗口、lint 状态以及 Norito 工件的链接。