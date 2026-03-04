---
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b920e21b96436755f7d37f7b5577465cb3e30016d36340c50f7c6f3a9a46919
source_last_modified: "2025-12-29T18:16:35.116499+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 尝试一下沙盒

开发者门户提供了一个可选的“Try it”控制台，因此您可以调用 Torii
端点无需离开文档。控制台转发请求
通过捆绑代理，浏览器可以绕过 CORS 限制，同时仍然
实施速率限制和身份验证。

## 先决条件

- Node.js 18.18 或更高版本（符合门户构建要求）
- 对 Torii 暂存环境的网络访问
- 不记名令牌，可以调用您计划行使的 Torii 路线

所有代理配置都是通过环境变量完成的。下表
列出了最重要的旋钮：

|变量|目的|默认 |
| ---| ---| ---|
| `TRYIT_PROXY_TARGET` |代理将请求转发到的基本 Torii URL | **必填** |
| `TRYIT_PROXY_LISTEN` |本地开发监听地址（格式`host:port`或`[ipv6]:port`）| `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` |可能调用代理的来源的逗号分隔列表 | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` |每个上游请求的标识符都放置在 `X-TryIt-Client` 中 | `docs-portal` |
| `TRYIT_PROXY_BEARER` |默认不记名令牌转发至 Torii | _空_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` |允许最终用户通过 `X-TryIt-Auth` 提供自己的代币 | `0` |
| `TRYIT_PROXY_MAX_BODY` |最大请求正文大小（字节）| `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` |上行超时（以毫秒为单位）| `10000` |
| `TRYIT_PROXY_RATE_LIMIT` |每个客户端 IP 每个速率窗口允许的请求数 | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` |速率限制滑动窗口（毫秒）| `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus 样式指标端点的可选侦听地址（`host:port` 或 `[ipv6]:port`）| _空（已禁用）_ |
| `TRYIT_PROXY_METRICS_PATH` |指标端点提供的 HTTP 路径 | `/metrics` |

该代理还公开 `GET /healthz`，返回结构化 JSON 错误，并且
从日志输出中编辑不记名令牌。

向文档用户公开代理时启用 `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`，以便 Swagger 和
RapiDoc 面板可以转发用户提供的不记名令牌。代理仍然执行速率限制，
编辑凭据，并记录请求是使用默认令牌还是每个请求覆盖。
将 `TRYIT_PROXY_CLIENT_ID` 设置为您想要作为 `X-TryIt-Client` 发送的标签
（默认为 `docs-portal`）。代理修剪并验证调用者提供的
`X-TryIt-Client` 值，回退到此默认值，以便临时网关可以
无需关联浏览器元数据即可审核来源。

## 本地启动代理

首次设置门户时安装依赖项：

```bash
cd docs/portal
npm install
```

运行代理并将其指向您的 Torii 实例：

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

该脚本记录绑定地址并将来自 `/proxy/*` 的请求转发到
配置 Torii 原点。

