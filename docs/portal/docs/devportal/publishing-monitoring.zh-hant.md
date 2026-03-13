---
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f68e8cc639bd6a780c33fd14ab4e25df1c6e9381595c7a4c44ff577fea02400d
source_last_modified: "2026-01-22T16:26:46.494851+00:00"
translation_last_reviewed: 2026-02-07
id: publishing-monitoring
title: SoraFS Publishing & Monitoring
sidebar_label: Publishing & Monitoring
description: Capture the end-to-end monitoring flow for SoraFS portal releases so DOCS-3c has deterministic probes, telemetry, and evidence bundles.
translator: machine-google-reviewed
---

路線圖項目 **DOCS-3c** 需要的不僅僅是包裝清單：在每個
SoraFS發布我們必須不斷證明開發者門戶，嘗試一下
代理和網關綁定保持健康。該頁面記錄了監控
[部署指南](./deploy-guide.md) 附帶的表面，所以 CI 等等
呼叫工程師可以執行與運營部門用於強制執行 SLO 相同的檢查。

## 管道回顧

1. **構建並簽名** – 按照[部署指南](./deploy-guide.md)運行
   `npm run build`、`scripts/preview_wave_preflight.sh` 和 Sigstore +
   清單提交步驟。預檢腳本發出 `preflight-summary.json`
   因此每個預覽都帶有構建/鏈接/探測元數據。
2. **固定並驗證** – `sorafs_cli manifest submit`、`cargo xtask soradns-verify-binding`、
   DNS 切換計劃為治理提供了確定性的人工製品。
3. **存檔證據** – 存儲 CAR 摘要、Sigstore 捆綁包、別名證明、
   探針輸出，以及 `docs_portal.json` 儀表板快照
   `artifacts/sorafs/<tag>/`。

## 監控通道

### 1. 發布監視器 (`scripts/monitor-publishing.mjs`)

新的 `npm run monitor:publishing` 命令包裝了門戶探針，嘗試一下
代理探針，並將驗證器綁定到單個 CI 友好的檢查中。提供一個
JSON 配置（簽入 CI 機密或 `configs/docs_monitor.json`）並運行：

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

添加 `--prom-out ../../artifacts/docs_monitor/monitor.prom`（並且可選
`--prom-job docs-preview`) 發出 Prometheus 文本格式指標，適用於
Pushgateway 上傳或直接在暫存/生產中抓取 Prometheus。的
指標鏡像 JSON 摘要，以便 SLO 儀表板和警報規則可以跟踪
門戶、嘗試一下、綁定和 DNS 運行狀況，無需解析證據包。

具有所需旋鈕和多個綁定的示例配置：

```json
{
  "portal": {
    "baseUrl": "https://docs-preview.sora.link",
    "paths": ["/", "/devportal/try-it", "/reference/torii-swagger"],
    "expectRelease": "preview-2026-02-14",
    "checkSecurity": true,
    "expectedSecurity": {
      "csp": "default-src 'self'; connect-src https://tryit-preview.sora",
      "permissionsPolicy": "fullscreen=()",
      "referrerPolicy": "strict-origin-when-cross-origin"
    }
  },
  "tryIt": {
    "proxyUrl": "https://tryit-preview.sora",
    "samplePath": "/proxy/v2/accounts/i105.../assets?limit=1",
    "method": "GET",
    "timeoutMs": 7000,
    "token": "${TRYIT_BEARER}",
    "metricsUrl": "https://tryit-preview.sora/metrics"
  },
  "bindings": [
    {
      "label": "portal",
      "bindingPath": "../../artifacts/sorafs/portal.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal.manifest.json"
    },
    {
      "label": "openapi",
      "bindingPath": "../../artifacts/sorafs/openapi.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/openapi.manifest.json"
    },
    {
      "label": "portal-sbom",
      "bindingPath": "../../artifacts/sorafs/portal-sbom.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal-sbom.manifest.json"
    }
  ],

  "dns": [
    {
      "label": "docs-preview CNAME",
      "hostname": "docs-preview.sora.link",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    },
    {
      "label": "docs-preview canonical",
      "hostname": "igjssx53t4ayu3d5qus5o6xtp2f5dvka5rewr6xgscpmh3x4io4q.gw.sora.id",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    }
  ]
}
```

監視器寫入 JSON 摘要（S3/SoraFS 友好）並在以下情況下退出非零值：
任何探測失敗，使其適合 Cron 作業、Buildkite 步驟或
Alertmanager Webhooks。通過 `--evidence-dir` 仍然存在 `summary.json`，
`portal.json`、`tryit.json` 和 `binding.json` 以及 `checksums.sha256`
清單，以便治理審查者可以區分監控結果，而無需
重新運行探針。

