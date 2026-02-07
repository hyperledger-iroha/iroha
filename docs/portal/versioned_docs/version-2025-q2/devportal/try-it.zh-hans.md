---
lang: zh-hans
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c389a2121f577bcf8893a0d5c0b898ec2ff5330f2f1727de3387da98f8369915
source_last_modified: "2025-12-29T18:16:35.904297+00:00"
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
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` |代理将请求转发到的基本 Torii URL | **必填** |
| `TRYIT_PROXY_LISTEN` |本地开发监听地址（格式`host:port`或`[ipv6]:port`）| `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` |可能调用代理的来源的逗号分隔列表 | `http://localhost:3000` |
| `TRYIT_PROXY_BEARER` |默认不记名令牌转发至 Torii | _空_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` |允许最终用户通过 `X-TryIt-Auth` 提供自己的代币 | `0` |
| `TRYIT_PROXY_MAX_BODY` |最大请求正文大小（字节）| `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` |上行超时（以毫秒为单位）| `10000` |
| `TRYIT_PROXY_RATE_LIMIT` |每个客户端 IP 每个速率窗口允许的请求数 | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` |速率限制滑动窗口（毫秒）| `60000` |

该代理还公开 `GET /healthz`，返回结构化 JSON 错误，并且
从日志输出中编辑不记名令牌。

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
配置了 Torii 原点。

## 连接门户小部件

当您构建或提供开发人员门户时，请设置小部件所使用的 URL
应该用于代理：

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

以下组件从 `docusaurus.config.js` 读取这些值：

- **Swagger UI** — 在 `/reference/torii-swagger` 处渲染；使用请求
  拦截器自动附加不记名令牌。
- **RapiDoc** — 在 `/reference/torii-rapidoc` 处呈现；镜像 token 字段
  并支持针对代理的尝试请求。
- **尝试控制台** — 嵌入 API 概述页面；让您发送自定义
  请求、查看标头并检查响应正文。

更改任何小部件中的令牌只会影响当前浏览器会话；的
代理永远不会保留或记录提供的令牌。

## 可观察性和操作

每个请求都会记录一次，其中包括方法、路径、来源、上游状态和
身份验证源（`override`、`default` 或 `client`）。代币从来都不是
存储 — 承载标头和 `X-TryIt-Auth` 值均在之前经过编辑
日志记录——这样你就可以将标准输出转发到中央收集器，而不必担心
秘密泄露。

### 健康探测和警报在部署期间或按计划运行捆绑探针：

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