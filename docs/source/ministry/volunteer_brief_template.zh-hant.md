---
lang: zh-hant
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

# 志願者簡介模板 (MINFO-3a)

路線圖參考：**MINFO-3a — 平衡的簡要模板和衝突披露。 **

志願者提交的簡短意見總結了公民小組希望政府在提出黑名單變更或其他部委執法動議時審查的立場。 MINFO-3a 要求每份簡報都遵循確定性結構，以便透明度管道能夠 (1) 呈現可比較的事實表，(2) 確認利益衝突已披露，以及 (3) 自動刪除或標記偏離主題的提交內容。此頁面定義了 `cargo xtask ministry-transparency` 中附帶的工具所需的規範字段、CSV 樣式事實表佈局和審核標籤。

> **Norito 架構：** `iroha_data_model::ministry::VolunteerBriefV1` 結構（版本 `1`）現在是所有提交的權威架構。工具和門戶驗證程序在發布摘要或在小組摘要中引用它之前會調用 `VolunteerBriefV1::validate`。

## 提交有效負載結構

|部分|領域 |要求|
|---------|--------|--------------|
| **信封** | `version` (u16) |必須是 `1`。版本保護允許該部毫無歧義地發展模式。 |
| **身份與立場** | `brief_id`（字符串，每個日曆年唯一）、`proposal_id`（黑名單或政策動議的鏈接）、`language` (BCP-47)、`stance` （`support`/`oppose`/`context`），`submitted_at`（RFC3339）|所有字段均必填。 `stance` 提供儀表板並且必須匹配允許的詞彙表。 |
| **作者信息** | `author.name`、`author.organization`（可選）、`author.contact`、`author.no_conflicts_certified`（布爾）| `author.contact` 是從公共儀表板中編輯的，但存儲在原始工件中。僅當作者證明沒有披露適用時才設置 `no_conflicts_certified: true`。 |
| **總結** | `summary.title`、`summary.abstract`、`summary.requested_action` |文本概述出現在事實表旁邊。將 `summary.abstract` 限制為 ≤2000 個字符。 |
| **事實表** | `fact_table` 陣列（參見下一節）|即使是短內褲也需要。 CLI 和透明度攝取作業會拒絕沒有事實表的提交。 |
| **披露** | `disclosures` 數組或 `author.no_conflicts_certified: true` |每個披露行必須包含 `type` (`financial`、`employment`、`governance`、`family`、`other`)、`entity`、 `relationship` 和 `details`。 |
| **審核元數據** | `moderation.off_topic`（布爾）、`moderation.tags`（枚舉字符串數組）、`moderation.notes` |審稿人用來壓制 astroturfing 或不相關的提交內容。離題條目不會對儀表板做出貢獻。 |

## 事實表規範

每個 `fact_table` 行捕獲一個機器可讀的聲明。將行存儲為具有以下字段的 JSON 對象：|領域|描述 |
|--------|-------------|
| `claim_id` |穩定標識符（例如 `VB-2026-04-F1`）。 |
| `claim` |對事實或影響的單句陳述。 |
| `status` | `corroborated`、`disputed`、`context-only` 之一。 |
| `impact` |包含 `governance`、`technical`、`compliance`、`community` 中的一個或多個的數組。 |
| `citations` |非空字符串數組。接受 URL、Torii 案例 ID 或 CID 參考。 |
| `evidence_digest` |支持文檔的可選 BLAKE3 校驗和。 |

自動化注意事項：
- 攝取作業計數 `fact_rows` 和 `fact_rows_with_citation` 以構建發布記分卡。沒有引用的行仍然出現在人類可讀的表中，但被跟踪為缺失的證據。
- 保持聲明簡潔並引用治理提案中使用的相同標識符，以便交叉鏈接具有確定性。

## 衝突披露要求

1. 當存在財務、就業、治理或家庭關係時，至少提供一項披露條目。
2. 使用 `author.no_conflicts_certified: true` 斷言“無已知衝突”。提交的內容必須包含披露條目或 `true` 認證；否則，它們會在攝取過程中被標記。
3. 只要存在公共文件（例如，公司備案、DAO 投票），請包含 `disclosures[i].evidence`。對於“無”認證，證據是可選的，但強烈建議提供。

