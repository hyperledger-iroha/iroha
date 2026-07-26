---
id: preview-invite-flow
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Preview invite flow
sidebar_label: Preview invite flow
description: Sequencing, evidence, and communications plan for the docs portal public preview waves.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## 目的

路線圖項目 **DOCS-SORA** 呼籲審閱者加入和公共預覽
在門戶退出測試版之前，邀請程序作為最終的攔截者。本頁
描述如何打開每個邀請波，哪些工件必須在之前發送
邀請出去，以及如何證明流程是可審計的。與它一起使用：

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) 為
  每個審稿人的處理。
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) 用於校驗和
  保證。
- [`devportal/observability`](./observability.md) 用於遙測導出和
  警報鉤子。

## 波浪計劃

|波|觀眾|進入標準|退出標準|筆記|
| ---| ---| ---| ---| ---|
| **W0 – 核心維護者** |文檔/SDK 維護人員驗證第一天的內容。 | `docs-portal-preview` GitHub 團隊已填充，`npm run serve` 校驗和門呈綠色，Alertmanager 已安靜 7 天。 |所有 P0 文檔均已審核，積壓工作已標記，無阻塞事件。 |用於驗證流程；沒有邀請電子郵件，只需分享預覽工件。 |
| **W1 – 合作夥伴** | SoraFS 運營商、Torii 集成商、NDA 下的治理審核員。 | W0 退出，法律條款獲得批准，Try-it 代理上演。 |收集的合作夥伴簽字（問題或簽名表格），遙測顯示 ≤10 個並發審核者，14 天內沒有安全回歸。 |強制執行邀請模板+請求門票。 |
| **W2 – 社區** |從社區候補名單中選定的貢獻者。 | W1 退出，事件演習演練，公共常見問題解答更新。 |反饋已消化，≥2 個文檔版本通過預覽管道發布，無需回滾。 |並發邀請上限 (≤25) 和每週批量。 |

記錄 `status.md` 和預覽請求中哪個波形處於活動狀態
跟踪器，以便治理可以一目了然地看到程序所在的位置。

## 飛行前檢查表

**在**安排波次邀請之前完成以下操作：

1. **可用 CI 工件**
   - 最新的 `docs-portal-preview` + 描述符上傳者
     `.github/workflows/docs-portal-preview.yml`。
   - `docs/portal/docs/devportal/deploy-guide.md` 中註明的 SoraFS 引腳
     （存在切換描述符）。
2. **校驗和執行**
   - `docs/portal/scripts/serve-verified-preview.mjs` 通過調用
     `npm run serve`。
   - `scripts/preview_verify.sh` 指令在 macOS + Linux 上測試。
3. **遙測基線**
   - `dashboards/grafana/docs_portal.json` 顯示健康的 Try it 流量和
     `docs.preview.integrity` 警報為綠色。
   - 最新的 `docs/portal/docs/devportal/observability.md` 附錄更新為
     Grafana 鏈接。
4. **治理文物**
   - 邀請跟踪器問題已準備就緒（每波一個問題）。
   - 複製審閱者註冊表模板（請參閱
     [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md))。
   - 該問題附帶法律和 SRE 所需的批准。

在發送任何郵件之前，在邀請跟踪器中記錄預檢完成情況。

## 流程步驟

1. **選擇候選人**
   - 從候補名單電子表格或合作夥伴隊列中提取。
   - 確保每位候選人都有完整的請求模板。
2. **批准訪問**
   - 為邀請跟踪器問題分配審批者。
   - 驗證先決條件（CLA/合同、可接受的用途、安全簡介）。
3. **發送邀請**
   - 填寫
     [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
     佔位符（`<preview_tag>`、`<request_ticket>`、聯繫人）。
   - 附加描述符+存檔哈希，嘗試暫存 URL，並支持
     渠道。
   - 將最終電子郵件（或 Matrix/Slack 記錄）存儲在問題中。
4. **跟踪入職**
   - 使用 `invite_sent_at`、`expected_exit_at` 更新邀請跟踪器，以及
     狀態（`pending`、`active`、`complete`、`revoked`）。
   - 鏈接到審稿人的可審核性接收請求。
5. **監控遙測**
   - 觀看 `docs.preview.session_active` 和 `TryItProxyErrors` 警報。
   - 如果遙測偏離基線，則提交事件並記錄
     邀請條目旁邊的結果。
6. **收集反饋並退出**
   - 一旦收到反饋或 `expected_exit_at` 通過，即可關閉邀請。
   - 用簡短的摘要更新浪潮問題（調查結果、事件、下一步
     行動），然後再進入下一個隊列。

## 證據和報告

|文物|存放地點 |刷新節奏 |
| ---| ---| ---|
|邀請追踪器問題 | `docs-portal-preview` GitHub 項目 |每次邀請後更新。 |
|審稿人名冊導出| `docs/portal/docs/devportal/reviewer-onboarding.md` 鏈接註冊表 |每週。 |
|遙測快照 | `docs/source/sdk/android/readiness/dashboards/<date>/`（重用遙測包）|每波 + 事件發生後。 |
|反饋摘要| `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md`（每波創建文件夾）|退出後 5 天內。 |
|治理會議記錄| `docs/portal/docs/devportal/preview-invite-notes/<date>.md` |在每個 DOCS-SORA 治理同步之前填充。 |

運行 `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
在每個批次之後生成機器可讀的事件摘要。附上渲染圖
JSON 到 Wave 問題，以便治理審核者可以確認邀請計數，而無需
重播整個日誌。

每當一波結束時，將證據列表附加到 `status.md`，以便路線圖
條目可以快速更新。

## 回滾和暫停條件

當發生以下任何情況時，暫停邀請流程（並通知治理）：

- 需要回滾的 Try it 代理事件 (`npm run manage:tryit-proxy`)。
- 警報疲勞：7​​ 天內針對僅限預覽的端點超過 3 個警報頁面。
- 合規性差距：在未簽署條款或未登錄的情況下發送邀請
  請求模板。
- 完整性風險：`scripts/preview_verify.sh` 檢測到校驗和不匹配。

僅在邀請跟踪器中記錄補救措施後才能恢復，並且
確認遙測儀表板至少穩定 48 小時。