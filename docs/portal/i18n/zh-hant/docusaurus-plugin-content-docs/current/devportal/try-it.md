---
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/try-it.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# 嘗試一下沙盒

開發者門戶提供了一個可選的“Try it”控制台，因此您可以調用 Torii
端點無需離開文檔。控制台轉發請求
通過捆綁代理，瀏覽器可以繞過 CORS 限制，同時仍然
實施速率限制和身份驗證。

## 先決條件

- Node.js 18.18 或更高版本（符合門戶構建要求）
- 對 Torii 暫存環境的網絡訪問
- 不記名令牌，可以調用您計劃行使的 Torii 路線

所有代理配置都是通過環境變量完成的。下表
列出了最重要的旋鈕：

|變量|目的|默認 |
| ---| ---| ---|
| `TRYIT_PROXY_TARGET` |代理將請求轉發到的基本 Torii URL | **必填** |
| `TRYIT_PROXY_LISTEN` |本地開發監聽地址（格式`host:port`或`[ipv6]:port`）| `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` |可能調用代理的來源的逗號分隔列表 | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` |每個上游請求的標識符都放置在 `X-TryIt-Client` 中 | `docs-portal` |
| `TRYIT_PROXY_BEARER` |默認不記名令牌轉發至 Torii | _空_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` |允許最終用戶通過 `X-TryIt-Auth` 提供自己的代幣 | `0` |
| `TRYIT_PROXY_MAX_BODY` |最大請求正文大小（字節）| `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` |上行超時（以毫秒為單位）| `10000` |
| `TRYIT_PROXY_RATE_LIMIT` |每個客戶端 IP 每個速率窗口允許的請求數 | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` |速率限制滑動窗口（毫秒） | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus 樣式指標端點的可選偵聽地址（`host:port` 或 `[ipv6]:port`）| _空（已禁用）_ |
| `TRYIT_PROXY_METRICS_PATH` |指標端點提供的 HTTP 路徑 | `/metrics` |

該代理還公開 `GET /healthz`，返回結構化 JSON 錯誤，並且
從日誌輸出中編輯不記名令牌。

向文檔用戶公開代理時啟用 `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`，以便 Swagger 和
RapiDoc 面板可以轉髮用戶提供的不記名令牌。代理仍然執行速率限制，
編輯憑據，並記錄請求是使用默認令牌還是每個請求覆蓋。
將 `TRYIT_PROXY_CLIENT_ID` 設置為您想要作為 `X-TryIt-Client` 發送的標籤
（默認為 `docs-portal`）。代理修剪並驗證調用者提供的
`X-TryIt-Client` 值，回退到此默認值，以便臨時網關可以
無需關聯瀏覽器元數據即可審核來源。

## 本地啟動代理

首次設置門戶時安裝依賴項：

```bash
cd docs/portal
npm install
```

運行代理並將其指向您的 Torii 實例：

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

該腳本記錄綁定地址並將來自 `/proxy/*` 的請求轉發到
配置 Torii 原點。

在綁定套接字之前，腳本會驗證
`static/openapi/torii.json` 與中記錄的摘要匹配
`static/openapi/manifest.json`。如果文件發生漂移，該命令將退出並顯示
錯誤並指示您運行 `npm run sync-openapi -- --latest`。出口
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` 僅用於緊急超越；代理將
記錄警告並繼續，以便您可以在維護時段內恢復。

## 連接門戶小部件

當您構建或提供開發人員門戶時，請設置小部件所使用的 URL
應該用於代理：

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

以下組件從 `docusaurus.config.js` 讀取這些值：

- **Swagger UI** — 在 `/reference/torii-swagger` 處渲染；預授權
  持有者方案當存在令牌時，用 `X-TryIt-Client` 標記請求，
  注入 `X-TryIt-Auth`，並在以下情況下通過代理重寫調用
  `TRYIT_PROXY_PUBLIC_URL` 已設置。
- **RapiDoc** — 在 `/reference/torii-rapidoc` 渲染；鏡像令牌字段，
  重用與 Swagger 面板相同的標頭，並以代理為目標
  配置 URL 時自動。
- **嘗試控制台** — 嵌入 API 概述頁面；讓您發送自定義
  請求、查看標頭並檢查響應正文。

兩個面板上都有一個**快照選擇器**，內容為
`docs/portal/static/openapi/versions.json`。將該索引填充為
`npm run sync-openapi -- --version=<label> --mirror=current --latest`所以
審閱者可以在歷史規範之間跳轉，查看記錄的 SHA-256 摘要，
並在使用前確認發布快照是否帶有簽名清單
交互式小部件。

更改任何小部件中的令牌只會影響當前瀏覽器會話；的
代理永遠不會保留或記錄提供的令牌。

## 短暫的 OAuth 令牌

為了避免將長期存在的 Torii 令牌分發給審閱者，請連接 Try it
控制台到您的 OAuth 服務器。當存在以下環境變量時
門戶呈現設備代碼登錄小部件，鑄造短期不記名令牌，
並自動將它們注入到控制台表單中。

|變量|目的|默認|
| ---| ---| ---|
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth 設備授權端點 (`/oauth/device/code`) | _空（已禁用）_ |
| `DOCS_OAUTH_TOKEN_URL` |接受 `grant_type=urn:ietf:params:oauth:grant-type:device_code` 的令牌端點 | _空_ |
| `DOCS_OAUTH_CLIENT_ID` |為文檔預覽註冊的 OAuth 客戶端標識符 | _空_ |
| `DOCS_OAUTH_SCOPE` |登錄期間請求的以空格分隔的範圍 | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` |將令牌綁定到的可選 API 受眾 | _空_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` |等待批准時的最小輪詢間隔（毫秒） | `5000`（<5000ms 的值被拒絕）|
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` |後備設備代碼過期窗口（秒）| `600`（必須保持在 300 到 900 之間）|
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` |後備訪問令牌生命週期（秒）| `900`（必須保持在 300 到 900 之間）|
| `DOCS_OAUTH_ALLOW_INSECURE` |設置為 `1` 用於有意跳過 OAuth 強制執行的本地預覽 | _取消設置_ |

配置示例：

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

當您運行 `npm run start` 或 `npm run build` 時，門戶會嵌入這些值
在 `docusaurus.config.js` 中。在本地預覽期間，嘗試卡會顯示
“使用設備代碼登錄”按鈕。用戶在您的 OAuth 上輸入顯示的代碼
驗證頁面；一旦設備流程成功，小部件就會：

- 將頒發的不記名令牌注入 Try it 控制台字段，
- 使用現有的 `X-TryIt-Client` 和 `X-TryIt-Auth` 標頭標記請求，
- 顯示剩餘壽命，以及
- 令牌過期時自動清除。

手動承載輸入仍然可用 - 無論何時您都可以忽略 OAuth 變量
想要強制審閱者自己粘貼臨時令牌，或導出
`DOCS_OAUTH_ALLOW_INSECURE=1` 用於匿名訪問的隔離本地預覽
是可以接受的。未配置 OAuth 的構建現在無法快速滿足
DOCS-1b 路線圖門。

📌 查看[安全強化和滲透測試清單](./security-hardening.md)
在將門戶暴露在實驗室之外之前；它記錄了威脅模型，
CSP/可信類型配置文件，以及現在用於 DOCS-1b 的滲透測試步驟。

## Norito-RPC 樣本

Norito-RPC 請求與 JSON 路由共享相同的代理和 OAuth 管道，
他們只需設置 `Content-Type: application/x-norito` 並發送
NRPC 規範中描述的預編碼 Norito 有效負載
（`docs/source/torii/nrpc_spec.md`）。
該存儲庫在 `fixtures/norito_rpc/` 下提供規範的有效負載，因此門戶
作者、SDK 所有者和審閱者可以重放 CI 使用的確切字節。

### 從 Try It 控制台發送 Norito 有效負載

1. 選擇一個夾具，例如 `fixtures/norito_rpc/transfer_asset.norito`。這些
   文件是原始 Norito 信封； **不要**對它們進行 base64 編碼。
2. 在 Swagger 或 RapiDoc 中，找到 NRPC 端點（例如
   `POST /v2/pipeline/submit`）並將 **Content-Type** 選擇器切換為
   `application/x-norito`。
3. 將請求正文編輯器切換為 **binary**（Swagger 的“文件”模式或
   RapiDoc 的“二進制/文件”選擇器）並上傳 `.norito` 文件。小部件
   通過代理流式傳輸字節而不進行任何更改。
4. 提交請求。如果 Torii 返回 `X-Iroha-Error-Code: schema_mismatch`，
   驗證您正在調用接受二進制有效負載的端點並且
   確認 `fixtures/norito_rpc/schema_hashes.json` 中記錄的模式哈希
   與您正在使用的 Torii 版本匹配。

控制台將最新的文件保留在內存中，以便您可以重新提交相同的文件
有效負載同時使用不同的授權令牌或 Torii 主機。添加
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` 到您的工作流程產生
NRPC-4 採用計劃中引用的證據包（日誌 + JSON 摘要），
這與在評論期間截屏“嘗試一下”響應非常搭配。

### CLI 示例 (curl)

相同的賽程可以通過 `curl` 在門戶外重播，這很有用
驗證代理或調試網關響應時：

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl \
  -H "Content-Type: application/x-norito" \
  -H "Authorization: ${TOKEN}" \
  --data-binary @fixtures/norito_rpc/transfer_asset.norito \
  "${TORII}/v2/pipeline/submit"
```

將夾具替換為 `transaction_fixtures.manifest.json` 中列出的任何條目
或者使用 `cargo xtask norito-rpc-fixtures` 編碼您自己的有效負載。當 Torii
處於金絲雀模式，您可以將 `curl` 指向 try-it 代理
(`https://docs.sora.example/proxy/v2/pipeline/submit`) 進行同樣的練習
門戶小部件使用的基礎設施。

## 可觀察性和操作每個請求都會記錄一次，其中包括方法、路徑、來源、上游狀態和
身份驗證源（`override`、`default` 或 `client`）。代幣從來都不是
存儲 — 承載標頭和 `X-TryIt-Auth` 值均在之前經過編輯
日誌記錄——這樣你就可以將標準輸出轉發到中央收集器，而不必擔心
秘密洩露。

### 健康探測和警報

在部署期間或按計劃運行捆綁探針：

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

環境旋鈕：

- `TRYIT_PROXY_SAMPLE_PATH` — 可選的 Torii 路線（無 `/proxy`）進行鍛煉。
- `TRYIT_PROXY_SAMPLE_METHOD` — 默認為 `GET`；設置為 `POST` 用於寫入路由。
- `TRYIT_PROXY_PROBE_TOKEN` — 為示例調用注入臨時承載令牌。
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — 覆蓋默認的 5 秒超時。
- `TRYIT_PROXY_PROBE_METRICS_FILE` — `probe_success`/`probe_duration_seconds` 的可選 Prometheus 文本文件目標。
- `TRYIT_PROXY_PROBE_LABELS` — 附加到指標的以逗號分隔的 `key=value` 對（默認為 `job=tryit-proxy` 和 `instance=<proxy URL>`）。
- `TRYIT_PROXY_PROBE_METRICS_URL` — 啟用 `TRYIT_PROXY_METRICS_LISTEN` 時必須成功響應的可選指標端點 URL（例如 `http://localhost:9798/metrics`）。

通過將探針指向可寫的位置，將結果輸入到文本文件收集器中
路徑（例如，`/var/lib/node_exporter/textfile_collector/tryit.prom`）和
添加任何自定義標籤：

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" \
npm run probe:tryit-proxy
```

該腳本自動重寫指標文件，以便您的收集器始終讀取
完整的有效負載。

配置 `TRYIT_PROXY_METRICS_LISTEN` 時，設置
`TRYIT_PROXY_PROBE_METRICS_URL` 到指標端點，以便探測快速失敗
如果刮擦表面消失（例如，入口配置錯誤或缺失
防火牆規則）。典型的生產設置是
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`。

對於輕量級警報，請將探測器連接到監控堆棧。 Prometheus
連續兩次失敗後進行分頁的示例：

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### 指標端點和儀表板

之前設置 `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798`（或任何主機/端口對）
啟動代理以公開 Prometheus 格式的指標端點。路徑
默認為 `/metrics` 但可以通過以下方式覆蓋
`TRYIT_PROXY_METRICS_PATH=/custom`。每次抓取都會返回每個方法的計數器
請求總數、速率限制拒絕、上游錯誤/超時、代理結果、
和延遲摘要：

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

將您的 Prometheus/OTLP 收集器指向指標端點並重用
現有 `dashboards/grafana/docs_portal.json` 面板，以便 SRE 可以觀察尾部
在不解析日誌的情況下，延遲和拒絕峰值。自動代理
發布 `tryit_proxy_start_timestamp_ms` 以幫助操作員檢測重啟。

### 回滾自動化

使用管理幫助程序更新或恢復目標 Torii URL。劇本
將以前的配置存儲在 `.env.tryit-proxy.bak` 中，因此回滾是
單個命令。

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

如果您的部署使用 `--env` 或 `TRYIT_PROXY_ENV` 覆蓋 env 文件路徑
將配置存儲在其他地方。