## 審核標籤和離題處理

審核者可以在進入透明度管道之前對提交內容進行標記：

- `moderation.off_topic: true` 從聚合計數中刪除該條目，同時遞增 `off_topic_rejections` 計數器。該行仍然可以在原始檔案中用於審計。
- `moderation.tags` 接受枚舉值：`duplicate`、`needs-translation`、`needs-follow-up`、`spam`、`astroturf`、`policy-escalation`。標籤可幫助下游審閱者進行分類，而無需重新閱讀完整的摘要。
- `moderation.notes` 存儲審核決定的簡短理由（≤512 個字符）。

## 提交清單

1. 使用此模板或下述幫助器 CLI 填寫 JSON 負載。
2. 填充至少一個事實表行；包括每行的引用。
3. 提供披露或明確設置`author.no_conflicts_certified: true`。
4. 附加審核元數據（默認 `off_topic: false`），以便審閱者可以快速分類。
5. 上傳前使用 `cargo xtask ministry-transparency ingest --volunteer <file>` 或任何 Norito 驗證器驗證有效負載。

## 驗證 CLI (MINFO-3)

該存儲庫現在為志願者簡介提供了專用驗證器：

```bash
cargo xtask ministry-transparency volunteer-validate \
  --input docs/examples/ministry/volunteer_brief_template.json \
  --json-output artifacts/ministry/volunteer_lint_report.json
```

關鍵行為：- 接受單獨的 JSON 對象*或*內褲數組；多次傳遞 `--input` 以在一次運行中檢查多個文件。
- 發出一個簡短的摘要，顯示錯誤和警告的數量；警告會突出顯示空的引文列表或過長的註釋，而錯誤則會阻止發表。
- 確保必填字段（`brief_id`、`proposal_id`、`stance`、事實表內容、披露或 `no_conflicts_certified`）與此模板匹配，並且枚舉值保留在記錄的詞彙表內。
- 當設置 `--json-output <path>` 時，驗證器會寫入一個機器可讀的清單，總結每個摘要（提案 ID、立場、狀態、錯誤/警告）。門戶的 `npm run generate:volunteer-lint` 命令使用此清單來顯示每個提案頁面旁邊的 lint 狀態。

將命令集成到門戶工作流程或 CI 中，以確保志願者提交的內容在到達透明度攝取作業之前符合 **MINFO-3**。

## 負載示例

請參閱 `docs/examples/ministry/volunteer_brief_template.json` 以獲取完整填充的示例，包括事實表行、披露和審核標籤。下游儀表板使用原始 JSON 並自動計算：

- `total_briefs`（不包括題外話）
- `fact_rows` / `fact_rows_with_citation`
- `disclosures_missing`
- `off_topic_rejections`

如果需要新字段，請在同一更改中更新此文檔和攝取摘要器 (`xtask/src/ministry.rs`)，以便治理證據保持可重現。

## 出版物 SLA 和門戶表面處理 (MINFO-3)

為了保持公民提交的透明度，門戶現在在通過驗證後以固定的節奏發布簡報：

1. **T+0–6 小時：** 通過志願者報名表或 `cargo xtask ministry-transparency ingest` 提交意見。驗證器運行 `VolunteerBriefV1::validate`，拒絕格式錯誤的有效負載，並發出 lint 報告（缺少披露、重複的事實 ID 等）。
2. **T+6–24 小時：** 接受的簡報將排隊等待翻譯/分類。應用審核標籤（`needs-translation`、`duplicate`、`policy-escalation`，...），偏離主題的條目將被存檔，但從聚合計數中排除。
3. **T+24–48 小時：** 門戶網站在相應的提案頁面旁邊發布簡報。現在，每個已發布的提案都鏈接到“志願者意見”，因此審閱者可以在不打開原始 JSON 的情況下閱讀支持/反對/背景摘要。

如果提交標記為 `policy-escalation` 或 `astroturf`，SLA 會收緊至 **12 小時**，以便治理可以快速響應。操作員可以通過文檔門戶 (`docs/portal/docs/ministry/volunteer-briefs.md`) 中的 **志願者簡介** 頁面審核 SLA，其中列出了最新的發布窗口、lint 狀態以及 Norito 工件的鏈接。