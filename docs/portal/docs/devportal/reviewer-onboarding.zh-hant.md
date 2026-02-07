---
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f42888a06cb49f9fe53f424ef77c84e2fa3a305f558e202be0fbbd4b3b0ea1d7
source_last_modified: "2025-12-29T18:16:35.114535+00:00"
translation_last_reviewed: 2026-02-07
id: reviewer-onboarding
title: Preview reviewer onboarding
sidebar_label: Reviewer onboarding
description: Process and checklists for enrolling reviewers in the docs portal public preview.
translator: machine-google-reviewed
---

## 概述

DOCS-SORA 跟踪開發人員門戶的分階段啟動。校驗和門控構建
(`npm run serve`) 並強化嘗試流程，解鎖下一個里程碑：
在公開預覽版廣泛開放之前，讓經過審查的審稿人加入。本指南
描述如何收集請求、驗證資格、提供訪問權限以及
參與者安全下機。請參閱
[預覽邀請流程](./preview-invite-flow.md) 用於群組規劃、邀請
節奏和遙測導出；以下步驟重點關注要採取的行動
一旦選定審稿人。

- **範圍：**需要訪問文檔預覽的審閱者（`docs-preview.sora`，
  GA 之前的 GitHub Pages 版本或 SoraFS 捆綁包）。
- **超出範圍：** Torii 或 SoraFS 操作員（由他們自己的入職培訓涵蓋）
  套件）和生產門戶部署（請參閱
  [`devportal/deploy-guide`](./deploy-guide.md))。

## 角色和先決條件

|角色 |典型目標 |所需文物|筆記|
| ---| ---| ---| ---|
|核心維護者 |驗證新指南，運行冒煙測試。 | GitHub 句柄、Matrix 聯繫人、已簽署的 CLA 存檔。 |通常已經在 `docs-preview` GitHub 團隊中；仍然提出請求，以便訪問是可審核的。 |
|合作夥伴審稿人 |在公開發布之前驗證 SDK 片段或治理內容。 |公司電子郵件、合法 POC、簽署的預覽條款。 |必須承認遙測+數據處理要求。 |
|社區志願者|提供有關指南的可用性反饋。 | GitHub 句柄、首選聯繫人、時區、CoC 接受情況。 |保持小規模；優先考慮已簽署貢獻者協議的審稿人。 |

所有審閱者類型必須：

1. 確認預覽製品的可接受使用政策。
2.閱讀安全性/可觀察性附錄
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md))。
3. 同意在提供任何服務之前運行 `docs/portal/scripts/preview_verify.sh`
   本地快照。

## 攝入工作流程

1.請請求者填寫
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   表單（或將其複制/粘貼到問題中）。至少捕獲：身份、聯繫方式
   方法、GitHub 句柄、預期審核日期以及確認
   已閱讀安全文檔。
2. 在 `docs-preview` 跟踪器中記錄請求（GitHub 問題或治理
   票）並指定審批者。
3. 驗證先決條件：
   - CLA/貢獻者協議存檔（或合作夥伴合同參考）。
   - 存儲在請求中的可接受使用確認。
   - 風險評估完成（例如，經法務部批准的合作夥伴審核員）。
4. 審批者在請求中籤字並將跟踪問題鏈接到任何
   變更管理條目（示例：`DOCS-SORA-Preview-####`）。

## 配置和工具

1. **共享工件** — 提供最新的預覽描述符+存檔
   CI 工作流程或 SoraFS 引腳（`docs-portal-preview` 工件）。提醒
   審稿人運行：

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **提供校驗和執行服務** - 將審核者指向校驗和門控
   命令：

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   這會重用 `scripts/serve-verified-preview.mjs`，因此不會出現未經驗證的構建
   意外啟動。

3. **授予 GitHub 訪問權限（可選）** — 如果審閱者需要未發布的分支，
   在審核期間將它們添加到 `docs-preview` GitHub 團隊，並
   在請求中記錄成員資格變更。

4. **溝通支持渠道** — 共享待命聯繫人 (Matrix/Slack)
   以及來自 [`incident-runbooks`](./incident-runbooks.md) 的事件程序。

5. **遙測+反饋** - 提醒審閱者匿名分析是
  收集（請參閱 [`observability`](./observability.md)）。提供反饋
  邀請中引用的表單或問題模板，並使用以下內容記錄事件
  [`preview-feedback-log`](./preview-feedback-log) 幫手所以波總結
  保持最新狀態。

## 審稿人清單

在訪問預覽之前，審閱者必須完成以下操作：

1. 驗證下載的工件 (`preview_verify.sh`)。
2. 通過 `npm run serve`（或 `serve:verified`）啟動門戶，以確保
   校驗和防護處於活動狀態。
3. 閱讀上面鏈接的安全性和可觀察性說明。
4. 使用設備代碼登錄（如果適用）測試 OAuth/Try it 控制台並
   避免重複使用生產代幣。
5. 在商定的跟踪器（問題、共享文檔或表單）和標籤中記錄結果
   它們帶有預覽版本標籤。

## 維護者職責和離職

|相|行動|
| ---| ---|
|開球 |確認請求中附加了接收清單，共享工件 + 說明，通過 [`preview-feedback-log`](./preview-feedback-log) 附加 `invite-sent` 條目，並在審核持續時間超過一周的情況下安排中點同步。 |
|監控|跟踪預覽遙測（查找異常的 Try it 流量、探測失敗），並在發生任何可疑情況時遵循事件運行手冊。當發現結果到達時記錄 `feedback-submitted`/`issue-opened` 事件，以便波指標保持準確。 |
|離職|撤銷臨時 GitHub 或 SoraFS 訪問權限，記錄 `access-revoked`，存檔請求（包括反饋摘要 + 未完成的操作），並更新審閱者註冊表。要求審閱者清除本地構建並附加從 [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md) 生成的摘要。 |

在輪換審閱者時使用相同的流程。保持
存儲庫中的書面記錄（問題 + 模板）有助於 DOCS-SORA 保持可審核性
讓治理確認預覽訪問遵循記錄的控制。

## 邀請模板和跟踪

- 開始每次外展活動
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  文件。它捕獲最低限度的法律語言、預覽校驗和指令、
  以及審閱者認可可接受使用政策的期望。
- 編輯模板時，替換`<preview_tag>`的佔位符，
  `<request_ticket>`，以及聯繫渠道。將最終消息的副本存儲在
  受理單，以便審閱者、批准者和審計員可以參考
  發送的確切措辭。
- 發送邀請後，更新跟踪電子表格或問題
  `invite_sent_at` 時間戳和預期結束日期，因此
  [預覽邀請流程](./preview-invite-flow.md) 舉報可領取隊列
  自動。