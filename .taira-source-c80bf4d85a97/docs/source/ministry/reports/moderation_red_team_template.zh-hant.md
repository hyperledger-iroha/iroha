---
lang: zh-hant
direction: ltr
source: docs/source/ministry/reports/moderation_red_team_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22bfdf5696bf3a58e7899e7d7b2ba77e404a05fa81304f12d6c78eeb1e8035e5
source_last_modified: "2025-12-29T18:16:35.982646+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill Report Template
summary: Copy this file for every MINFO-9 drill to capture metadata, evidence, and remediation actions.
translator: machine-google-reviewed
---

> **如何使用：** 每次練習後立即將此模板複製到 `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md`。保持文件名小寫、連字符並與 Alertmanager 中記錄的鑽取 ID 對齊。

# 紅隊演習報告 — `<SCENARIO NAME>`

- **鑽頭 ID：** `<YYYYMMDD>-<scenario>`
- **日期和窗口：** `<YYYY-MM-DD HH:MMZ – HH:MMZ>`
- **場景類別：** `smuggling | bribery | gateway | ...`
- **運營商：** `<names / handles>`
- **儀表板因提交而凍結：** `<git SHA>`
- **證據包：** `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`
- **SoraFS CID（可選）：** `<cid>`  
- **相關路線圖項目：** `MINFO-9`，以及任何鏈接的票證。

## 1. 目標和進入條件

- **主要目標**
  - `<e.g. Verify denylist TTL enforcement under smuggling attack>`
- **先決條件已確認**
  - `emergency_canon_policy.md` 版本 `<tag>`
  - `dashboards/grafana/ministry_moderation_overview.json` 摘要 `<sha256>`
  - 覆蓋權限待命：`<name>`

## 2. 執行時間線

|時間戳 (UTC) |演員 |行動/命令|結果/註釋|
|----------------|--------------------|--------------------------------|----------------|
|  |  |  |  |

> 包括 Torii 請求 ID、塊哈希、覆蓋批准和 Alertmanager 鏈接。

## 3. 觀察和指標

|公制|目標|觀察|通過/失敗 |筆記|
|--------|--------|----------|------------|--------|
|警報響應延遲| `<X> min` | `<Y> min` | ✅/⚠️ |  |
|審核檢測率 | `>= <value>` |  |  |  |
|網關異常檢測| `Alert fired` |  |  |  |

- `Grafana export:` `artifacts/.../dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/.../alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `<path>`

## 4. 調查結果與補救措施

|嚴重性 |尋找|業主|目標日期 |狀態/鏈接|
|----------|---------|--------|-------------|----------------|
|高|  |  |  |  |

記錄校準清單、拒絕名單策略或 SDK/工具必須如何更改。鏈接到 GitHub/Jira 問題並記下阻止/未阻止狀態。

## 5. 治理和批准

- **事件指揮官簽字：** `<name / timestamp>`
- **管理委員會審查日期：** `<meeting id>`
- **後續檢查清單：** `[ ] status.md updated`、`[ ] roadmap row updated`、`[ ] transparency packet annotated`

## 6. 附件

- `[ ] CLI logbook (`logs/.md`)`
- `[ ] Dashboard JSON export`
- `[ ] Alertmanager history`
- `[ ] SoraFS manifest / CAR`
- `[ ] Override audit log`

一旦上傳到證據包和 SoraFS 快照，就用 `[x]` 標記每個附件。

---

_最後更新：{{ 日期 |默認（“2026-02-20”）}}_