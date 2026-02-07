---
lang: zh-hant
direction: ltr
source: docs/source/docs_devrel/monthly_sync_agenda.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2f89131efc0c79ddf63d71a25c04029014ba58393fb6336e676181322bc5066
source_last_modified: "2025-12-29T18:16:35.952009+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Docs/DevRel 每月同步議程

該議程正式確定了每月 Docs/DevRel 同步，該同步被引用
`roadmap.md`（請參閱“將本地化人員配置審核添加到每月 Docs/DevRel
同步”）和 Android AND5 i18n 計劃。使用它作為規范清單，並且
每當路線圖可交付成果添加或取消議程項目時更新它。

## 節奏與物流

- **頻率：** 每月（通常是第二個星期四，16:00UTC）
- **持續時間：** 45 分鐘 + 可選 15 分鐘深潛停留
- **位置：** Zoom (`https://meet.sora.dev/docs-devrel-sync`) 與共享
  HackMD 或 `docs/source/docs_devrel/minutes/<yyyy-mm>.md` 中的註釋
- **觀眾：** 文檔/DevRel 經理（主席）、文檔工程師、本地化
  程序經理、SDK DX TL（Android、Swift、JS）、產品文檔、發布
  工程代表、支持/QA 觀察員
- **協調人：** 文檔/DevRel 經理；任命一名輪流抄寫員，他將
  在 24 小時內將會議紀要提交到存儲庫中

## 工作前檢查表

|業主|任務|文物|
|--------|------|----------|
|抄寫員|使用下面的模板創建本月的筆記文件 (`docs/source/docs_devrel/minutes/<yyyy-mm>.md`)。 |筆記文件 |
|本地化項目經理 |刷新`docs/source/sdk/android/i18n_plan.md#translation-status`和人員配置日誌；預先填寫提議的決定。 | i18n 計劃 |
| DX TL |運行 `ci/check_android_docs_i18n.sh` 或 `scripts/sync_docs_i18n.py --dry-run` 並附加摘要以供討論。 | CI 文物 |
|文檔工具 |從 `docs/source/sdk/android/i18n_requests/` 導出 `docs/i18n/manifest.json` 摘要 + 未完成票據列表。 |艙單和票證摘要 |
|支持/發布 |收集需要 Docs/DevRel 操作的任何升級（例如，等待預覽邀請、阻止審閱者反饋）。 | Status.md 或升級文檔 |

## 議程塊1. **點名和目標（5 分鐘）**
   - 確認法定人數、抄寫員和後勤。
   - 突出顯示任何緊急事件（文檔預覽中斷、本地化阻止）。
2. **本地化人員配置審查（15分鐘）**
   - 登錄查看人員配置決策
     `docs/source/sdk/android/i18n_plan.md#staffing-decision-log`。
   - 確認未結採購訂單 (`DOCS-L10N-*`) 的狀態和臨時覆蓋範圍。
   - 比較 CI 新鮮度輸出與翻譯狀態表；呼出任何
     其區域設置 SLA（>5 個工作日）將在下一個工作日之前被違反的文檔
     同步。
   - 決定是否需要升級（產品運營、財務、承包商
     管理）。將決定記錄在人員配置日誌和月度日誌中
     分鐘，包括所有者 + 截止日期。
   - 如果人員配備狀況良好，請記錄確認信息，以便路線圖行動能夠
     帶證據回到🈺/🈴。
3. **文檔/路線圖更新（10 分鐘）**
   - DOCS-SORA 門戶工作、Try-It 代理和 SoraFS 發布的狀態
     準備狀態。
   - 突出顯示當前版本列車所需的文檔債務或審閱者。
4. **SDK 亮點（10 分鐘）**
   - Android AND5/AND7 文檔準備情況、Swift IOS5 奇偶校驗、JS GA 進度。
   - 捕獲將影響文檔的共享裝置或模式差異。
5. **行動回顧和停車場（5分鐘）**
   - 重新訪問上次同步中的未清項目；確認關閉。
   - 在筆記文件中記錄新操作，並明確所有者和截止日期。

## 本地化人員配置審核模板

在每月的會議記錄中包含下表：

|語言環境 |容量（全職員工）|承諾和採購訂單 |風險/升級|決策與所有者 |
|--------|----------------|--------------------|--------------------------------|--------------------|
|日本 |例如，0.5 個承包商 + 0.1 個文檔備份 | PO `DOCS-L10N-4901`（等待簽名）| “2026 年 3 月 4 日之前未簽署合同” | “升級至產品運營 — @docs-devrel，截止日期為 2026 年 3 月 2 日”|
|他|例如，0.1 文檔工程師 |輪換進入 PTO 2026-03-18 | “需要後備審稿人” | “@docs-lead 將在 2026 年 3 月 5 日之前確定備份”|

還記錄一個簡短的敘述，內容包括：

- **SLA 展望：** 任何預計會錯過五個工作日 SLA 和
  緩解措施（交換優先級、招募備份供應商等）。
- **門票和資產健康狀況：** 未完成的條目
  `docs/source/sdk/android/i18n_requests/` 以及屏幕截圖/資產是否
  為譯者做好準備。

### 本地化人員配置審核日誌記錄

- **會議記錄：** 將人員配置表 + 敘述複製到
  `docs/source/docs_devrel/minutes/<yyyy-mm>.md`（所有語言環境均鏡像
  英語分鐘通過同一目錄下的本地化文件）。鏈接條目
  回到議程（`docs/source/docs_devrel/monthly_sync_agenda.md`）所以
  治理可以追踪證據。
- **國際化計劃：**更新人員配置決策日誌和翻譯狀態表
  會議結束後立即在 `docs/source/sdk/android/i18n_plan.md` 中。
- **狀態：** 當人員配置決策影響路線圖大門時，請在
  `status.md`（Docs/DevRel 部分）引用分鍾文件和 i18n 計劃
  更新。

## 會議紀要模板

將此骨架複製到 `docs/source/docs_devrel/minutes/<yyyy-mm>.md` 中：

```markdown
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Docs/DevRel Monthly Sync — 2026-03-12

## Attendees
- Chair: …
- Scribe: …
- Participants: …

## Agenda Notes
1. Roll call & objectives — …
2. Localization staffing review — include table + narrative.
3. Docs/roadmap updates — …
4. SDK highlights — …
5. Action review & parking lot — …

## Decisions & Actions
| Item | Owner | Due | Notes |
|------|-------|-----|-------|
| JP contractor PO follow-up | @docs-devrel-manager | 2026-03-02 | Example entry |
```會議結束後立即通過 PR 發布筆記，並從 `status.md` 鏈接它們
在參考風險或人員配置決策時。

## 後續預期

1. **承諾的分鐘數：** 24 小時內 (`docs/source/docs_devrel/minutes/`)。
2. **國際化計劃更新：**調整人員配置日誌和翻譯表
   反映新的承諾或升級。
3. **Status.md 條目：** 總結任何高風險決策以保留路線圖
   同步。
4. **提交升級：** 當審核要求升級時，創建/刷新
   相關票據（例如產品運營、財務審批、供應商入職）
   並在會議記錄和 i18n 計劃中引用它。

通過遵循此議程，路線圖要求包括本地化
Docs/DevRel 每月同步中的人員配備審核保持可審計，並且下游
團隊總是知道在哪裡可以找到證據。