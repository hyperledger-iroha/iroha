---
id: publishing-monitoring
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Publishing & Monitoring
sidebar_label: Publishing & Monitoring
description: Capture the end-to-end monitoring flow for SoraFS portal releases so DOCS-3c has deterministic probes, telemetry, and evidence bundles.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

路线图项目 **DOCS-3c** 需要的不仅仅是包装清单：在每个
SoraFS发布我们必须不断证明开发者门户，尝试一下
代理和网关绑定保持健康。该页面记录了监控
[部署指南](./deploy-guide.md) 附带的表面，所以 CI 等等
呼叫工程师可以执行与运营部门用于强制执行 SLO 相同的检查。

## 管道回顾

1. **构建并签名** – 按照[部署指南](./deploy-guide.md)运行
   `npm run build`、`scripts/preview_wave_preflight.sh` 和 Sigstore +
   清单提交步骤。预检脚本发出 `preflight-summary.json`
   因此每个预览都带有构建/链接/探测元数据。
2. **固定并验证** – `sorafs_cli manifest submit`、`cargo xtask soradns-verify-binding`、
   DNS 切换计划为治理提供了确定性的人工制品。
3. **存档证据** – 存储 CAR 摘要、Sigstore 捆绑包、别名证明、
   探针输出，以及 `docs_portal.json` 仪表板快照
   `artifacts/sorafs/<tag>/`。

## 监控通道

### 1. 发布监视器 (`scripts/monitor-publishing.mjs`)

新的 `npm run monitor:publishing` 命令包装了门户探针，尝试一下
代理探针，并将验证器绑定到单个 CI 友好的检查中。提供一个
JSON 配置（签入 CI 机密或 `configs/docs_monitor.json`）并运行：

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

添加 `--prom-out ../../artifacts/docs_monitor/monitor.prom`（并且可选
`--prom-job docs-preview`) 发出 Prometheus 文本格式指标，适用于
Pushgateway 上传或直接在暂存/生产中抓取 Prometheus。的
指标镜像 JSON 摘要，以便 SLO 仪表板和警报规则可以跟踪
门户、尝试一下、绑定和 DNS 运行状况，无需解析证据包。

具有所需旋钮和多个绑定的示例配置：

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
    "samplePath": "/proxy/v1/accounts/i105.../assets?limit=1",
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

监视器写入 JSON 摘要（S3/SoraFS 友好）并在以下情况下退出非零值：
任何探测失败，使其适合 Cron 作业、Buildkite 步骤或
Alertmanager Webhooks。通过 `--evidence-dir` 仍然存在 `summary.json`，
`portal.json`、`tryit.json` 和 `binding.json` 以及 `checksums.sha256`
清单，以便治理审查者可以区分监控结果，而无需
重新运行探针。

> **TLS 护栏：** `monitorPortal` 拒绝 `http://` 基本 URL，除非您设置
> 配置中的 `allowInsecureHttp: true`。保持生产/分段探针开启
> HTTPS；选择加入仅适用于本地预览。

每个绑定条目针对捕获的数据运行 `cargo xtask soradns-verify-binding`
`portal.gateway.binding.json` 捆绑包（和可选的 `manifestJson`）所以别名，
证据状态和内容 CID 与已发布的证据保持一致。的
可选的 `hostname` 防护确认别名派生的规范主机与
您打算升级的网关主机，防止偏离的 DNS 切换
记录绑定。

可选的 `dns` 块将 DOCS-7 的 SoraDNS 部署连接到同一显示器。
每个条目解析一个主机名/记录类型对（例如
`docs-preview.sora.link` → `docs-preview.sora.link.gw.sora.name` CNAME) 和
确认答案与 `expectedRecords` 或 `expectedIncludes` 匹配。第二个
上面代码片段中的条目硬编码了由以下方法生成的规范哈希主机名
`cargo xtask soradns-hosts --name docs-preview.sora.link`；监视器现在证明
人类友好的别名和规范哈希 (`igjssx53…gw.sora.id`)
解决固定的漂亮主机。这使得 DNS 升级证据自动化：
如果任一主机发生漂移，即使 HTTP 绑定仍然存在，监视器也会失败
装订正确的清单。

### 2. OpenAPI版本清单防护

DOCS-2b 的“签名 OpenAPI 清单”要求现在提供自动防护：
`ci/check_openapi_spec.sh` 调用 `npm run check:openapi-versions`，它调用
`scripts/verify-openapi-versions.mjs` 进行交叉检查
`docs/portal/static/openapi/versions.json` 与实际 Torii 规格和
体现出来。警卫核实：

- `versions.json` 中列出的每个版本在下面都有一个匹配的目录
  `static/openapi/versions/`。
- 每个条目的 `bytes` 和 `sha256` 字段与磁盘上的规范文件匹配。
- `latest` 别名镜像 `current` 条目（摘要/大小/签名元数据）
  所以默认下载不能漂移。
- 签名条目引用了一个清单，其 `artifact.path` 指向
  相同的规范，其签名/公钥十六进制值与清单匹配。

每当您镜像新规范时，请在本地运行防护：

```bash
cd docs/portal
npm run check:openapi-versions
```

失败消息包括陈旧文件提示 (`npm run sync-openapi -- --latest`)
因此门户贡献者知道如何刷新快照。把守卫留在里面
CI 阻止门户发布已签名的清单和已发布的摘要
不同步。

### 2. 仪表板和警报

- **`dashboards/grafana/docs_portal.json`** – DOCS-3c 主板。面板
  跟踪 `torii_sorafs_gateway_refusals_total`，复制 SLA 未命中，尝试一下
  代理错误和探测延迟（`docs.preview.integrity` 覆盖）。导出
  每次发布后登机并将其附在操作票上。
- **尝试代理警报** – Alertmanager 规则 `TryItProxyErrors` 触发
  `probe_success{job="tryit-proxy"}` 持续下降或
  `tryit_proxy_requests_total{status="error"}` 尖峰。
- **网关 SLO** – `DocsPortal/GatewayRefusals` 确保别名绑定继续
  公布固定的清单摘要；升级链接至
  `cargo xtask soradns-verify-binding` 在发布期间捕获的 CLI 记录。

### 3. 证据追踪

每次监控运行应附加：

- `monitor-publishing` 证据包（`summary.json`、每个部分的文件和
  `checksums.sha256`）。
- `docs_portal` 板在发布窗口上的 Grafana 屏幕截图。
- 尝试代理更改/回滚记录（`npm run manage:tryit-proxy` 日志）。
- `cargo xtask soradns-verify-binding` 的别名验证输出。

将它们存储在 `artifacts/sorafs/<tag>/monitoring/` 下并将它们链接到
发布问题，以便审计跟踪在 CI 日志过期后仍然存在。

## 操作清单

1. 通过Step7运行部署指南。
2.使用生产配置执行`npm run monitor:publishing`；存档
   JSON 输出。
3. 捕获 Grafana 面板（`docs_portal`、`TryItProxyErrors`、
   `DocsPortal/GatewayRefusals`）并将其附加到发行票上。
4. 安排定期监控（建议：每 15 分钟一次），指向
   具有相同配置的生产 URL 以满足 DOCS-3c SLO 门。
5. 发生事件时，重新运行监控命令 `--json-out` 进行记录
   证据之前/之后并将其附加到尸检中。

遵循此循环将关闭 DOCS-3c：门户构建流程、发布管道、
监控堆栈现在位于具有可重现命令的单个剧本中，
示例配置和遥测挂钩。