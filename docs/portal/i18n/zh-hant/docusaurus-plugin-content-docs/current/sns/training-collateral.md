---
id: training-collateral
lang: zh-hant
direction: ltr
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS Training Collateral
description: Curriculum, localization workflow, and annex evidence capture required by SN-8.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> 鏡子 `docs/source/sns/training_collateral.md`。簡報時使用此頁面
> 每個後綴發布之前的註冊商、DNS、監護人和財務團隊。

## 1. 課程概覽

|軌道 |目標 |預讀|
|--------|------------|------------|
|註冊商操作 |提交清單、監控 KPI 儀表板、升級錯誤。 | `sns/onboarding-kit`、`sns/kpi-dashboard`。 |
| DNS 和網關 |應用解析器骨架，排練凍結/回滾。 | `sorafs/gateway-dns-runbook`，直接模式策略示例。 |
|監護人和理事會|執行爭議、更新治理附錄、記錄附件。 | `sns/governance-playbook`，管家記分卡。 |
|金融與分析 |捕獲 ARPU/批量指標，發布附件包。 | `finance/settlement-iso-mapping`，KPI 儀表板 JSON。 |

### 模塊流程

1. **M1 — KPI 定位（30 分鐘）：** 步行後綴過濾器、出口和逃犯
   凍結計數器。可交付成果：帶有 SHA-256 摘要的 PDF/CSV 快照。
2. **M2 — 清單生命週期（45 分鐘）：** 構建並驗證註冊商清單，
   通過 `scripts/sns_zonefile_skeleton.py` 生成解析器骨架。可交付成果：
   git diff 顯示骨架 + GAR 證據。
3. **M3——糾紛演練（40分鐘）：** 模擬監護人凍結+申訴、抓捕
   監護人 CLI 日誌位於 `artifacts/sns/training/<suffix>/<cycle>/logs/` 下方。
4. **M4 — 附件捕獲（25 分鐘）：** 導出儀表板 JSON 並運行：

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

   可交付成果：更新附件 Markdown + 監管 + 門戶備忘錄塊。

## 2. 本地化工作流程

- 語言：`ar`、`es`、`fr`、`ja`、`pt`、`ru`、`ur`。
- 每個翻譯都位於源文件旁邊
  （`docs/source/sns/training_collateral.<lang>.md`）。更新 `status` +
  刷新後為`translation_last_reviewed`。
- 每種語言的資產屬於
  `artifacts/sns/training/<suffix>/<lang>/<cycle>/`（幻燈片/、工作簿/、
  錄音/、日誌/)。
- 編輯英文後運行`python3 scripts/sync_docs_i18n.py --lang <code>`
  源以便翻譯者看到新的哈希值。

### 交貨清單

1. 本地化後更新翻譯存根 (`status: complete`)。
2. 將幻燈片導出為 PDF 並上傳到每種語言的 `slides/` 目錄。
3. 記錄≤10分鐘的KPI演練；來自語言存根的鏈接。
4. 標記為 `sns-training` 的文件治理票證，包含幻燈片/工作簿
   摘要、記錄鏈接和附件證據。

## 3. 培訓資產

- 幻燈片大綱：`docs/examples/sns_training_template.md`。
- 工作簿模板：`docs/examples/sns_training_workbook.md`（每個與會者一份）。
- 邀請+提醒：`docs/examples/sns_training_invite_email.md`。
- 評估表：`docs/examples/sns_training_eval_template.md`（回复
  存檔於 `artifacts/sns/training/<suffix>/<cycle>/feedback/` 下）。

## 4. 調度和指標

|循環|窗口|指標|筆記|
|--------|--------|---------|--------|
| 2026-03 | KPI 審核後 |出席率%，附件摘要已記錄| `.sora` + `.nexus` 隊列 |
| 2026-06 |預 `.dao` GA |財務準備度≥90% |包括政策更新|
| 2026-09 |擴展|爭議演練<20分鐘，附件SLA≤2天 |與 SN-7 激勵措施保持一致 |

在 `docs/source/sns/reports/sns_training_feedback.md` 中捕獲匿名反饋
因此後續團隊可以改進本地化和實驗室。