---
id: observability
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/observability.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Portal Observability & Analytics
sidebar_label: Observability
description: Telemetry, release tagging, and verification automation for the developer portal.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

DOCS-SORA 路线图需要分析、综合探针和断开的链接
每个预览版本的自动化。本说明记录了现在的管道
随门户一起提供，以便操作员可以进行监控而不会泄漏访客
数据。

## 发布标签

- 设置 `DOCS_RELEASE_TAG=<identifier>`（回退到 `GIT_COMMIT` 或 `dev`）
  建设门户。该值被注入到 `<meta name="sora-release">` 中
  因此探测器和仪表板可以区分部署。
- `npm run build` 发出 `build/release.json` （由
  `scripts/write-checksums.mjs`) 描述标签、时间戳和可选
  `DOCS_RELEASE_SOURCE`。相同的文件被捆绑到预览工件中，并且
  由链接检查器报告引用。

## 隐私保护分析

- 将 `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` 配置为
  启用轻量级跟踪器。有效负载包含`{事件、路径、区域设置、
  发布，ts}` with no referrer or IP metadata, and `navigator.sendBeacon`
  尽可能使用以避免阻塞导航。
- 使用 `DOCS_ANALYTICS_SAMPLE_RATE` (0–1) 控制采样。追踪器商店
  最后发送的路径，并且永远不会为同一导航发出重复事件。
- 该实现位于 `src/components/AnalyticsTracker.jsx` 中，并且是
  通过 `src/theme/Root.js` 全局安装。

## 合成探针

- `npm run probe:portal` 针对常见路由发出 GET 请求
  （`/`、`/norito/overview`、`/reference/torii-swagger` 等）并验证
  `sora-release` 元标记匹配 `--expect-release`（或
  `DOCS_RELEASE_TAG`）。示例：

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

每个路径都会报告失败，因此可以轻松地在探测成功时控制 CD。

## 断链自动化

- `npm run check:links` 扫描 `build/sitemap.xml`，确保每个条目映射到
  本地文件（检查 `index.html` 回退），并写入
  `build/link-report.json` 包含发布元数据、总数、失败、
  以及 `checksums.sha256` 的 SHA-256 指纹（公开为 `manifest.id`）
  因此每份报告都可以与工件清单联系起来。
- 当页面丢失时，脚本以非零值退出，因此 CI 可以阻止发布
  陈旧或损坏的路线。报告引用了尝试过的候选路径，
  这有助于将路由回归跟踪回文档树。

## Grafana 仪表板和警报

- `dashboards/grafana/docs_portal.json` 发布 **文档门户发布**
  Grafana 板。它运送以下面板：
  - *网关拒绝 (5m)* 使用 `torii_sorafs_gateway_refusals_total` 范围
    `profile`/`reason`，以便 SRE 可以检测不良策略推送或令牌故障。
  - *Alias 缓存刷新结果* 和 *Alias Proof Age p90* 跟踪
    `torii_sorafs_alias_cache_*` 证明在 DNS 削减之前存在新的证据
    结束了。
  - *Pin 注册表清单计数* 加上 *活动别名计数* 统计镜像
    PIN 注册积压和总别名，以便治理可以审核每个版本。
  - *网关 TLS 到期（小时）* 突出显示发布网关的 TLS 的时间
    证书即将到期（警报阈值为 72 小时）。
  - *复制 SLA 结果*和*复制待办事项*密切关注
    `torii_sorafs_replication_*` 遥测，确保所有副本符合 GA
    发布后栏。
- 使用内置模板变量（`profile`、`reason`）重点关注
  `docs.sora` 发布配置文件或调查所有网关的峰值。
- PagerDuty 路由使用仪表板面板作为证据：名为的警报
  `DocsPortal/GatewayRefusals`、`DocsPortal/AliasCache` 和
  `DocsPortal/TLSExpiry` 当相应系列违反其规定时起火
  阈值。将警报的运行手册链接到此页面，以便值班工程师可以
  重放确切的 Prometheus 查询。

## 把它放在一起

1. 在 `npm run build` 期间，设置发布/分析环境变量并
   让构建后步骤发出 `checksums.sha256`、`release.json` 和
   `link-report.json`。
2. 针对预览主机名运行 `npm run probe:portal`
   `--expect-release` 连接到同一标签。保存标准输出以供发布
   清单。
3. 运行 `npm run check:links` 以快速修复损坏的站点地图条目和存档
   生成的 JSON 报告以及预览工件。 CI 放弃了
   最新报告 `artifacts/docs_portal/link-report.json`，以便治理可以
   直接从构建日志下载证据包。
4. 将分析端点转发到您的隐私保护收集器（貌似合理，
   自托管 OTEL 采集等）并确保采样率记录在案
   发布以便仪表板正确解释计数。
5. CI 已通过预览/部署工作流程连接这些步骤
   （`.github/workflows/docs-portal-preview.yml`，
   `.github/workflows/docs-portal-deploy.yml`），因此本地演练只需要
   涵盖秘密特定行为。