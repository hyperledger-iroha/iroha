---
lang: zh-hant
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5246118a539e2031dcafb8cf384ac7d20b8abc28b67ee1555e1b1211779fe390
source_last_modified: "2026-01-22T16:26:46.508367+00:00"
translation_last_reviewed: 2026-02-07
title: Norito Try-It Console
description: Use the developer-portal proxy, Swagger, and RapiDoc widgets to send real Torii / Norito-RPC requests directly from the documentation site.
translator: machine-google-reviewed
---

該門戶捆綁了三個交互式界面，將流量中繼到 Torii：

- `/reference/torii-swagger` 處的 **Swagger UI** 呈現簽名的 OpenAPI 規範，並在設置 `TRYIT_PROXY_PUBLIC_URL` 時自動通過代理重寫請求。
- `/reference/torii-rapidoc` 的 **RapiDoc** 公開了與文件上傳和內容類型選擇器相同的架構，適用於 `application/x-norito`。
- Norito 概述頁面上的 **Try it sandbox** 為臨時 REST 請求和 OAuth 設備登錄提供了輕量級表單。

所有三個小部件都將請求發送到本地 **Try-It 代理** (`docs/portal/scripts/tryit-proxy.mjs`)。代理驗證 `static/openapi/torii.json` 是否與 `static/openapi/manifest.json` 中的簽名摘要匹配，實施速率限制器，編輯日誌中的 `X-TryIt-Auth` 標頭，並使用 `X-TryIt-Client` 標記每個上游調用，以便 Torii 操作員可以審核流量源。

## 啟動代理

```bash
cd docs/portal
npm install
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
# Optional, use short-lived tokens only:
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
export TRYIT_PROXY_CLIENT_ID="docs-portal"
export DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1
npm run tryit-proxy
```

- `TRYIT_PROXY_TARGET` 是您要使用的 Torii 基本 URL。
- `TRYIT_PROXY_ALLOWED_ORIGINS` 必須包含應嵌入控制台的每個門戶源（本地開發服務器、生產主機名、預覽 URL）。
- `TRYIT_PROXY_PUBLIC_URL` 由 `docusaurus.config.js` 消耗並通過 `customFields.tryIt` 注入到小部件中。
- `TRYIT_PROXY_BEARER`僅在`DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`時加載；否則，用戶必須通過控制台或 OAuth 設備流提供自己的令牌。
- `TRYIT_PROXY_CLIENT_ID` 設置每個請求攜帶的 `X-TryIt-Client` 標籤。
  允許從瀏覽器提供 `X-TryIt-Client`，但值會被修剪
  如果它們包含控製字符則被拒絕。

啟動時，代理運行 `verifySpecDigest`，如果清單已過時，則退出並顯示修復提示。運行 `npm run sync-openapi -- --latest` 下載最新的 Torii 規範或通過 `TRYIT_PROXY_ALLOW_STALE_SPEC=1` 進行緊急覆蓋。

要更新或回滾代理目標而不手動編輯環境文件，請使用幫助程序：

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## 連接小部件

