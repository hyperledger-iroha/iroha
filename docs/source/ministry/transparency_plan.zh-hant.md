---
lang: zh-hant
direction: ltr
source: docs/source/ministry/transparency_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2639f4f7692e13ed61cc6246c87b047dc7415a6c9243ca7c046e6ccea8b55e9a
source_last_modified: "2025-12-29T18:16:35.983537+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Transparency & Audit Plan
summary: Implementation plan for roadmap item MINFO-8 covering quarterly transparency reports, privacy guardrails, dashboards, and automation.
translator: machine-google-reviewed
---

# 透明度和審計報告 (MINFO-8)

路線圖參考：**MINFO-8 — 透明度和審計報告** 和 **MINFO-8a — 隱私保護髮布流程**

信息部必鬚髮布確定性透明度製品，以便社區能夠審核審核效率、申訴處理和黑名單流失情況。本文檔定義了在 2026 年第 3 季度目標之前關閉 MINFO-8 所需的範圍、工件、隱私控制和操作工作流程。

## 目標和可交付成果

- 製作季度透明度數據包，總結 AI 審核准確性、上訴結果、拒絕名單流失、志願者小組活動以及與 MINFO 預算相關的財務變動。
- 發送隨附的原始數據包 (Norito JSON + CSV) 以及儀表板，以便公民無需等待靜態 PDF 即可對指標進行切片。
- 在發布任何數據集之前強制執行隱私保證（差異隱私+最小計數規則）和簽名證明。
- 在治理 DAG 和 SoraFS 中記錄每個出版物，以便歷史文物保持不可變且可獨立驗證。

### 人工製品矩陣

|文物 |描述 |格式|存儲|
|----------|-------------|--------|---------|
|透明度摘要|人類可讀的報告，包含執行摘要、要點、風險項目 |降價 → PDF | `docs/source/ministry/reports/<YYYY-Q>.md` → `artifacts/ministry/transparency/<YYYY-Q>/summary.pdf` |
|數據附錄| Canonical Norito 捆綁包，包含已清理的表（`ModerationLedgerBlockV1`、上訴、黑名單增量）| `.norito` + `.json` | `artifacts/ministry/transparency/<YYYY-Q>/data`（鏡像到 SoraFS CID）|
|指標 CSV |儀表板的便捷 CSV 導出（AI FP/FN、上訴 SLA、拒絕名單流失）| `.csv` |相同的目錄，散列和簽名 |
|儀表板快照 | `ministry_transparency_overview` 面板的 Grafana JSON 導出 + 警報規則 | `.json` | `dashboards/grafana/ministry_transparency_overview.json` / `dashboards/alerts/ministry_transparency_rules.yml` |
|出處清單 | Norito 清單綁定摘要、SoraFS CID、簽名、發佈時間戳 | `.json` + 獨立簽名 | `artifacts/ministry/transparency/<YYYY-Q>/manifest.json(.sig)`（還附有治理投票）|

## 數據源和管道

|來源 |飼料|筆記|
|--------|------|--------|
|審核分類賬 (`docs/source/sorafs_transparency_plan.md`) |每小時 `ModerationLedgerBlockV1` 導出存儲在 CAR 文件中 | SFM-4c 已經上線；重新用於季度匯總。 |
| AI標定+誤報率| `docs/source/sorafs_ai_moderation_plan.md` 夾具 + 校準清單 (`docs/examples/ai_moderation_calibration_*.json`) |按策略、區域和模型配置文件聚合的指標。 |
|上訴登記冊 | MINFO-7 金庫工具發出的 Norito `AppealCaseV1` 事件 |包含權益轉讓、小組名冊、SLA 計時器。 |
|拒絕者流失 |來自 Merkle 註冊表 (MINFO-6) 的 `MinistryDenylistChangeV1` 事件 |包括哈希族、TTL、緊急規範標誌。 |
|國庫流動| `MinistryTreasuryTransferV1` 事件（申訴存款、小組獎勵）|與 `finance/mminfo_gl.csv` 平衡。 |緊急規範治理、TTL 限制和審查要求現已生效
[`docs/source/ministry/emergency_canon_policy.md`](emergency_canon_policy.md)，確保
流失指標捕獲層（`standard`、`emergency`、`permanent`）、canon id、
並審查 Torii 在加載時強制執行的截止日期。

處理階段：
1. **將**原始事件攝取到 `ministry_transparency_ingest`（Rust 服務鏡像透明賬本攝取器）。每晚運行，冪等。
2. **每季度總計** `ministry_transparency_builder`。在隱私過濾器之前輸出 Norito 數據附錄以及每個指標的表。
3. 通過 `cargo xtask ministry-transparency sanitize`（或 `scripts/ministry/dp_sanitizer.py`）**清理**指標，並發出帶有元數據的 CSV/JSON 切片。
4. **打包**工件，使用 `ministry_release_signer` 對其進行簽名，並上傳到 SoraFS + 治理 DAG。

## 2026 年第 3 季度參考版本