> **TLS 護欄：** `monitorPortal` 拒絕 `http://` 基本 URL，除非您設置
> 配置中的 `allowInsecureHttp: true`。保持生產/分段探針開啟
> HTTPS；選擇加入僅適用於本地預覽。

每個綁定條目針對捕獲的數據運行 `cargo xtask soradns-verify-binding`
`portal.gateway.binding.json` 捆綁包（和可選的 `manifestJson`）所以別名，
證據狀態和內容 CID 與已發布的證據保持一致。的
可選的 `hostname` 防護確認別名派生的規範主機與
您打算升級的網關主機，防止偏離的 DNS 切換
記錄綁定。

可選的 `dns` 塊將 DOCS-7 的 SoraDNS 部署連接到同一顯示器。
每個條目解析一個主機名/記錄類型對（例如
`docs-preview.sora.link` → `docs-preview.sora.link.gw.sora.name` CNAME) 和
確認答案與 `expectedRecords` 或 `expectedIncludes` 匹配。第二個
上面代碼片段中的條目硬編碼了由以下方法生成的規范哈希主機名
`cargo xtask soradns-hosts --name docs-preview.sora.link`；監視器現在證明
人類友好的別名和規范哈希 (`igjssx53…gw.sora.id`)
解決固定的漂亮主機。這使得 DNS 升級證據自動化：
如果任一主機發生漂移，即使 HTTP 綁定仍然存在，監視器也會失敗
裝訂正確的清單。

### 2. OpenAPI版本清單防護

DOCS-2b 的“簽名 OpenAPI 清單”要求現在提供自動防護：
`ci/check_openapi_spec.sh` 調用 `npm run check:openapi-versions`，它調用
`scripts/verify-openapi-versions.mjs` 進行交叉檢查
`docs/portal/static/openapi/versions.json` 與實際 Torii 規格和
體現出來。警衛核實：

- `versions.json` 中列出的每個版本在下面都有一個匹配的目錄
  `static/openapi/versions/`。
- 每個條目的 `bytes` 和 `sha256` 字段與磁盤上的規範文件匹配。
- `latest` 別名鏡像 `current` 條目（摘要/大小/簽名元數據）
  所以默認下載不能漂移。
- 簽名條目引用了一個清單，其 `artifact.path` 指向
  相同的規範，其簽名/公鑰十六進制值與清單匹配。

每當您鏡像新規範時，請在本地運行防護：

```bash
cd docs/portal
npm run check:openapi-versions
```

失敗消息包括陳舊文件提示 (`npm run sync-openapi -- --latest`)
因此門戶貢獻者知道如何刷新快照。把守衛留在裡面
CI 阻止門戶發布已簽名的清單和已發布的摘要
不同步。

### 2. 儀表板和警報

- **`dashboards/grafana/docs_portal.json`** – DOCS-3c 主板。面板
  跟踪 `torii_sorafs_gateway_refusals_total`，複製 SLA 未命中，嘗試一下
  代理錯誤和探測延遲（`docs.preview.integrity` 覆蓋）。導出
  每次發布後登機並將其附在操作票上。
- **嘗試代理警報** – Alertmanager 規則 `TryItProxyErrors` 觸發
  `probe_success{job="tryit-proxy"}` 持續下降或
  `tryit_proxy_requests_total{status="error"}` 尖峰。
- **網關 SLO** – `DocsPortal/GatewayRefusals` 確保別名綁定繼續
  公佈固定的清單摘要；升級鏈接至
  `cargo xtask soradns-verify-binding` 在發布期間捕獲的 CLI 記錄。

### 3. 證據追踪

每次監控運行應附加：

- `monitor-publishing` 證據包（`summary.json`、每個部分的文件和
  `checksums.sha256`）。
- `docs_portal` 板在發布窗口上的 Grafana 屏幕截圖。
- 嘗試代理更改/回滾記錄（`npm run manage:tryit-proxy` 日誌）。
- `cargo xtask soradns-verify-binding` 的別名驗證輸出。

將它們存儲在 `artifacts/sorafs/<tag>/monitoring/` 下並將它們鏈接到
發布問題，以便審計跟踪在 CI 日誌過期後仍然存在。

## 操作清單

1. 通過Step7運行部署指南。
2.使用生產配置執行`npm run monitor:publishing`；存檔
   JSON 輸出。
3. 捕獲 Grafana 面板（`docs_portal`、`TryItProxyErrors`、
   `DocsPortal/GatewayRefusals`）並將其附加到發行票上。
4. 安排定期監控（建議：每 15 分鐘一次），指向
   具有相同配置的生產 URL 以滿足 DOCS-3c SLO 門。
5. 發生事件時，重新運行監控命令 `--json-out` 進行記錄
   證據之前/之後並將其附加到屍檢中。

遵循此循環將關閉 DOCS-3c：門戶構建流程、發布管道、
監控堆棧現在位於具有可重現命令的單個劇本中，
示例配置和遙測掛鉤。