代理監聽後為門戶提供服務：

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` 公開了以下旋鈕：

|變量|目的|
| ---| ---|
| `TRYIT_PROXY_PUBLIC_URL` | URL 注入到 Swagger、RapiDoc 和 Try it 沙箱中。保持未設置狀態可在未經授權的預覽期間隱藏小部件。 |
| `TRYIT_PROXY_DEFAULT_BEARER` |可選的默認令牌存儲在內存中。需要 `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` 和僅 HTTPS CSP 防護 (DOCS-1b)，除非您在本地傳遞 `DOCS_SECURITY_ALLOW_INSECURE=1`。 |
| `DOCS_OAUTH_*` |啟用 OAuth 設備流（`OAuthDeviceLogin` 組件），以便審閱者可以在不離開門戶的情況下創建短期令牌。 |

當 OAuth 變量存在時，沙箱會呈現一個 **使用設備代碼登錄** 按鈕，該按鈕遍歷配置的身份驗證服務器（有關確切形狀，請參閱 `config/security-helpers.js`）。通過設備流發出的令牌僅緩存在瀏覽器會話中。

## 發送 Norito-RPC 有效負載

1. 使用 [Norito 快速入門](./quickstart.md) 中描述的 CLI 或代碼片段構建 `.norito` 有效負載。代理轉發 `application/x-norito` 主體不變，因此您可以重複使用與 `curl` 一起發布的相同工件。
2. 打開 `/reference/torii-rapidoc`（二進制有效負載首選）或 `/reference/torii-swagger`。
3. 從下拉列表中選擇所需的 Torii 快照。快照已簽名；該面板顯示 `static/openapi/manifest.json` 中記錄的清單摘要。
4. 在“Try it”抽屜中選擇 `application/x-norito` 內容類型，單擊 **選擇文件**，然後選擇您的負載。代理將請求重寫為 `/proxy/v1/pipeline/submit` 並用 `X-TryIt-Client=docs-portal-rapidoc` 對其進行標記。
5. 要下載 Norito 響應，請設置 `Accept: application/x-norito`。 Swagger/RapiDoc 在同一個抽屜中公開標頭選擇器，並通過代理將二進製文件流回。

對於純 JSON 路由，嵌入式 Try it 沙箱通常更快：輸入路徑（例如 `/v1/accounts/ih58.../assets`），選擇 HTTP 方法，在需要時粘貼 JSON 正文，然後點擊 **發送請求** 以內聯檢查標頭、持續時間和有效負載。

## 故障排除

|症狀|可能的原因 |修復|
| ---| ---| ---|
|瀏覽器控制台顯示 CORS 錯誤或沙箱警告代理 URL 丟失。 |代理未運行或源未列入白名單。 |啟動代理，確保 `TRYIT_PROXY_ALLOWED_ORIGINS` 覆蓋您的門戶主機，然後重新啟動 `npm run start`。 |
| `npm run tryit-proxy` 退出並顯示“摘要不匹配”。 | Torii OpenAPI 捆綁包已更改為上游。 |運行 `npm run sync-openapi -- --latest`（或 `--version=<tag>`）並重試。 |
|小組件返回 `401` 或 `403`。 |令牌丟失、過期或範圍不足。 |使用 OAuth 設備流或將有效的承載令牌粘貼到沙箱中。對於靜態令牌，您必須導出 `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`。 |
|來自代理的 `429 Too Many Requests`。 |超出每個 IP 的速率限制。 |為可信環境或限制測試腳本提高 `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS`。所有速率限制拒絕都會增加 `tryit_proxy_rate_limited_total`。 |

## 可觀察性

- `npm run probe:tryit-proxy`（`scripts/tryit-proxy-probe.mjs` 的包裝）調用 `/healthz`，可選擇執行示例路由，並為 `probe_success` / `probe_duration_seconds` 發出 Prometheus 文本文件。配置 `TRYIT_PROXY_PROBE_METRICS_FILE` 以與 node_exporter 集成。
- 設置 `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` 以公開計數器（`tryit_proxy_requests_total`、`tryit_proxy_rate_limited_total`、`tryit_proxy_upstream_failures_total`）和延遲直方圖。 `dashboards/grafana/docs_portal.json` 板讀取這些指標以實施 DOCS-SORA SLO。
- 運行時日誌位於標準輸出上。每個條目包括請求 ID、上游狀態、身份驗證源（`default`、`override` 或 `client`）和持續時間；秘密在發布前被編輯。

如果您需要驗證 `application/x-norito` 有效負載是否達到 Torii 不變，請運行 Jest 套件 (`npm test -- tryit-proxy`) 或檢查 `docs/portal/scripts/__tests__/tryit-proxy.test.mjs` 下的裝置。回歸測試涵蓋壓縮的 Norito 二進製文件、簽名的 OpenAPI 清單和代理降級路徑，以便 NRPC 推出保留永久的證據跟踪。