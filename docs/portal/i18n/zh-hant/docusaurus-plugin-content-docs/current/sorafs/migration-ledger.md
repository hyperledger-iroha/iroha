---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: "SoraFS Migration Ledger"
description: "Canonical change log tracking every migration milestone, owners, and required follow-ups."
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> 改編自 [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md)。

# SoraFS 遷移賬本

該分類帳鏡像了 SoraFS 中捕獲的遷移更改日誌
架構 RFC。條目按里程碑分組並列出有效的
窗口、受影響的團隊以及所需的行動。遷移計劃的更新
必須修改此頁面和 RFC (`docs/source/sorafs_architecture_rfc.md`)
使下游消費者保持一致。

|里程碑|有效窗口|變更摘要|受影響的團隊 |行動項目|狀態 |
|------------|------------------|----------------|----------------|------------------------|--------|
| M1 |第 7-12 週 | CI 強制執行確定性固定裝置；暫存中可用的別名證明；工具公開了明確的期望標誌。 |文檔、存儲、治理 |確保裝置保持簽名，在暫存註冊表中註冊別名，通過 `--car-digest/--root-cid` 強制更新發布清單。 | ⏳ 待定 |

引用這些里程碑的治理控制平面會議記錄如下
`docs/source/sorafs/`。團隊應在每行下方添加註明日期的要點
當發生值得注意的事件時（例如，新的別名註冊、註冊表事件
回顧）以提供可審計的書面記錄。

## 最近更新

- 2025-11-01 — 向治理委員會分發 `migration_roadmap.md` 並
  供審查的操作員名單；等待下屆理事會會議批准
  （參考：`docs/source/sorafs/council_minutes_2025-10-29.md` 後續）。
- 2025 年 11 月 2 日 — Pin 註冊表寄存器 ISI 現在強制執行共享分塊器/策略
  通過 `sorafs_manifest` 助手進行驗證，保持鏈上路徑對齊
  帶有 Torii 支票。
- 2026-02-13 — 將提供商廣告推出階段 (R0–R3) 添加到分類賬中，並
  發布了相關的儀表板和操作員指南
  （`provider_advert_rollout.md`、`grafana_sorafs_admission.json`）。