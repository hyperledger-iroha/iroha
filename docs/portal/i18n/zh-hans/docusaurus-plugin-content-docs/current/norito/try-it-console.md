---
lang: zh-hans
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito Try-It Console
description: Use the developer-portal proxy, Swagger, and RapiDoc widgets to send real Torii / Norito-RPC requests directly from the documentation site.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

该门户捆绑了三个交互式界面，将流量中继到 Torii：

- `/reference/torii-swagger` 处的 **Swagger UI** 呈现签名的 OpenAPI 规范，并在设置 `TRYIT_PROXY_PUBLIC_URL` 时自动通过代理重写请求。
- `/reference/torii-rapidoc` 的 **RapiDoc** 公开了与文件上传和内容类型选择器相同的架构，适用于 `application/x-norito`。
- Norito 概述页面上的 **Try it sandbox** 为临时 REST 请求和 OAuth 设备登录提供了轻量级表单。

所有三个小部件都将请求发送到本地 **Try-It 代理** (`docs/portal/scripts/tryit-proxy.mjs`)。代理验证 `static/openapi/torii.json` 是否与 `static/openapi/manifest.json` 中的签名摘要匹配，实施速率限制器，编辑日志中的 `X-TryIt-Auth` 标头，并使用 `X-TryIt-Client` 标记每个上游调用，以便 Torii 操作员可以审核流量源。

## 启动代理

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
- `TRYIT_PROXY_ALLOWED_ORIGINS` 必须包含应嵌入控制台的每个门户源（本地开发服务器、生产主机名、预览 URL）。
- `TRYIT_PROXY_PUBLIC_URL` 由 `docusaurus.config.js` 消耗并通过 `customFields.tryIt` 注入到小部件中。
- `TRYIT_PROXY_BEARER`仅在`DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`时加载；否则，用户必须通过控制台或 OAuth 设备流提供自己的令牌。
- `TRYIT_PROXY_CLIENT_ID` 设置每个请求携带的 `X-TryIt-Client` 标签。
  允许从浏览器提供 `X-TryIt-Client`，但值会被修剪
  如果它们包含控制字符则被拒绝。

启动时，代理运行 `verifySpecDigest`，如果清单已过时，则退出并显示修复提示。运行 `npm run sync-openapi -- --latest` 下载最新的 Torii 规范或通过 `TRYIT_PROXY_ALLOW_STALE_SPEC=1` 进行紧急覆盖。

要更新或回滚代理目标而不手动编辑环境文件，请使用帮助程序：

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## 连接小部件

代理监听后为门户提供服务：

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` 公开了以下旋钮：

|变量|目的|
| ---| ---|
| `TRYIT_PROXY_PUBLIC_URL` | URL 注入到 Swagger、RapiDoc 和 Try it 沙箱中。保持未设置状态可在未经授权的预览期间隐藏小部件。 |
| `TRYIT_PROXY_DEFAULT_BEARER` |可选的默认令牌存储在内存中。需要 `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` 和仅 HTTPS CSP 防护 (DOCS-1b)，除非您在本地传递 `DOCS_SECURITY_ALLOW_INSECURE=1`。 |
| `DOCS_OAUTH_*` |启用 OAuth 设备流（`OAuthDeviceLogin` 组件），以便审阅者可以在不离开门户的情况下创建短期令牌。 |

当 OAuth 变量存在时，沙箱会呈现一个 **使用设备代码登录** 按钮，该按钮遍历配置的身份验证服务器（有关确切形状，请参阅 `config/security-helpers.js`）。通过设备流发出的令牌仅缓存在浏览器会话中。

## 发送 Norito-RPC 有效负载

1. 使用 [Norito 快速入门](./quickstart.md) 中描述的 CLI 或代码片段构建 `.norito` 有效负载。代理转发 `application/x-norito` 主体不变，因此您可以重复使用与 `curl` 一起发布的相同工件。
2. 打开 `/reference/torii-rapidoc`（二进制有效负载首选）或 `/reference/torii-swagger`。
3. 从下拉列表中选择所需的 Torii 快照。快照已签名；该面板显示 `static/openapi/manifest.json` 中记录的清单摘要。
4. 在“Try it”抽屉中选择 `application/x-norito` 内容类型，单击 **选择文件**，然后选择您的负载。代理将请求重写为 `/proxy/v1/pipeline/submit` 并用 `X-TryIt-Client=docs-portal-rapidoc` 对其进行标记。
5. 要下载 Norito 响应，请设置 `Accept: application/x-norito`。 Swagger/RapiDoc 在同一个抽屉中公开标头选择器，并通过代理将二进制文件流回。

对于纯 JSON 路由，嵌入式 Try it 沙箱通常更快：输入路径（例如 `/v1/accounts/soraカタカナ.../assets`），选择 HTTP 方法，在需要时粘贴 JSON 正文，然后点击 **发送请求** 以内联检查标头、持续时间和有效负载。

## 故障排除

|症状|可能的原因 |修复|
| ---| ---| ---|
|浏览器控制台显示 CORS 错误或沙箱警告代理 URL 丢失。 |代理未运行或源未列入白名单。 |启动代理，确保 `TRYIT_PROXY_ALLOWED_ORIGINS` 覆盖您的门户主机，然后重新启动 `npm run start`。 |
| `npm run tryit-proxy` 退出并显示“摘要不匹配”。 | Torii OpenAPI 捆绑包已更改为上游。 |运行 `npm run sync-openapi -- --latest`（或 `--version=<tag>`）并重试。 |
|小组件返回 `401` 或 `403`。 |令牌丢失、过期或范围不足。 |使用 OAuth 设备流或将有效的承载令牌粘贴到沙箱中。对于静态令牌，您必须导出 `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`。 |
|来自代理的 `429 Too Many Requests`。 |超出每个 IP 的速率限制。 |为可信环境或限制测试脚本提高 `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS`。所有速率限制拒绝都会增加 `tryit_proxy_rate_limited_total`。 |

## 可观察性

- `npm run probe:tryit-proxy`（`scripts/tryit-proxy-probe.mjs` 的包装）调用 `/healthz`，可选择执行示例路由，并为 `probe_success` / `probe_duration_seconds` 发出 Prometheus 文本文件。配置 `TRYIT_PROXY_PROBE_METRICS_FILE` 以与 node_exporter 集成。
- 设置 `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` 以公开计数器（`tryit_proxy_requests_total`、`tryit_proxy_rate_limited_total`、`tryit_proxy_upstream_failures_total`）和延迟直方图。 `dashboards/grafana/docs_portal.json` 板读取这些指标以实施 DOCS-SORA SLO。
- 运行时日志位于标准输出上。每个条目包括请求 ID、上游状态、身份验证源（`default`、`override` 或 `client`）和持续时间；秘密在发布前被编辑。

如果您需要验证 `application/x-norito` 有效负载是否达到 Torii 不变，请运行 Jest 套件 (`npm test -- tryit-proxy`) 或检查 `docs/portal/scripts/__tests__/tryit-proxy.test.mjs` 下的装置。回归测试涵盖压缩的 Norito 二进制文件、签名的 OpenAPI 清单和代理降级路径，以便 NRPC 推出保留永久的证据跟踪。