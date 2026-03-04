---
lang: zh-hans
direction: ltr
source: docs/source/ministry/referendum_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 922d972376b67a2f8c0c03ded95db6576e16e229e4bcb62d920b0ffda49c93ac
source_last_modified: "2025-12-29T18:16:35.980526+00:00"
translation_last_reviewed: 2026-02-07
title: Referendum Packet Workflow (MINFO-4)
summary: Produce the complete referendum dossier (`ReferendumPacketV1`) combining the proposal, neutral summary, sortition artefacts, and impact report.
translator: machine-google-reviewed
---

# 公投数据包工作流程 (MINFO-4)

路线图项目 **MINFO-4 — 审查面板和公投合成器**现已推出
由新的 `ReferendumPacketV1` Norito 模式加上 CLI 帮助程序实现
如下所述。工作流程捆绑了政策陪审团所需的所有工件
投票形成单个 JSON 文档，以便治理、审计和透明度
门户网站可以确定性地重放证据。

## 输入

1. **议程提案** — `cargo xtask ministry-agenda validate` 使用相同的 JSON。
2. **志愿者简介** — 通过 linting 后生成的精选数据集
   `cargo xtask ministry-transparency volunteer-validate`。
3. **AI 审核清单** — 治理签名的 `ModerationReproManifestV1`。
4. **分类摘要** — 由以下人员发出的确定性人工制品
   `cargo xtask ministry-agenda sortition`。 JSON如下
   [`PolicyJurySortitionV1`](./policy_jury_ballots.md) 因此治理可以
   重现 POP 快照摘要和等待列表/故障转移接线。
5. **影响报告** — 哈希系列/报告通过生成
   `cargo xtask ministry-agenda impact`。

## CLI 用法

```bash
cargo xtask ministry-panel packet \
  --proposal artifacts/ministry/proposals/AC-2026-001.json \
  --volunteer artifacts/ministry/volunteer_briefs.json \
  --ai-manifest artifacts/ministry/ai_manifest.json \
  --panel-round RP-2026-05 \
  --sortition artifacts/ministry/agenda_sortition_2026Q1.json \
  --impact artifacts/ministry/impact/AC-2026-001.json \
  --summary-out artifacts/ministry/review_panel_summary.json \
  --output artifacts/ministry/referendum_packets/AC-2026-001.json
```

`packet` 子命令运行中性摘要合成器 (MINFO-4a)，重用
现有的志愿者固定装置，并通过以下方式丰富产出：

- `ReferendumSortitionEvidence` — 算法、种子和名册摘要
  抽签神器。
- `ReferendumPanelist[]` — 每个选定的理事会成员加上 Merkle 证明
  需要审核他们的抽签。
- `ReferendumImpactSummary` — 每个哈希系列的总数和冲突列表
  影响报告。

当您仍需要独立的 `ReviewPanelSummaryV1` 时，请使用 `--summary-out`
文件；否则，数据包将在 `review_summary` 下嵌入摘要。

## 输出结构

`ReferendumPacketV1` 居住在
`crates/iroha_data_model/src/ministry/mod.rs`，可跨 SDK 使用。
关键部分包括：

- `proposal` — 原始 `AgendaProposalV1` 对象。
- `review_summary` — MINFO-4a 发出的平衡摘要。
- `sortition` / `panelists` — 就座理事会的可复制证明。
- `impact_summary` — 每个哈希族的重复/策略冲突证据。

有关完整示例，请参阅 `docs/examples/ministry/referendum_packet_example.json`。
将生成的数据包与签名的 AI 一起附加到每个公投档案中
亮点部分引用的明显和透明的人工制品。