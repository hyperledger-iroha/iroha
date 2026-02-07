---
lang: zh-hant
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

# 審查小組摘要 (MINFO-4a)

路線圖項目 **MINFO-4a — 中立摘要生成器** 需要一個可重複的工作流程，將已接受的議程提案、志願者簡報語料庫和經過驗證的 AI 審核清單轉換為中立的公投摘要。可交付成果必須：

- 將輸出記錄為 Norito 結構 (`ReviewPanelSummaryV1`)，以便治理可以將其與清單和選票一起存檔。
- 整理源材料，當審查小組沒有平衡的支持/反對覆蓋範圍或事實缺少引用時，會快速失敗。
- 在每個亮點中參考人工智能清單和提案證據包，確保政策陪審團在投票前看到自動化和人類背景。

## CLI 用法

該工作流程作為 `cargo xtask` 的一部分提供：

```bash
cargo xtask ministry-panel synthesize \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --volunteer docs/examples/ministry/volunteer_brief_template.json \
  --ai-manifest docs/examples/ai_moderation_calibration_manifest_202602.json \
  --panel-round RP-2026-05 \
--output artifacts/review_panel/AC-2026-001-RP-2026-05.json
```

所需輸入：

1. `--proposal` – 遵循 `AgendaProposalV1` 的 JSON 有效負載。幫助程序在生成摘要之前驗證架構。
2. `--volunteer` – `docs/source/ministry/volunteer_brief_template.md` 後面的志願者簡介的 JSON 數組。偏離主題的條目將被自動忽略。
3. `--ai-manifest` – 治理簽名的 `ModerationReproManifestV1` 描述了篩選內容的 AI 委員會。
4. `--panel-round` – 當前審核輪次的標識符 (`RP-YYYY-##`)。
5. `--output` – 目標文件或 `-` 以流式傳輸到標準輸出。使用 `--language` 覆蓋提案語言，並使用 `--generated-at` 在回填歷史記錄時提供確定性 Unix 時間戳（毫秒）。

生成獨立摘要後，運行
[`cargo xtask ministry-panel packet`](referendum_packet.md) 組裝助手
完整的公投檔案 (`ReferendumPacketV1`)。供應
`--summary-out` 到數據包命令將保留相同的摘要文件，同時
將其嵌入到下游消費者的數據包對像中。

### 通過 `ministry-transparency ingest` 實現自動化

已經為季度證據包運行 `cargo xtask ministry-transparency ingest` 的團隊現在可以將審核小組摘要拼接到同一管道中：

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

所有四個 `--panel-*` 標誌必須一起提供（並且需要 `--volunteer`）。該命令將審查小組摘要發送到 `--panel-summary-out`，將解析後的有效負載嵌入到攝取快照中，並記錄校驗和，以便下游工具可以證明證據。

## Linting 和故障模式

`cargo xtask ministry-panel synthesize` 在編寫摘要之前強制執行以下不變量：

- **平衡立場：** 必須至少提供一份支持簡報和一份反對簡報。缺少覆蓋率會終止運行並出現​​描述性錯誤。
- **引文覆蓋率：**亮點僅由包含引文的事實行生成。缺失的引文永遠不會阻止構建，但每個受影響的摘要都會在輸出中的 `warnings[]` 下列出。
- **每個突出顯示引用：** 每個突出顯示都包含對 (a) 志願者事實行、(b) AI 清單 ID 和 (c) 提案中的第一個證據附件的引用，因此數據包始終鏈接回已簽名的工件。如果任何檢查失敗，該命令將以非零狀態退出並指向有問題的記錄。成功運行會寫入與 `ReviewPanelSummaryV1` 架構匹配的 JSON 文件，並且可以嵌入到治理清單中。

## 輸出結構

`ReviewPanelSummaryV1` 存在於 `crates/iroha_data_model/src/ministry/mod.rs` 中，每個消費者都可以通過 `iroha_data_model` 箱使用。關鍵部分包括：

- `overview` – 政策陪審團數據包的標題、中性總結句和決策上下文。
- `stance_distribution` – 每個立場的摘要和事實行計數。下游儀表板在發布之前會讀取此內容以確認覆蓋範圍。
- `highlights` – 每個立場最多兩個事實摘要以及完全合格的引文。
- `ai_manifest` – 從再現性清單中提取元數據（清單 UUID、運行程序版本、閾值）。
- `volunteer_references` – 用於審核的每個簡要統計數據（語言、立場、行、引用行）。
- `warnings` – 描述跳過項目的自由格式 lint 消息（例如，缺少引用的事實行）。

## 示例

`docs/examples/ministry/review_panel_summary_example.json` 包含使用幫助程序生成的完整示例。它展示了平衡的支持/反對覆蓋率、引文連接、清單參考文獻以及無法提升為亮點的事實行的警告字符串。在擴展需要使用中性摘要的儀表板、治理清單或 SDK 工具時使用它。

> **提示：** 將生成的摘要與已簽名的人工智能清單和志願者摘要一起包含在公投證據包中，以便政策陪審團可以驗證審查小組引用的每個工件。