- 首個治理門控捆綁包（2026-Q3）於 2026-10-07 通過 `make check-ministry-transparency` 生成。工件位於 `artifacts/ministry/transparency/2026-Q3/`（包括 `sanitized_metrics.json`、`dp_report.json`、`summary.md`、`checksums.sha256`、`transparency_manifest.json` 和 `transparency_release_action.json`）中，並被鏡像到SoraFS CID `7f4c2d81a6b13579ccddeeff00112233`。
- `docs/source/ministry/reports/2026-Q3.md` 中捕獲了出版物詳細信息、指標表和批准​​，該文件現在作為審核員審查第三季度窗口的規範參考。
- CI 在版本離開暫存之前應用 `ci/check_ministry_transparency.sh` / `make check-ministry-transparency`，驗證工件摘要、Grafana/警報哈希和清單元數據，以便未來每個季度都遵循相同的證據線索。

## 指標和儀表板

Grafana 儀表板 (`dashboards/grafana/ministry_transparency_overview.json`) 顯示以下面板：

- AI 調節精度：每個模型的 FP/FN 速率、漂移與校準目標以及與 `docs/source/sorafs/reports/ai_moderation_calibration_*.md` 相關的警報閾值。
- 申訴生命週期：提交、SLA 合規性、撤銷、債券銷毀、每層積壓。
- 拒絕列表流失：每個哈希系列的添加/刪除、TTL 過期、緊急規範調用。
- 志願者簡介和小組多樣性：每種語言的提交內容、利益衝突披露、發布滯後。 `docs/source/ministry/volunteer_brief_template.md` 中指定了平衡的簡要字段，確保事實表和審核標籤是機器可讀的。
- 國庫餘額：存款、支出、未償債務（提供給 MINFO-7）。

警報規則（編入 `dashboards/alerts/ministry_transparency_rules.yml`）涵蓋：
- FP/FN 相對於校準基線的偏差 >25%。
- 上訴 SLA 錯過率每季度 >5%。
- 緊急規範 TTL 早於政策。
- 發布滯後於季度結束後 >14 天。

## 隱私和發布護欄 (MINFO-8a)|公制類別 |機制|參數|額外的警衛|
|--------------|-----------|------------|--------------------|
|計數（上訴、黑名單變更、志願者簡介）|拉普拉斯噪聲 | ε=0.75 每季度，δ=1e-6 |抑制後置噪聲值<5的桶；每季度每個演員的剪輯貢獻為 1。 |
|人工智能準確度 |分子/分母上的高斯噪聲 | ε=0.5，δ=1e-6 |僅當淨化樣本數≥50（`min_accuracy_samples` 下限）時才發布並發佈置信區間。 |
|國庫流動|無噪音（已在鏈上公開）| — |屏蔽賬戶名稱（資金庫 ID 除外）；包括默克爾證明。 |

發布要求：
- 差異化隱私報告包括 epsilon/delta 賬本和 RNG 種子承諾 (`blake3(seed)`)。
- 敏感示例（證據哈希）已被編輯，除非已在公開 Merkle 收據中。
- 修訂日誌附加到描述所有已刪除字段和理由的摘要中。

## 發布工作流程和時間表

| T 型窗 |任務|所有者 |證據|
|----------|------|----------|----------|
|季度收盤後 T+3d |凍結原始導出，觸發聚合作業 |部委可觀察性 TL | `ministry_transparency_ingest.log`，管道作業 ID |
| T+7 天 |查看原始指標，進行 DP 消毒劑試運行 |數據信任團隊 |消毒劑報告 (`artifacts/.../dp_report.json`) |
| T+10 天 |摘要草案+數據附錄|文檔/DevRel + 政策分析師 | `docs/source/ministry/reports/<YYYY-Q>.md` |
| T+12 天 |簽署文物，製作清單，上傳至 SoraFS |運營/治理秘書處| `manifest.json(.sig)`、SoraFS CID |
| T+14 天 |發布儀表板+警報，發布治理公告|可觀察性 + 通訊 | Grafana 導出、警報規則哈希、治理投票鏈接 |

每個版本必須經過以下人員的批准：
1. 部可觀測性TL（數據完整性）
2. 治理委員會聯絡（政策）
3. 文檔/通訊主管（公開措辭）

