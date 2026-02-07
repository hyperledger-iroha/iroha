---
lang: zh-hant
direction: ltr
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-12-29T18:16:35.079313+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS 培訓幻燈片模板

此 Markdown 大綱反映了主持人應適應的幻燈片
他們的語言群體。將這些部分複製到 Keynote/PowerPoint/Google 中
根據需要滑動並本地化要點、屏幕截圖和圖表。

## 標題幻燈片
- 計劃：“Sora 名稱服務入門”
- 字幕：指定後綴+週期（例如，`.sora — 2026‑03`）
- 演講者+隸屬關係

## KPI導向
- `docs/portal/docs/sns/kpi-dashboard.md` 的屏幕截圖或嵌入
- 解釋後綴過濾器的項目符號列表、ARPU 表、凍結跟踪器
- 用於導出 PDF/CSV 的標註

## 清單生命週期
- 圖表：註冊商 → Torii → 治理 → DNS/網關
- 參考 `docs/source/sns/registry_schema.md` 的步驟
- 帶註釋的示例清單摘錄

## 爭議和凍結演習
- 監護人干預流程圖
- 參考 `docs/source/sns/governance_playbook.md` 的清單
- 凍結工單時間線示例

## 附件捕獲
- 顯示 `cargo xtask sns-annex ... --portal-entry ...` 的命令片段
- 提醒將 Grafana JSON 歸檔到 `artifacts/sns/regulatory/<suffix>/<cycle>/` 下
- 鏈接至 `docs/source/sns/reports/.<suffix>/<cycle>.md`

## 後續步驟
- 培訓反饋鏈接（參見 `docs/examples/sns_training_eval_template.md`）
- Slack/Matrix 通道手柄
- 即將到來的里程碑日期