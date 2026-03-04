---
id: security-hardening
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/security-hardening.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Security hardening & pen-test checklist
sidebar_label: Security hardening
description: Harden the developer portal before exposing the Try it sandbox outside the lab.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## 概述

路線圖項目 **DOCS-1b** 需要 OAuth 設備代碼登錄，內容豐富
安全策略以及預覽門戶之前的可重複滲透測試
可以在非實驗室網絡上運行。本附錄解釋了威脅模型、
存儲庫中實施的控制措施以及門禁審查的上線清單
必須執行。

- **範圍：** Try it 代理、嵌入式 Swagger/RapiDoc 面板和自定義
  嘗試使用 `docs/portal/src/components/TryItConsole.jsx` 渲染的控制台。
- **超出範圍：** Torii 本身（由 Torii 準備情況審查涵蓋）和 SoraFS
  出版（由 DOCS-3/7 涵蓋）。

## 威脅模型

|資產|風險|緩解措施 |
| ---| ---| ---|
| Torii 不記名代幣 |在文檔沙箱之外被盜或重複使用 |設備代碼登錄 (`DOCS_OAUTH_*`) 會生成短期令牌，代理會編輯標頭，控制台會自動使緩存的憑據過期。 |
|嘗試一下代理 |濫用作為開放中繼或繞過 Torii 速率限制 | `scripts/tryit-proxy*.mjs` 強制執行來源允許列表、速率限制、運行狀況探測和顯式 `X-TryIt-Auth` 轉發；沒有保留任何憑據。 |
|門戶運行時 |跨站點腳本或惡意嵌入| `docusaurus.config.js` 注入 Content-Security-Policy、Trusted Types 和 Permissions-Policy 標頭；內聯腳本僅限於 Docusaurus 運行時。 |
|可觀測數據|遙測丟失或篡改| `docs/portal/docs/devportal/observability.md` 記錄探針/儀表板； `scripts/portal-probe.mjs` 在發布之前在 CI 中運行。 |

對手包括查看公共預覽版的好奇用戶、惡意行為者
測試被盜鏈接，以及嘗試抓取存儲的受感染瀏覽器
憑據。所有控件都必須在不受信任的商品瀏覽器上運行
網絡。

## 所需的控制

1. **OAuth設備碼登錄**
   - 配置 `DOCS_OAUTH_DEVICE_CODE_URL`、`DOCS_OAUTH_TOKEN_URL`、
     `DOCS_OAUTH_CLIENT_ID`，以及構建環境中的相關旋鈕。
   - Try it 卡呈現一個登錄小部件 (`OAuthDeviceLogin.jsx`)
     獲取設備代碼、輪詢令牌端點並自動清除令牌
     一旦過期。緊急情況下仍可使用手動承載超控
     後備。
   - 當 OAuth 配置丟失或
     後備 TTL 漂移到 DOCS-1b 規定的 300 秒至 900 秒窗口之外；
     僅針對一次性本地預覽設置 `DOCS_OAUTH_ALLOW_INSECURE=1`。
2. **代理護欄**
   - `scripts/tryit-proxy.mjs` 強制執行允許的來源、速率限制、請求
     大小上限和上游超時，同時標記流量
     `X-TryIt-Client` 並從日誌中編輯令牌。
   - `scripts/tryit-proxy-probe.mjs` 加上 `docs/portal/docs/devportal/observability.md`
     定義活性探針和儀表板規則；在每個之前運行它們
     推出。
3. **CSP、可信類型、權限策略**
   - `docusaurus.config.js` 現在導出確定性安全標頭：
     `Content-Security-Policy`（默認-src self，嚴格connect/img/script
     列表、可信類型要求）、`Permissions-Policy` 和
     `Referrer-Policy: no-referrer`。
   - CSP 連接列表將 OAuth 設備代碼和令牌端點列入白名單
     （僅限 HTTPS，除非 `DOCS_SECURITY_ALLOW_INSECURE=1`）因此設備登錄有效
     而不放寬其他來源的沙箱。
   - 標頭直接嵌入生成的 HTML 中，因此靜態主機可以這樣做
     不需要額外的配置。將內聯腳本限制為
     Docusaurus 引導程序。
4. **運行手冊、可觀察性和回滾**
   - `docs/portal/docs/devportal/observability.md` 描述了探頭和
     用於監視登錄失敗、代理響應代碼和請求的儀表板
     預算。
   - `docs/portal/docs/devportal/incident-runbooks.md` 涵蓋升級
     沙箱被濫用時的路徑；將其與
     `scripts/tryit-proxy-rollback.mjs` 安全翻轉端點。

## 滲透測試和發布清單

為每個預覽促銷完成此列表（將結果附加到版本中）
票）：

1. **驗證 OAuth 接線**
   - 使用生產 `DOCS_OAUTH_*` 導出在本地運行 `npm run start`。
   - 從乾淨的瀏覽器配置文件中，打開“嘗試”控制台並確認
     設備代碼流鑄造一個令牌，倒計時生命週期，並清除
     過期或註銷後的字段。
2. **探測代理**
   - `npm run tryit-proxy` 針對暫存 Torii，然後執行
     `npm run probe:tryit-proxy` 以及已配置的示例路徑。
   - 檢查日誌中的 `authSource=override` 條目並確認速率限制
     當超出窗口時增加計數器。
3. **確認 CSP/可信類型**
   - `npm run build` 並打開 `build/index.html`。確保`<元
     http-equiv="Content-Security-Policy">` 標記與預期指令匹配
     並且 DevTools 在加載預覽時顯示沒有 CSP 違規。
   - 使用`npm run probe:portal`（或curl）獲取部署的HTML；探頭
     現在，當 `Content-Security-Policy`、`Permissions-Policy` 或
     `Referrer-Policy` 元標記丟失或與聲明的值不同
     在 `docusaurus.config.js` 中，因此治理審核者可以依賴退出
     代碼而不是盯著curl 輸出。
4. **審查可觀察性**
   - 驗證 Try it 代理儀表板是否為綠色（速率限制、錯誤率、
     運行狀況探測指標）。
   - 在 `docs/portal/docs/devportal/incident-runbooks.md` 中運行事件演習
     如果主機發生更改（新的 Netlify/SoraFS 部署）。
5. **記錄結果**
   - 將屏幕截圖/日誌附加到發布票證中。
   - 在補救報告模板中記錄每項發現
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     因此所有者、SLA 和重新測試證據很容易在以後進行審核。
   - 鏈接回此清單，以便 DOCS-1b 路線圖項目保持可審核狀態。

如果任何步驟失敗，請停止升級，提交阻止問題，並記下
`status.md` 中的修復計劃。