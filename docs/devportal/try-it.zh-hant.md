---
lang: zh-hant
direction: ltr
source: docs/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 791d88296d9e52d3272ce3ac324e498fa3c622c323edc8c988302efe5092f0b4
source_last_modified: "2025-12-29T18:16:35.067551+00:00"
translation_last_reviewed: 2026-02-07
title: Try It Sandbox Guide
summary: How to run the Torii staging proxy and developer portal sandbox.
translator: machine-google-reviewed
---

開發人員門戶為 Torii REST API 提供了一個“嘗試”控制台。本指南
解釋如何啟動支持代理並將控制台連接到登台
網關而不暴露憑據。

## 先決條件

- Iroha 存儲庫簽出（工作空間根目錄）。
- Node.js 18.18+（與門戶基線匹配）。
- Torii 端點可從您的工作站（臨時或本地）訪問。

## 1. 生成 OpenAPI 快照（可選）

控制台重複使用與門戶參考頁面相同的 OpenAPI 負載。如果
您已更改 Torii 路由，請重新生成快照：

```bash
cargo xtask openapi
```

該任務寫入 `docs/portal/static/openapi/torii.json`。

## 2. 啟動 Try It 代理

從存儲庫根目錄：

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional defaults
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### 環境變量

|變量|描述 |
|----------|-------------|
| `TRYIT_PROXY_TARGET` | Torii 基本 URL（必需）。 |
| `TRYIT_PROXY_ALLOWED_ORIGINS` |允許使用代理的以逗號分隔的來源列表（默認為 `http://localhost:3000`）。 |
| `TRYIT_PROXY_BEARER` |可選的默認承載令牌應用於所有代理請求。 |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` |設置為 `1` 以逐字轉發調用者的 `Authorization` 標頭。 |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` |內存中速率限制器設置（默認值：每 60 秒 60 個請求）。 |
| `TRYIT_PROXY_MAX_BODY` |接受的最大請求負載（字節，默認 1MiB）。 |
| `TRYIT_PROXY_TIMEOUT_MS` | Torii 請求的上行超時（默認 10000 毫秒）。 |

代理暴露：

- `GET /healthz` — 準備情況檢查。
- `/proxy/*` — 代理請求，保留路徑和查詢字符串。

## 3.啟動門戶

在單獨的終端中：

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

訪問 `http://localhost:3000/api/overview` 並使用 Try It 控制台。一樣的
環境變量配置 Swagger UI 和 RapiDoc 嵌入。

## 4. 運行單元測試

該代理公開了一個快速的基於節點的測試套件：

```bash
npm run test:tryit-proxy
```

測試涵蓋地址解析、來源處理、速率限制和承載
注射。

## 5. 探測自動化和指標

使用捆綁探針驗證 `/healthz` 和示例端點：

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

環境旋鈕：

- `TRYIT_PROXY_SAMPLE_PATH` — 可選的 Torii 路線（無 `/proxy`）進行鍛煉。
- `TRYIT_PROXY_SAMPLE_METHOD` — 默認為 `GET`；設置為 `POST` 用於寫入路由。
- `TRYIT_PROXY_PROBE_TOKEN` — 為示例調用注入臨時承載令牌。
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — 覆蓋默認的 5 秒超時。
- `TRYIT_PROXY_PROBE_METRICS_FILE` — Prometheus `probe_success`/`probe_duration_seconds` 的文本文件目標。
- `TRYIT_PROXY_PROBE_LABELS` — 附加到度量的以逗號分隔的 `key=value` 對（默認為 `job=tryit-proxy` 和 `instance=<proxy URL>`）。

當設置 `TRYIT_PROXY_PROBE_METRICS_FILE` 時，腳本重寫文件
原子地，所以你的node_exporter/textfile收集器總是看到一個完整的
有效負載。示例：

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

將生成的指標轉發到 Prometheus 並在
當 `probe_success` 下降到 `0` 時，開發人員門戶文檔頁面。

## 6. 生產強化檢查表

在本地開發之外發布代理之前：

- 在代理（反向代理或託管網關）之前終止 TLS。
- 配置結構化日誌記錄並轉發到可觀察性管道。
- 輪換不記名令牌並將其存儲在您的秘密管理器中。
- 監控代理的 `/healthz` 端點和聚合延遲指標。
- 將速率限制與您的 Torii 暫存配額保持一致；調整`Retry-After`
  向客戶端傳達限制的行為。