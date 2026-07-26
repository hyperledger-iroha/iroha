---
lang: zh-hant
direction: ltr
source: docs/source/ministry/reports/2026-Q3-template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f313d8010f2a7174c90f51dea512bcab6eb4a207df9199f28a7352944cb43c8b
source_last_modified: "2025-12-29T18:16:35.981653+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Transparency Report — 2026 Q3 (Template)
summary: Scaffold for the MINFO-8 quarterly transparency packet; replace all tokens before publication.
quarter: 2026-Q3
translator: machine-google-reviewed
---

<!--
  Usage:
    1. Copy this file when drafting a new quarter (e.g., 2026-Q3 → 2026-Q3.md).
    2. Replace every {{TOKEN}} marker and remove instructional callouts.
    3. Attach supporting artefacts (data appendix, CSVs, manifest, Grafana export) under artifacts/ministry/transparency/<YYYY-Q>/.
-->

# 執行摘要

> 提供一段關於審核准確性、申訴結果、拒絕名單流失和財務亮點的摘要。提及發布是否滿足 T+14 截止日期。

## 季度回顧

### 亮點
- {{HIGHLIGHT_1}}
- {{HIGHLIGHT_2}}
- {{HIGHLIGHT_3}}

### 風險與緩解措施

|風險|影響 |緩解措施 |業主|狀態 |
|------|--------|------------|--------|--------|
| {{RISK_1}} | {{影響}} | {{緩解}} | {{所有者}} | {{狀態}} |
| {{RISK_2}} | {{影響}} | {{緩解}} | {{所有者}} | {{狀態}} |

## 指標概述

所有指標均源自 DP sanitizer 運行後的 `ministry_transparency_builder`（Norito 捆綁包）。附上下面引用的相應 CSV 切片。

### AI 審核准確性

|型號簡介 |地區 | FP 率（目標）| FN 利率（目標）|漂移與校準|樣本量|筆記|
|--------------|--------|------------------|------------------|------------------------|----------|------|
| {{個人資料}} | {{地區}} | {{fp_rate}} ({{fp_target}}) | {{fn_rate}} ({{fn_target}}) | {{fn_rate}} {{漂移}} | {{樣本}} | {{註釋}} |

### 上訴和小組活動

|公制|價值| SLA 目標 |趨勢與 Q-1 |筆記|
|--------|--------|------------|----------------|--------|
|收到的上訴 | {{appeals_received}} | {{sla}} | {{delta}} | {{註釋}} |
|中值解決時間 | {{中位數分辨率}} | {{sla}}| {{delta}} | {{註釋}} |
|反轉率 | {{reversal_rate}} | {{目標}} | {{delta}} | {{註釋}} |
|面板利用率| {{panel_utilization}} | {{目標}} | {{delta}} | {{註釋}} |

### 拒絕名單和緊急佳能

|公制|計數|差壓噪聲 (ε) |緊急標誌| TTL 合規性 |筆記|
|--------|--------|--------------|------------------|----------------|--------------------|
|哈希添加 | {{補充}} | {{epsilon_counts}} | {{flags}} | {{ttl_status}} | {{註釋}} |
|哈希刪除 | {{刪除}} | {{epsilon_counts}} | {{flags}} | {{ttl_status}} | {{註釋}} |
|佳能調用 | {{canon_incalls}} |不適用 | {{flags}} | {{ttl_status}} | {{註釋}} |

### 國債變動

|流量|金額 (MINFO) |來源參考|筆記|
|------|----------------|--------------------|--------|
|上訴存款 | {{金額}} | {{tx_ref}} | {{註釋}} |
|小組獎勵 | {{金額}} | {{tx_ref}} | {{註釋}} |
|運營支出| {{金額}} | {{tx_ref}} | {{註釋}} |

### 志願者和外展信號

|公制|價值|目標|筆記|
|--------|--------|--------|--------|
|志願者簡介發布 | {{值}} | {{目標}} | {{註釋}} |
|涵蓋的語言 | {{值}} | {{目標}} | {{註釋}} |
|舉辦治理研討會| {{值}} | {{目標}} | {{註釋}} |

## 差異化隱私和消毒

總結消毒劑的運行情況並包括 RNG 的承諾。

- 消毒劑工作：`{{CI_JOB_URL}}`
- DP參數：ε={{epsilon_total}}，δ={{delta_total}}
- RNG承諾：`{{blake3_seed_commitment}}`
- 抑制的存儲桶：{{suppressed_buckets}}
- 質量檢查審核員：{{審核員}}

附上 `artifacts/ministry/transparency/{{Quarter}}/dp_report.json` 並記下任何手動干預。## 數據附件

|文物|路徑| SHA-256 |已上傳至 SoraFS？ |筆記|
|----------|------|---------|----------|--------|
|摘要 PDF | `artifacts/ministry/transparency/{{Quarter}}/summary.pdf` | {{哈希}} | {{是/否}} | {{註釋}} |
| Norito 數據附錄 | `artifacts/ministry/transparency/{{Quarter}}/data/appendix.norito` | {{哈希}} | {{是/否}} | {{註釋}} |
|指標 CSV 包 | `artifacts/ministry/transparency/{{Quarter}}/data/csv/` | {{哈希}} | {{是/否}} | {{註釋}} |
| Grafana 出口 | `dashboards/grafana/ministry_transparency_overview.json` | {{哈希}} | {{是/否}} | {{註釋}} |
|警報規則| `dashboards/alerts/ministry_transparency_rules.yml` | {{哈希}} | {{是/否}} | {{註釋}} |
|出處清單 | `artifacts/ministry/transparency/{{Quarter}}/manifest.json` | {{哈希}} | {{是/否}} | {{註釋}} |
|清單簽名 | `artifacts/ministry/transparency/{{Quarter}}/manifest.json.sig` | {{哈希}} | {{是/否}} | {{註釋}} |

## 出版物元數據

|領域|價值|
|--------|--------|
|發布季度 | {{季度}} |
|發佈時間戳 (UTC) | {{時間戳}} |
| SoraFS CID | `{{cid}}` |
|治理投票ID | {{vote_id}} |
|清單摘要 (`blake2b`) | `{{manifest_digest}}` |
| Git 提交/標記 | `{{git_rev}}` |
|發布所有者 | {{所有者}} |

## 批准

|角色 |名稱 |決定|時間戳|筆記|
|------|------|----------|------------|--------|
|部委可觀察性 TL | {{名稱}} | ✅/⚠️ | {{時間戳}} | {{註釋}} |
|治理委員會聯絡| {{名稱}} | ✅/⚠️ | {{時間戳}} | {{註釋}} |
|文檔/通訊主管 | {{名稱}} | ✅/⚠️ | {{時間戳}} | {{註釋}} |

## 變更日誌和後續行動

- {{CHANGELOG_ITEM_1}}
- {{CHANGELOG_ITEM_2}}

### 打開操作項

|項目 |業主|到期|狀態 |筆記|
|------|--------|-----|--------|--------|
| {{行動}} | {{所有者}} | {{到期}} | {{狀態}} | {{註釋}} |

### 聯繫方式

- 主要聯繫人：{{contact_name}} (`{{chat_handle}}`)
- 升級路徑：{{escalation_details}}
- 分發列表：{{mailing_list}}

_模板版本：2026-03-25。進行結構更改時更新修訂日期。 _