在绑定套接字之前，脚本会验证
`static/openapi/torii.json` 与中记录的摘要匹配
`static/openapi/manifest.json`。如果文件发生漂移，该命令将退出并显示
错误并指示您运行 `npm run sync-openapi -- --latest`。出口
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` 仅用于紧急超越；代理将
记录警告并继续，以便您可以在维护时段内恢复。

## 连接门户小部件

当您构建或提供开发人员门户时，请设置小部件所使用的 URL
应该用于代理：

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

以下组件从 `docusaurus.config.js` 读取这些值：

- **Swagger UI** — 在 `/reference/torii-swagger` 处渲染；预授权
  持有者方案当存在令牌时，用 `X-TryIt-Client` 标记请求，
  注入 `X-TryIt-Auth`，并在以下情况下通过代理重写调用
  `TRYIT_PROXY_PUBLIC_URL` 已设置。
- **RapiDoc** — 在 `/reference/torii-rapidoc` 渲染；镜像令牌字段，
  重用与 Swagger 面板相同的标头，并以代理为目标
  配置 URL 时自动。
- **尝试控制台** — 嵌入 API 概述页面；让您发送自定义
  请求、查看标头并检查响应正文。

两个面板上都有一个**快照选择器**，内容为
`docs/portal/static/openapi/versions.json`。将该索引填充为
`npm run sync-openapi -- --version=<label> --mirror=current --latest`所以
审阅者可以在历史规范之间跳转，查看记录的 SHA-256 摘要，
并在使用前确认发布快照是否带有签名清单
交互式小部件。

更改任何小部件中的令牌只会影响当前浏览器会话；的
代理永远不会保留或记录提供的令牌。

## 短暂的 OAuth 令牌

为了避免将长期存在的 Torii 令牌分发给审阅者，请连接 Try it
控制台到您的 OAuth 服务器。当存在以下环境变量时
门户呈现设备代码登录小部件，铸造短期不记名令牌，
并自动将它们注入到控制台表单中。

|变量|目的|默认 |
| ---| ---| ---|
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth 设备授权端点 (`/oauth/device/code`) | _空（已禁用）_ |
| `DOCS_OAUTH_TOKEN_URL` |接受 `grant_type=urn:ietf:params:oauth:grant-type:device_code` 的令牌端点 | _空_ |
| `DOCS_OAUTH_CLIENT_ID` |为文档预览注册的 OAuth 客户端标识符 | _空_ |
| `DOCS_OAUTH_SCOPE` |登录期间请求的以空格分隔的范围 | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` |将令牌绑定到的可选 API 受众 | _空_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` |等待批准时的最小轮询间隔（毫秒） | `5000`（<5000ms 的值被拒绝）|
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` |后备设备代码过期窗口（秒）| `600`（必须保持在 300 到 900 之间）|
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` |后备访问令牌生命周期（秒）| `900`（必须保持在 300 到 900 之间）|
| `DOCS_OAUTH_ALLOW_INSECURE` |设置为 `1` 用于有意跳过 OAuth 强制执行的本地预览 | _取消设置_ |

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

当您运行 `npm run start` 或 `npm run build` 时，门户会嵌入这些值
在 `docusaurus.config.js` 中。在本地预览期间，尝试卡会显示
“使用设备代码登录”按钮。用户在您的 OAuth 上输入显示的代码
验证页面；一旦设备流程成功，小部件就会：

- 将颁发的不记名令牌注入 Try it 控制台字段，
- 使用现有的 `X-TryIt-Client` 和 `X-TryIt-Auth` 标头标记请求，
- 显示剩余寿命，以及
- 令牌过期时自动清除。

手动承载输入仍然可用 - 无论何时您都可以忽略 OAuth 变量
想要强制审阅者自己粘贴临时令牌，或导出
`DOCS_OAUTH_ALLOW_INSECURE=1` 用于匿名访问的隔离本地预览
是可以接受的。未配置 OAuth 的构建现在无法快速满足
DOCS-1b 路线图门。

📌 查看[安全强化和渗透测试清单](./security-hardening.md)
在将门户暴露在实验室之外之前；它记录了威胁模型，
CSP/可信类型配置文件，以及现在用于 DOCS-1b 的渗透测试步骤。

## Norito-RPC 样本

Norito-RPC 请求与 JSON 路由共享相同的代理和 OAuth 管道，
他们只需设置 `Content-Type: application/x-norito` 并发送
NRPC 规范中描述的预编码 Norito 有效负载
（`docs/source/torii/nrpc_spec.md`）。
该存储库在 `fixtures/norito_rpc/` 下提供规范的有效负载，因此门户
作者、SDK 所有者和审阅者可以重放 CI 使用的确切字节。

### 从 Try It 控制台发送 Norito 有效负载

1. 选择一个夹具，例如 `fixtures/norito_rpc/transfer_asset.norito`。这些
   文件是原始 Norito 信封； **不要**对它们进行 base64 编码。
2. 在 Swagger 或 RapiDoc 中，找到 NRPC 端点（例如
   `POST /v1/pipeline/submit`）并将 **Content-Type** 选择器切换为
   `application/x-norito`。
3. 将请求正文编辑器切换为 **binary**（Swagger 的“文件”模式或
   RapiDoc 的“二进制/文件”选择器）并上传 `.norito` 文件。小部件
   通过代理流式传输字节而不进行任何更改。
4. 提交请求。如果 Torii 返回 `X-Iroha-Error-Code: schema_mismatch`，
   验证您正在调用接受二进制有效负载的端点并且
   确认 `fixtures/norito_rpc/schema_hashes.json` 中记录的模式哈希
   与您正在使用的 Torii 版本匹配。

控制台将最新的文件保留在内存中，以便您可以重新提交相同的文件
有效负载同时使用不同的授权令牌或 Torii 主机。添加
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` 到您的工作流程产生
NRPC-4 采用计划中引用的证据包（日志 + JSON 摘要），
这与在评论期间截屏“尝试一下”响应非常搭配。

