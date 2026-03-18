---
lang: zh-hant
direction: ltr
source: docs/source/ministry/red_team_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f4a2e50e18749f64212dee5186bfd3a3d034ac906d1a506d420cdcb1b5117517
source_last_modified: "2025-12-29T18:16:35.979926+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Red-Team Status (MINFO-9)
summary: Snapshot of the chaos drill program covering upcoming runs, last completed scenario, and remediation items.
translator: machine-google-reviewed
---

# 事工紅隊狀態

本頁是對[審核紅隊計劃](moderation_red_team_plan.md) 的補充
通過跟踪近期演練日曆、證據包和補救措施
狀態。每次運行後與捕獲的文物一起更新它
`artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`。

## 即將進行的演習

|日期 (UTC) |場景 |所有者 |證據準備|筆記|
|------------|---------|----------|----------------|--------|
| 2026-11-12 | **蒙眼行動** — Taikai 混合模式走私演習與網關降級嘗試 |安全工程（Miyu Sato），部門行動（Liam O’Connor）| `scripts/ministry/scaffold_red_team_drill.py` 捆綁包 `docs/source/ministry/reports/red_team/2026-11-operation-blindfold.md` + 暫存目錄 `artifacts/ministry/red-team/2026-11/operation-blindfold/` |練習 GAR/Taikai 重疊以及 DNS 故障轉移；需要在啟動前將 Merkle 快照列入拒絕列表，並在捕獲儀表板後運行 `export_red_team_evidence.py`。 |

## 最後一次演練快照

|日期 (UTC) |場景 |證據包|補救和跟進|
|------------|---------|-----------------|----------------------------------------|
| 2026-08-18 | **海玻璃行動** — 網關走私、治理重播和警報限電演練 | `artifacts/ministry/red-team/2026-08/operation-seaglass/`（Grafana 導出、Alertmanager 日誌、`seaglass_evidence_manifest.json`）| **開放：** 重放密封自動化（`MINFO-RT-17`，所有者：Governance Ops，截止日期為 2026 年 9 月 5 日）； pin 儀表板凍結為 SoraFS（`MINFO-RT-18`，可觀察性，2026 年 8 月 25 日到期）。 **已關閉：** 日誌模板已更新以攜帶 Norito 清單哈希值。 |

## 跟踪和工具

- 使用`scripts/ministry/moderation_payload_tool.py`封裝注射劑
  每個場景的有效負載和拒絕列表補丁。
- 通過 `scripts/ministry/export_red_team_evidence.py` 記錄儀表板/日誌捕獲
  每次演習後立即進行，以便證據清單包含簽名的哈希值。
- CI 防護 `ci/check_ministry_red_team.sh` 強制執行已提交的演練報告
  不包含佔位符文本並且引用的工件之前存在
  合併。

請參閱 `status.md`（§ *部委紅隊狀態*）以獲取引用的實時摘要
在每週的協調電話中。