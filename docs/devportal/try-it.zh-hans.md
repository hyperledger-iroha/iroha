---
lang: zh-hans
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

开发人员门户为 Torii REST API 提供了一个“尝试”控制台。本指南
解释如何启动支持代理并将控制台连接到登台
网关而不暴露凭据。

## 先决条件

- Iroha 存储库签出（工作空间根目录）。
- Node.js 18.18+（与门户基线匹配）。
- Torii 端点可从您的工作站（临时或本地）访问。

## 1. 生成 OpenAPI 快照（可选）

控制台重复使用与门户参考页面相同的 OpenAPI 负载。如果
您已更改 Torii 路由，请重新生成快照：

```bash
cargo xtask openapi
```

该任务写入 `docs/portal/static/openapi/torii.json`。

## 2. 启动 Try It 代理

从存储库根目录：

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional defaults
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### 环境变量

|变量|描述 |
|----------|-------------|
| `TRYIT_PROXY_TARGET` | Torii 基本 URL（必需）。 |
| `TRYIT_PROXY_ALLOWED_ORIGINS` |允许使用代理的以逗号分隔的来源列表（默认为 `http://localhost:3000`）。 |
| `TRYIT_PROXY_BEARER` |可选的默认承载令牌应用于所有代理请求。 |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` |设置为 `1` 以逐字转发调用者的 `Authorization` 标头。 |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` |内存中速率限制器设置（默认值：每 60 秒 60 个请求）。 |
| `TRYIT_PROXY_MAX_BODY` |接受的最大请求负载（字节，默认 1MiB）。 |
| `TRYIT_PROXY_TIMEOUT_MS` | Torii 请求的上行超时（默认 10000 毫秒）。 |

代理暴露：

- `GET /healthz` — 准备情况检查。
- `/proxy/*` — 代理请求，保留路径和查询字符串。

## 3.启动门户

在单独的终端中：

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

访问 `http://localhost:3000/api/overview` 并使用 Try It 控制台。一样的
环境变量配置 Swagger UI 和 RapiDoc 嵌入。

## 4. 运行单元测试

该代理公开了一个快速的基于节点的测试套件：

```bash
npm run test:tryit-proxy
```

测试涵盖地址解析、来源处理、速率限制和承载
注射。

## 5. 探测自动化和指标

使用捆绑探针验证 `/healthz` 和示例端点：

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

环境旋钮：

- `TRYIT_PROXY_SAMPLE_PATH` — 可选的 Torii 路线（无 `/proxy`）进行锻炼。
- `TRYIT_PROXY_SAMPLE_METHOD` — 默认为 `GET`；设置为 `POST` 用于写入路由。
- `TRYIT_PROXY_PROBE_TOKEN` — 为示例调用注入临时承载令牌。
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — 覆盖默认的 5 秒超时。
- `TRYIT_PROXY_PROBE_METRICS_FILE` — Prometheus `probe_success`/`probe_duration_seconds` 的文本文件目标。
- `TRYIT_PROXY_PROBE_LABELS` — 附加到度量的以逗号分隔的 `key=value` 对（默认为 `job=tryit-proxy` 和 `instance=<proxy URL>`）。

当设置 `TRYIT_PROXY_PROBE_METRICS_FILE` 时，脚本重写文件
原子地，所以你的node_exporter/textfile收集器总是看到一个完整的
有效负载。示例：

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

将生成的指标转发到 Prometheus 并在
当 `probe_success` 下降到 `0` 时，开发人员门户文档页面。

## 6. 生产强化检查表

在本地开发之外发布代理之前：

- 在代理（反向代理或托管网关）之前终止 TLS。
- 配置结构化日志记录并转发到可观察性管道。
- 轮换不记名令牌并将其存储在您的秘密管理器中。
- 监控代理的 `/healthz` 端点和聚合延迟指标。
- 将速率限制与您的 Torii 暂存配额保持一致；调整`Retry-After`
  向客户端传达限制的行为。