### CLI 示例 (curl)

相同的赛程可以通过 `curl` 在门户外重播，这很有用
验证代理或调试网关响应时：

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl \
  -H "Content-Type: application/x-norito" \
  -H "Authorization: ${TOKEN}" \
  --data-binary @fixtures/norito_rpc/transfer_asset.norito \
  "${TORII}/v1/pipeline/submit"
```

将夹具替换为 `transaction_fixtures.manifest.json` 中列出的任何条目
或者使用 `cargo xtask norito-rpc-fixtures` 编码您自己的有效负载。当 Torii
处于金丝雀模式，您可以将 `curl` 指向 try-it 代理
(`https://docs.sora.example/proxy/v1/pipeline/submit`) 进行同样的练习
门户小部件使用的基础设施。

## 可观察性和操作每个请求都会记录一次，其中包括方法、路径、来源、上游状态和
身份验证源（`override`、`default` 或 `client`）。代币从来都不是
存储 — 承载标头和 `X-TryIt-Auth` 值均在之前经过编辑
日志记录——这样你就可以将标准输出转发到中央收集器，而不必担心
秘密泄露。

### 健康探测和警报

在部署期间或按计划运行捆绑探针：

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

环境旋钮：

- `TRYIT_PROXY_SAMPLE_PATH` — 可选的 Torii 路线（无 `/proxy`）进行锻炼。
- `TRYIT_PROXY_SAMPLE_METHOD` — 默认为 `GET`；设置为 `POST` 用于写入路由。
- `TRYIT_PROXY_PROBE_TOKEN` — 为示例调用注入临时承载令牌。
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — 覆盖默认的 5 秒超时。
- `TRYIT_PROXY_PROBE_METRICS_FILE` — `probe_success`/`probe_duration_seconds` 的可选 Prometheus 文本文件目标。
- `TRYIT_PROXY_PROBE_LABELS` — 附加到指标的以逗号分隔的 `key=value` 对（默认为 `job=tryit-proxy` 和 `instance=<proxy URL>`）。
- `TRYIT_PROXY_PROBE_METRICS_URL` — 启用 `TRYIT_PROXY_METRICS_LISTEN` 时必须成功响应的可选指标端点 URL（例如 `http://localhost:9798/metrics`）。

通过将探针指向可写的位置，将结果输入到文本文件收集器中
路径（例如，`/var/lib/node_exporter/textfile_collector/tryit.prom`）和
添加任何自定义标签：

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" \
npm run probe:tryit-proxy
```

该脚本自动重写指标文件，以便您的收集器始终读取
完整的有效负载。

配置 `TRYIT_PROXY_METRICS_LISTEN` 时，设置
`TRYIT_PROXY_PROBE_METRICS_URL` 到指标端点，以便探测快速失败
如果刮擦表面消失（例如，入口配置错误或缺失
防火墙规则）。典型的生产设置是
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`。

对于轻量级警报，请将探测器连接到监控堆栈。 Prometheus
连续两次失败后进行分页的示例：

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

### 指标端点和仪表板

之前设置 `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798`（或任何主机/端口对）
启动代理以公开 Prometheus 格式的指标端点。路径
默认为 `/metrics` 但可以通过以下方式覆盖
`TRYIT_PROXY_METRICS_PATH=/custom`。每次抓取都会返回每个方法的计数器
请求总数、速率限制拒绝、上游错误/超时、代理结果、
和延迟摘要：

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

将您的 Prometheus/OTLP 收集器指向指标端点并重用
现有 `dashboards/grafana/docs_portal.json` 面板，以便 SRE 可以观察尾部
在不解析日志的情况下，延迟和拒绝峰值。自动代理
发布 `tryit_proxy_start_timestamp_ms` 以帮助操作员检测重启。

### 回滚自动化

使用管理帮助程序更新或恢复目标 Torii URL。剧本
将以前的配置存储在 `.env.tryit-proxy.bak` 中，因此回滚是
单个命令。

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

如果您的部署使用 `--env` 或 `TRYIT_PROXY_ENV` 覆盖 env 文件路径
将配置存储在其他地方。