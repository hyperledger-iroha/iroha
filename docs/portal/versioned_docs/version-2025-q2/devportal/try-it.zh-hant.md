---
lang: zh-hant
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c389a2121f577bcf8893a0d5c0b898ec2ff5330f2f1727de3387da98f8369915
source_last_modified: "2025-12-29T18:16:35.904297+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
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
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` |代理將請求轉發到的基本 Torii URL | **必填** |
| `TRYIT_PROXY_LISTEN` |本地開發監聽地址（格式`host:port`或`[ipv6]:port`）| `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` |可能調用代理的來源的逗號分隔列表 | `http://localhost:3000` |
| `TRYIT_PROXY_BEARER` |默認不記名令牌轉發至 Torii | _空_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` |允許最終用戶通過 `X-TryIt-Auth` 提供自己的代幣 | `0` |
| `TRYIT_PROXY_MAX_BODY` |最大請求正文大小（字節）| `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` |上行超時（以毫秒為單位）| `10000` |
| `TRYIT_PROXY_RATE_LIMIT` |每個客戶端 IP 每個速率窗口允許的請求數 | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` |速率限制滑動窗口（毫秒）| `60000` |

該代理還公開 `GET /healthz`，返回結構化 JSON 錯誤，並且
從日誌輸出中編輯不記名令牌。

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
配置了 Torii 原點。

## 連接門戶小部件

當您構建或提供開發人員門戶時，請設置小部件所使用的 URL
應該用於代理：

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

以下組件從 `docusaurus.config.js` 讀取這些值：

- **Swagger UI** — 在 `/reference/torii-swagger` 處渲染；使用請求
  攔截器自動附加不記名令牌。
- **RapiDoc** — 在 `/reference/torii-rapidoc` 處呈現；鏡像 token 字段
  並支持針對代理的嘗試請求。
- **嘗試控制台** — 嵌入 API 概述頁面；讓您發送自定義
  請求、查看標頭並檢查響應正文。

更改任何小部件中的令牌只會影響當前瀏覽器會話；的
代理永遠不會保留或記錄提供的令牌。

## 可觀察性和操作

每個請求都會記錄一次，其中包括方法、路徑、來源、上游狀態和
身份驗證源（`override`、`default` 或 `client`）。代幣從來都不是
存儲 — 承載標頭和 `X-TryIt-Auth` 值均在之前經過編輯
日誌記錄——這樣你就可以將標準輸出轉發到中央收集器，而不必擔心
秘密洩露。

### 健康探測和警報在部署期間或按計劃運行捆綁探針：

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