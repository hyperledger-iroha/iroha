---
lang: zh-hant
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-12-29T18:16:35.080069+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS 培訓工作簿模板

使用本工作簿作為每個培訓隊列的規範講義。更換
在分發給與會者之前添加佔位符 (`<...>`)。

## 會議詳細信息
- 後綴：`<.sora | .nexus | .dao>`
- 週期：`<YYYY-MM>`
- 語言：`<ar/es/fr/ja/pt/ru/ur>`
- 協調員：`<name>`

## 實驗 1 — KPI 導出
1. 打開門戶 KPI 儀表板 (`docs/portal/docs/sns/kpi-dashboard.md`)。
2. 按後綴 `<suffix>` 和時間範圍 `<window>` 過濾。
3. 導出 PDF + CSV 快照。
4. 在此處記錄導出的 JSON/PDF 的 SHA-256：`______________________`。

## 實驗 2 — 清單演習
1. 從 `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json` 獲取示例清單。
2. 使用 `cargo run --bin sns_manifest_check -- --input <file>` 進行驗證。
3. 使用 `scripts/sns_zonefile_skeleton.py` 生成解析器骨架。
4. 粘貼差異摘要：
   ```
   <git diff output>
   ```

## 實驗 3 — 爭議模擬
1. 使用監護人 CLI 啟動凍結（案例 ID `<case-id>`）。
2. 記錄爭議哈希：`______________________`。
3. 將證據日誌上傳至`artifacts/sns/training/<suffix>/<cycle>/logs/`。

## 實驗室 4 — 附件自動化
1. 導出 Grafana 儀表板 JSON 並將其複製到 `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json`。
2. 運行：
   ```bash
   cargo xtask sns-annex \
     --suffix <suffix> \
     --cycle <cycle> \
     --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --output docs/source/sns/reports/<suffix>/<cycle>.md \
     --regulatory-entry docs/source/sns/regulatory/<memo>.md \
     --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```
3.粘貼附件路徑+SHA-256輸出：`________________________________`。

## 反饋說明
- 什麼是不清楚的？
- 哪些實驗室隨著時間的推移而運行？
- 觀察到工具錯誤嗎？

將完成的工作簿返還給輔導員；他們屬於
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`。