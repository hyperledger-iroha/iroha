---
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 13ace436f02f9656c5cbebed2fb297a1c4502027092a8b17d3f03023e8cb1d3a
source_last_modified: "2025-12-29T18:16:35.113124+00:00"
translation_last_reviewed: 2026-02-07
id: public-preview-invite
title: Public preview invite playbook
sidebar_label: Preview invite playbook
description: Checklist for announcing the docs portal preview to external reviewers.
translator: machine-google-reviewed
---

## 計劃目標

本手冊解釋瞭如何在發布後宣布並運行公共預覽版
審閱者入職工作流程已上線。它通過以下方式保持 DOCS-SORA 路線圖的誠實性：
確保每一次邀請都附帶可驗證的物品、安全指南和
清晰的反饋路徑。

- **觀眾：** 精選的社區成員、合作夥伴和維護者名單
  簽署預覽可接受使用政策。
- **上限：** 默認 Wave 大小 ≤ 25 名審閱者、14 天訪問窗口、事件
  24小時內回复。

## 啟動門控清單

在發送任何邀請之前完成這些任務：

1. CI 中上傳的最新預覽工件（`docs-portal-preview`，
   校驗和清單、描述符、SoraFS 捆綁包）。
2. `npm run --prefix docs/portal serve`（校驗和門控）在同一標籤上進行測試。
3. 審閱者入職票據已獲得批准並鏈接到邀請波。
4. 安全性、可觀察性和事件文檔經過驗證
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md))。
5. 準備反饋表或問題模板（包括嚴重性字段、
   重現步驟、屏幕截圖和環境信息）。
6. 公告副本由 Docs/DevRel + 治理審核。

## 邀請包

每份邀請必須包括：

1. **已驗證的工件** — 提供 SoraFS 清單/計劃或 GitHub 工件
   鏈接加上校驗和清單和描述符。參考驗證
   明確命令，以便審閱者可以在啟動站點之前運行它。
2. **服務說明** — 包括校驗和門控預覽命令：

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **安全提醒** — 註明令牌自動過期，鏈接
   不得共享，並應立即報告事件。
4. **反饋渠道** — 鏈接到問題模板/表格並澄清回复
   時間預期。
5. **計劃日期** — 提供開始/結束日期、辦公時間或同步會議，
   和下一個刷新窗口。

示例電子郵件位於
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
涵蓋了這些要求。更新佔位符（日期、URL、聯繫人）
發送之前。

## 暴露預覽主機

僅在入職完成並更改票證後才推廣預覽主機
已獲批准。參見【預覽主機曝光指南】(./preview-host-exposure.md)
了解本節中使用的端到端構建/發布/驗證步驟。

1. **構建並打包：** 標記發布標籤並產生確定性
   文物。

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   pin腳本寫入`portal.car`，`portal.manifest.*`，`portal.pin.proposal.json`，
   以及 `artifacts/sorafs/` 下的 `portal.dns-cutover.json`。將這些文件附加到
   邀請 Wave，以便每個審閱者都可以驗證相同的位。

2. **發布預覽別名：** 重新運行不帶 `--skip-submit` 的命令
   （供應 `TORII_URL`、`AUTHORITY`、`PRIVATE_KEY[_FILE]` 和
   治理頒發的別名證明）。該腳本將清單綁定到
   `docs-preview.sora` 並發出 `portal.manifest.submit.summary.json` plus
   `portal.pin.report.json` 用於證據包。

3. **探測部署：** 確認別名解析且校驗和匹配
   發送邀請之前的標籤。

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   將 `npm run serve` (`scripts/serve-verified-preview.mjs`) 放在手邊，作為
   回退，以便審閱者可以在預覽邊緣閃爍時啟動本地副本。

## 通訊時間表

|日 |行動|業主|
| ---| ---| ---|
| D-3 |完成邀請副本、刷新工件、試運行驗證 |文檔/開發版本 |
| D-2 |治理簽字+變更票|文檔/DevRel + 治理 |
| D-1 |使用模板發送邀請，使用收件人列表更新跟踪器 |文檔/開發版本 |
| d |啟動電話會議/辦公時間，監控遙測儀表板 |文檔/開發版本 + 待命 |
| D+7 |中點反饋摘要、分類阻塞問題|文檔/開發版本 |
| D+14 |關閉wave，撤銷臨時訪問，在 `status.md` 中發布摘要 |文檔/開發版本 |

## 訪問跟踪和遙測

1. 記錄每個收件人、邀請時間戳和撤銷日期
   預覽反饋記錄器（參見
   [`preview-feedback-log`](./preview-feedback-log)) 所以每個波共享
   相同的證據線索：

   ```bash
   # Append a new invite event to artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   支持的事件有 `invite-sent`、`acknowledged`、
   `feedback-submitted`、`issue-opened` 和 `access-revoked`。日誌位於
   默認為`artifacts/docs_portal_preview/feedback_log.json`；將其附加到
   邀請波票以及同意書。使用摘要助手
   在結賬前生成可審計的匯總：

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   JSON 摘要枚舉每波邀請、開放收件人、反饋
   計數以及最近事件的時間戳。助手的支持者是
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   因此相同的工作流程可以在本地或 CI 中運行。使用摘要模板
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   發布浪潮回顧時。
2. 使用用於波形的 `DOCS_RELEASE_TAG` 標記遙測儀表板，以便
   峰值可以與邀請群體相關。
3.部署後運行`npm run probe:portal -- --expect-release=<tag>`
   確認預覽環境公佈了正確的版本元數據。
4. 捕獲運行手冊模板中的所有事件並將其鏈接到群組。

## 反饋和結束

1. 在共享文檔或問題板上匯總反饋。為項目添加標籤
   `docs-preview/<wave>`，以便路線圖所有者可以輕鬆查詢它們。
2. 使用預覽記錄器的摘要輸出來填充波形報告，然後
   總結 `status.md` 中的隊列（參與者、主要發現、計劃
   修復）並更新 `roadmap.md`（如果 DOCS-SORA 里程碑發生更改）。
3. 按照以下的卸載步驟操作
   [`reviewer-onboarding`](./reviewer-onboarding.md)：撤銷訪問、存檔
   請求，並感謝參與者。
4. 通過刷新工件、重新運行校驗和門來準備下一波，
   並使用新日期更新邀請模板。

始終如一地應用此劇本可以使預覽程序保持可審核性
隨著門戶接近正式發布，為 Docs/DevRel 提供了一種可重複的方式來擴展邀請。