## 自動化和證據存儲- 使用 `cargo xtask ministry-transparency ingest` 從原始源（分類賬、上訴、拒絕名單、財務、志願者）構建季度快照。跟進 `cargo xtask ministry-transparency build`，在發布之前發出儀表板指標 JSON 以及簽名的清單。
- 紅隊鏈接：將一個或多個 `--red-team-report docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` 文件傳遞​​到攝取步驟，以便透明度快照和清理後的指標攜帶鑽探 ID、場景類、證據包路徑和儀表板 SHA 以及分類帳/上訴/拒絕列表數據。這使得 MINFO-9 演練節奏反映在每個透明數據包中，無需手動編輯。
- 志願者提交的材料必須遵循 `docs/source/ministry/volunteer_brief_template.md`（例如：`docs/examples/ministry/volunteer_brief_template.json`）。攝取步驟需要這些對象的 JSON 數組，自動過濾 `moderation.off_topic` 條目，強制披露證明，並記錄事實表覆蓋範圍，以便儀表板可以突出顯示缺失的引用。
- 額外的自動化位於 `scripts/ministry/` 下。 `dp_sanitizer.py` 包裝 `cargo xtask ministry-transparency sanitize` 命令，而 `transparency_release.py`（與出處工具一起添加）現在打包工件，從 `sorafs_cli car pack|proof verify` 摘要（或顯式 `--sorafs-cid`）派生 SoraFS CID，並寫入`transparency_manifest.json` 和 `transparency_release_action.json`（捕獲清單摘要、SoraFS CID 和儀表板 git SHA 的 `TransparencyReleaseV1` 治理負載）。將 `--governance-dir <path>` 傳遞到 `transparency_release.py`（或運行 `cargo xtask ministry-transparency anchor --action artifacts/.../transparency_release_action.json --governance-dir <path>`）以對 Norito 負載進行編碼，並在發布之前將其（加上 JSON 摘要）放入治理 DAG 目錄中。同一標誌還在 `<governance-dir>/publisher/head_requests/ministry_transparency/` 下發出 `MinistryTransparencyHeadUpdateV1` 請求，引用季度、SoraFS CID、清單路徑和 IPNS 密鑰別名（可通過 `--ipns-key` 覆蓋）。提供 `--auto-head-update` 以通過 `publisher_head_updater.py` 立即處理該請求，當需要同時發布 IPNS 時，可以選擇傳遞 `--head-update-ipns-template '/usr/local/bin/ipfs name publish --key {ipns_key} /ipfs/{cid}'`。否則，稍後運行 `scripts/ministry/publisher_head_updater.py --governance-dir <path>`（如果需要，使用相同的模板）以排空隊列，追加 `publisher/head_updates.log`，更新 `publisher/ipns_heads/<key>.json`，並將處理後的 JSON 存檔在 `head_requests/ministry_transparency/processed/` 下。
- 工件存儲在 `artifacts/ministry/transparency/<YYYY-Q>/` 下，帶有由發布密鑰簽名的 `checksums.sha256` 文件。該樹現在在 `artifacts/ministry/transparency/2026-Q3/` 上帶有參考包（經過清理的指標、DP 報告、摘要、清單、治理操作），以便工程師可以離線測試工具，`scripts/ministry/check_transparency_release.py` 在本地驗證摘要/季度元數據，而 `ci/check_ministry_transparency.sh` 在上傳證據之前在 CI 中運行相同的驗證。檢查器現在強制執行記錄的 DP 預算（對於計數，ε≤0.75，對於精度，ε≤0.5，δ≤1e−6），並且每當 `min_accuracy_samples` 或抑制閾值漂移時，或者當存儲桶洩漏低於這些下限的值而不被抑制時，構建都會失敗。將腳本視為路線圖 (MINFO-8) 和 CI 之間的契約：如果隱私參數發生變化，請一起調整上表和檢查器。- 治理錨點：創建引用清單摘要、SoraFS CID 和儀表板 git SHA 的 `TransparencyReleaseV1` 操作（`iroha_data_model::ministry::TransparencyReleaseV1` 定義規範有效負載）。

## 未完成的任務和後續步驟

|任務|狀態 |筆記|
|------|--------|--------|
|實施 `ministry_transparency_ingest` + 構建器作業 | 🈺 進行中 | `cargo xtask ministry-transparency ingest|build` 現在縫合賬本/上訴/拒絕名單/財務提要；剩下的工作連接 DP sanitizer + 發布腳本管道。 |
|發布 Grafana 儀表板 + 警報包 | 🈴已完成 |儀表板 + 警報文件位於 `dashboards/grafana/ministry_transparency_overview.json` 和 `dashboards/alerts/ministry_transparency_rules.yml` 下；在推出期間將它們連接到 PagerDuty `ministry-transparency`。 |
|自動化 DP 消毒劑 + 來源清單 | 🈴已完成 | `cargo xtask ministry-transparency sanitize`（包裝器：`scripts/ministry/dp_sanitizer.py`）發出經過清理的指標 + DP 報告，`scripts/ministry/transparency_release.py` 現在寫入 `checksums.sha256` 加上 `transparency_manifest.json` 以獲取來源。 |
|創建季度報告模板(`reports/<YYYY-Q>.md`) | 🈴已完成 |模板添加於 `docs/source/ministry/reports/2026-Q3-template.md`；每季度複製/重命名並在發布前替換 `{{...}}` 令牌。 |
|線治理 DAG 錨定 | 🈴已完成 | `TransparencyReleaseV1` 位於 `iroha_data_model::ministry` 中，`scripts/ministry/transparency_release.py` 發出 JSON 有效負載，`cargo xtask ministry-transparency anchor` 將 `.to` 工件編碼到配置的治理 DAG 目錄中，以便發布者可以自動獲取版本。 |

交付文檔、儀表板規範和工作流程將 MINFO-8 從 🈳 移動到 🈺。剩餘的工程任務（作業、腳本、警報接線）在上表中進行跟踪，並應在第一個 Q32026 發布之前完成。