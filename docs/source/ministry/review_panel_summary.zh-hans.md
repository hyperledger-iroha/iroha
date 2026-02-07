---
lang: zh-hans
direction: ltr
source: docs/source/ministry/review_panel_summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7325e72d18ec406eb134622ab51211fbb6582ebcc26bd719499e209db70f761b
source_last_modified: "2025-12-29T18:16:35.983094+00:00"
translation_last_reviewed: 2026-02-07
title: Review Panel Summary Workflow (MINFO-4a)
summary: Generate the neutral referendum summary with balanced citations, AI manifest references, and volunteer brief coverage.
translator: machine-google-reviewed
---

# 审查小组摘要 (MINFO-4a)

路线图项目 **MINFO-4a — 中立摘要生成器** 需要一个可重复的工作流程，将已接受的议程提案、志愿者简报语料库和经过验证的 AI 审核清单转换为中立的公投摘要。可交付成果必须：

- 将输出记录为 Norito 结构 (`ReviewPanelSummaryV1`)，以便治理可以将其与清单和选票一起存档。
- 整理源材料，当审查小组没有平衡的支持/反对覆盖范围或事实缺少引用时，会快速失败。
- 在每个亮点中参考人工智能清单和提案证据包，确保政策陪审团在投票前看到自动化和人类背景。

## CLI 用法

该工作流程作为 `cargo xtask` 的一部分提供：

```bash
cargo xtask ministry-panel synthesize \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --volunteer docs/examples/ministry/volunteer_brief_template.json \
  --ai-manifest docs/examples/ai_moderation_calibration_manifest_202602.json \
  --panel-round RP-2026-05 \
--output artifacts/review_panel/AC-2026-001-RP-2026-05.json
```

所需输入：

1. `--proposal` – 遵循 `AgendaProposalV1` 的 JSON 有效负载。帮助程序在生成摘要之前验证架构。
2. `--volunteer` – `docs/source/ministry/volunteer_brief_template.md` 后面的志愿者简介的 JSON 数组。偏离主题的条目将被自动忽略。
3. `--ai-manifest` – 治理签名的 `ModerationReproManifestV1` 描述了筛选内容的 AI 委员会。
4. `--panel-round` – 当前审核轮次的标识符 (`RP-YYYY-##`)。
5. `--output` – 目标文件或 `-` 以流式传输到标准输出。使用 `--language` 覆盖提案语言，并使用 `--generated-at` 在回填历史记录时提供确定性 Unix 时间戳（毫秒）。

生成独立摘要后，运行
[`cargo xtask ministry-panel packet`](referendum_packet.md) 组装助手
完整的公投档案 (`ReferendumPacketV1`)。供应
`--summary-out` 到数据包命令将保留相同的摘要文件，同时
将其嵌入到下游消费者的数据包对象中。

### 通过 `ministry-transparency ingest` 实现自动化

已经为季度证据包运行 `cargo xtask ministry-transparency ingest` 的团队现在可以将审核小组摘要拼接到同一管道中：

```bash
cargo xtask ministry-transparency ingest \
  --quarter 2026-Q4 \
  --ledger artifacts/ministry/ledger.json \
  --appeals artifacts/ministry/appeals.json \
  --denylist artifacts/ministry/denylist.json \
  --treasury artifacts/ministry/treasury.json \
  --volunteer artifacts/ministry/volunteer_briefs.json \
  --panel-proposal artifacts/ministry/proposal_AC-2026-041.json \
  --panel-ai-manifest artifacts/ministry/ai_manifest.json \
  --panel-round RP-2026-05 \
  --panel-summary-out artifacts/ministry/review_panel_summary.json \
  --output artifacts/ministry/ingest.json
```

所有四个 `--panel-*` 标志必须一起提供（并且需要 `--volunteer`）。该命令将审查小组摘要发送到 `--panel-summary-out`，将解析后的有效负载嵌入到摄取快照中，并记录校验和，以便下游工具可以证明证据。

## Linting 和故障模式

`cargo xtask ministry-panel synthesize` 在编写摘要之前强制执行以下不变量：

- **平衡立场：** 必须至少提供一份支持简报和一份反对简报。缺少覆盖率会终止运行并出现描述性错误。
- **引文覆盖率：**亮点仅由包含引文的事实行生成。缺少的引文永远不会阻止构建，但每个受影响的摘要都会在输出中的 `warnings[]` 下列出。
- **每个突出显示引用：** 每个突出显示都包含对 (a) 志愿者事实行、(b) AI 清单 ID 和 (c) 提案中的第一个证据附件的引用，因此数据包始终链接回已签名的工件。如果任何检查失败，该命令将以非零状态退出并指向有问题的记录。成功运行会写入与 `ReviewPanelSummaryV1` 架构匹配的 JSON 文件，并且可以嵌入到治理清单中。

## 输出结构

`ReviewPanelSummaryV1` 存在于 `crates/iroha_data_model/src/ministry/mod.rs` 中，每个消费者都可以通过 `iroha_data_model` 箱使用。关键部分包括：

- `overview` – 政策陪审团数据包的标题、中性总结句和决策上下文。
- `stance_distribution` – 每个立场的摘要和事实行计数。下游仪表板在发布之前会读取此内容以确认覆盖范围。
- `highlights` – 每个立场最多两个事实摘要以及完全合格的引文。
- `ai_manifest` – 从再现性清单中提取元数据（清单 UUID、运行程序版本、阈值）。
- `volunteer_references` – 用于审核的每个简要统计数据（语言、立场、行、引用行）。
- `warnings` – 描述跳过项目的自由格式 lint 消息（例如，缺少引用的事实行）。

## 示例

`docs/examples/ministry/review_panel_summary_example.json` 包含使用帮助程序生成的完整示例。它展示了平衡的支持/反对覆盖率、引文连接、清单参考文献以及无法提升为亮点的事实行的警告字符串。在扩展需要使用中性摘要的仪表板、治理清单或 SDK 工具时使用它。

> **提示：** 将生成的摘要与已签名的人工智能清单和志愿者摘要一起包含在公投证据包中，以便政策陪审团可以验证审查小组引用的每个工件。