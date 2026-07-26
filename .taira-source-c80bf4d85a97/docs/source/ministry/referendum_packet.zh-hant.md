---
lang: zh-hant
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

# 公投數據包工作流程 (MINFO-4)

路線圖項目 **MINFO-4 — 審查面板和公投合成器**現已推出
由新的 `ReferendumPacketV1` Norito 模式加上 CLI 幫助程序實現
如下所述。工作流程捆綁了政策陪審團所需的所有工件
投票形成單個 JSON 文檔，以便治理、審計和透明度
門戶網站可以確定性地重放證據。

## 輸入

1. **議程提案** — `cargo xtask ministry-agenda validate` 使用相同的 JSON。
2. **志願者簡介** — 通過 linting 後生成的精選數據集
   `cargo xtask ministry-transparency volunteer-validate`。
3. **AI 審核清單** — 治理簽名的 `ModerationReproManifestV1`。
4. **分類摘要** — 由以下人員發出的確定性人工製品
   `cargo xtask ministry-agenda sortition`。 JSON如下
   [`PolicyJurySortitionV1`](./policy_jury_ballots.md) 因此治理可以
   重現 POP 快照摘要和等待列表/故障轉移接線。
5. **影響報告** — 哈希系列/報告通過生成
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

`packet` 子命令運行中性摘要合成器 (MINFO-4a)，重用
現有的志願者固定裝置，並通過以下方式豐富產出：

- `ReferendumSortitionEvidence` — 算法、種子和名冊摘要
  抽籤神器。
- `ReferendumPanelist[]` — 每個選定的理事會成員加上 Merkle 證明
  需要審核他們的抽籤。
- `ReferendumImpactSummary` — 每個哈希系列的總數和衝突列表
  影響報告。

當您仍需要獨立的 `ReviewPanelSummaryV1` 時，請使用 `--summary-out`
文件；否則，數據包將在 `review_summary` 下嵌入摘要。

## 輸出結構

`ReferendumPacketV1` 居住在
`crates/iroha_data_model/src/ministry/mod.rs`，可跨 SDK 使用。
關鍵部分包括：

- `proposal` — 原始 `AgendaProposalV1` 對象。
- `review_summary` — MINFO-4a 發出的平衡摘要。
- `sortition` / `panelists` — 就座理事會的可複制證明。
- `impact_summary` — 每個哈希族的重複/策略衝突證據。

有關完整示例，請參閱 `docs/examples/ministry/referendum_packet_example.json`。
將生成的數據包與簽名的 AI 一起附加到每個公投檔案中
亮點部分引用的明顯和透明的人工製品。