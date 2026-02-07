---
lang: zh-hant
direction: ltr
source: docs/source/ministry/reports/2026-08-mod-red-team-operation-seaglass.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 64cd1112df2f1fc95571ee4ed269e64bde6bf73bd94b19bbf0eaa80a5b43c219
source_last_modified: "2025-12-29T18:16:35.981105+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill — Operation SeaGlass
summary: Evidence and remediation log for the Operation SeaGlass moderation drill (gateway smuggling, governance replay, alert brownout).
translator: machine-google-reviewed
---

# 紅隊演習 — 海玻璃行動

- **鑽頭 ID：** `20260818-operation-seaglass`
- **日期和窗口：** `2026-08-18 09:00Z – 11:00Z`
- **場景類別：** `smuggling`
- **運營商：** `Miyu Sato, Liam O'Connor`
- **儀表板因提交而凍結：** `364f9573b`
- **證據包：** `artifacts/ministry/red-team/2026-08/operation-seaglass/`
- **SoraFS CID（可選）：** `not pinned (local bundle only)`
- **相關路線圖項目：** `MINFO-9`，以及鏈接的後續項目 `MINFO-RT-17` / `MINFO-RT-18`。

## 1. 目標和進入條件

- **主要目標**
  - 在減載警報期間嘗試走私時驗證拒絕名單 TTL 實施和網關隔離。
  - 在審核操作手冊中確認治理重放檢測和警報限電處理。
- **先決條件已確認**
  - `emergency_canon_policy.md` 版本 `v2026-08-seaglass`。
  - `dashboards/grafana/ministry_moderation_overview.json` 摘要 `sha256:ef5210b5b08d219242119ec4ceb61cb68ee4e42ce2eea8a67991fbff95501cc8`。
  - 覆蓋待命權限：`Kenji Ito (GovOps pager)`。

## 2. 執行時間線

|時間戳 (UTC) |演員 |行動/命令|結果/註釋|
|----------------|--------------------|--------------------------------|----------------|
| 09:00:12 | 09:00:12佐藤美優 |通過 `scripts/ministry/export_red_team_evidence.py --freeze-only` 在 `364f9573b` 凍結儀表板/警報 |基線捕獲並存儲在 `dashboards/` | 下
| 09:07:44 | 09:07:44利亞姆·奧康納 | 利亞姆·奧康納已發布拒絕名單快照 + GAR 覆蓋到 `sorafs_cli ... gateway update-denylist --policy-tier emergency` 的暫存 |快照已接受；覆蓋Alertmanager中記錄的窗口|
| 09:17:03 | 09:17:03佐藤美優 |使用 `moderation_payload_tool.py --scenario seaglass` 注入走私負載 + 治理重放 | 3 分 12 秒後發出警報；治理重播已標記 |
| 09:31:47 | 09:31:47利亞姆·奧康納 | 利亞姆·奧康納然證據出口和密封艙單 `seaglass_evidence_manifest.json` |證據包以及存儲在 `manifests/` 下的哈希值 |

## 3. 觀察和指標

|公制|目標|觀察|通過/失敗 |筆記|
|--------|--------|----------|------------|--------|
|警報響應延遲| = 0.98 | 0.992 | 0.992 ✅ |檢測到走私和重放負載 |
|網關異常檢測|警報已發出 |警報解除+自動隔離| ✅ |在重試預算耗盡之前應用隔離 |

- `Grafana export:` `artifacts/ministry/red-team/2026-08/operation-seaglass/dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/ministry/red-team/2026-08/operation-seaglass/alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `artifacts/ministry/red-team/2026-08/operation-seaglass/manifests/seaglass_evidence_manifest.json`

## 4. 調查結果與補救措施

|嚴重性 |尋找|業主|目標日期 |狀態/鏈接|
|----------|---------|--------|-------------|----------------|
|高|治理重放警報已觸發，但觸發等待列表故障轉移時，SoraFS 密封延遲了 2m |治理行動（利亞姆·奧康納）| 2026-09-05 | `MINFO-RT-17` open — 將重放密封自動化添加到故障轉移路徑 |
|中等|儀表板凍結未固定到 SoraFS；運營商依賴本地捆綁|可觀察性（佐藤美悠）| 2026-08-25 | `MINFO-RT-18` 打開 — 引腳 `dashboards/*` 至 SoraFS，在下一次鑽孔之前帶有簽名的 CID |
|低| CLI 日誌在第一遍中省略了 Norito 清單哈希 |部門行動（伊藤賢二）| 2026-08-22 |鑽孔時固定；日誌中更新模板 |記錄校準清單、拒絕名單策略或 SDK/工具必須如何更改。鏈接到 GitHub/Jira 問題並記下阻止/未阻止狀態。

## 5. 治理和批准

- **事件指揮官簽字：** `Miyu Sato @ 2026-08-18T11:22Z`
- **管理委員會審查日期：** `GovOps-2026-08-22`
- **後續清單：** `[x] status.md updated`、`[x] roadmap row updated`、`[x] transparency packet annotated`

## 6. 附件

- `[x] CLI logbook (logs/operation_seaglass.log)`
- `[x] Dashboard JSON export`
- `[x] Alertmanager history`
- `[x] SoraFS manifest / CAR`
- `[ ] Override audit log`

一旦上傳到證據包和 SoraFS 快照，就用 `[x]` 標記每個附件。

---

_最後更新：2026-08-18_