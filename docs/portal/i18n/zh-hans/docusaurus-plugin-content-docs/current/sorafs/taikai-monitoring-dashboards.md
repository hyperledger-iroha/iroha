---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/taikai-monitoring-dashboards.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Taikai Monitoring Dashboards
description: Portal summary of the viewer/cache Grafana boards that back SN13-C evidence
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Taikai 路由清单 (TRM) 准备情况取决于两个 Grafana 板及其
同伴警报。此页面反映了以下内容的亮点
`dashboards/grafana/taikai_viewer.json`，`dashboards/grafana/taikai_cache.json`，
和 `dashboards/alerts/taikai_viewer_rules.yml`，以便审阅者可以跟进
无需克隆存储库。

## 查看器仪表板 (`taikai_viewer.json`)

- **实时边缘和延迟：** 面板可视化 p95/p99 延迟直方图
  （`taikai_ingest_segment_latency_ms`、`taikai_ingest_live_edge_drift_ms`）每
  集群/流。观察 p99 >900ms 或漂移 >1.5s（触发
  `TaikaiLiveEdgeDrift` 警报）。
- **段错误：** 将 `taikai_ingest_segment_errors_total{reason}` 分解为
  暴露解码失败、沿袭重放尝试或明显不匹配。
  每当此面板上升到高于 SN13-C 事件时，请附上屏幕截图
  “警告”带。
- **查看者和 CEK 健康状况：** 面板源自 `taikai_viewer_*` 指标跟踪
  CEK 轮换年龄、PQ 保护组合、重新缓冲计数和警报汇总。 CEK
  小组强制执行轮换 SLA，治理在批准新协议之前会对其进行审查
  别名。
- **别名遥测快照：** `/status → telemetry.taikai_alias_rotations`
  桌子直接位于板上，以便操作员可以确认清单摘要
  在附上治理证据之前。

## 缓存仪表板 (`taikai_cache.json`)

- **层压力：** 面板图表 `sorafs_taikai_cache_{hot,warm,cold}_occupancy`
  和 `sorafs_taikai_cache_promotions_total`。使用这些来查看是否有 TRM
  轮换使特定层超载。
- **QoS 拒绝：** `sorafs_taikai_qos_denied_total` 在缓存压力时出现
  强制节流；每当速率偏离零时，就对钻探日志进行注释。
- **出口利用率：** 帮助确认 SoraFS 出口与 Taikai 保持同步
  CMAF 窗口旋转时的查看器。

## 警报和证据捕获

- 分页规则位于 `dashboards/alerts/taikai_viewer_rules.yml` 和映射一中
  与上述面板之一（`TaikaiLiveEdgeDrift`、`TaikaiIngestFailure`、
  `TaikaiCekRotationLag`，证明健康警告）。确保每一次生产
  集群将这些连接到 Alertmanager。
- 演习期间捕获的快照/屏幕截图必须存储在
  `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` 以及假脱机文件和
  `/status` JSON。使用 `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`
  将执行附加到共享钻取日志中。
- 当仪表板更改时，将 JSON 文件的 SHA-256 摘要包含在
  门户 PR 描述，以便审核员可以将托管 Grafana 文件夹与
  回购版本。

## 证据包清单

SN13-C 审查期望每次演习或事件都运送所列的相同物品
在 Taikai 主播操作手册中。按以下顺序捕获它们，以便捆绑包
准备好进行治理审查：

1.复制最新的`taikai-anchor-request-*.json`，
   `taikai-trm-state-*.json` 和 `taikai-lineage-*.json` 文件来自
   `config.da_ingest.manifest_store_dir/taikai/`。这些线轴文物证明
   哪个路由清单 (TRM) 和沿袭窗口处于活动状态。帮手
   `cargo xtask taikai-anchor-bundle --spool <dir> --copy-dir <out> --out <out>/anchor_bundle.json [--signing-key <ed25519>]`
   将复制假脱机文件，发出哈希值，并可选择签署摘要。
2.记录`/v2/status`输出过滤到
   `.telemetry.taikai_alias_rotations[]` 并将其存储在假脱机文件旁边。
   审阅者将报告的 `manifest_digest_hex` 和窗口边界与
   复制的假脱机状态。
3.导出上面列出的指标的Prometheus快照并截图
   具有相关集群/流过滤器的查看器/缓存仪表板
   视图。将原始 JSON/CSV 和屏幕截图放入 artefact 文件夹中。
4. 包含引用规则的 Alertmanager 事件 ID（如果有）
   `dashboards/alerts/taikai_viewer_rules.yml` 并注意它们是否自动关闭
   一旦病情清除。

将所有内容存储在 `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` 下，以便钻取
审计和 SN13-C 治理审查可以获取单个档案。

## 练习节奏和记录

- 每个月第一个星期二 15:00 UTC 进行 Taikai 锚定演习。
  该时间表在 SN13 治理同步之前保持证据新鲜。
- 捕获上述工件后，将执行附加到共享账本
  与 `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`。的
  帮助程序发出 `docs/source/sorafs/runbooks-index.md` 所需的 JSON 条目。
- 链接 Runbook 索引条目中的存档工件并升级任何失败的项目
  通过媒体平台 WG/SRE 在 48 小时内发出警报或仪表板回归
  频道。
- 保留练习摘要屏幕截图集（延迟、漂移、错误、CEK 旋转、
  缓存压力）与线轴捆绑在一起，以便操作员可以准确地显示如何
  排练期间仪表板表现良好。

请参阅 [Taikai Anchor Runbook](./taikai-anchor-runbook.md) 了解
完整的 Sev1 程序和证据清单。此页面仅捕获
SN13-C 在离开之前需要的仪表板特定指导 🈺。