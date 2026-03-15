---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/taikai-anchor-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 50261b1f3173cd3916b29c81e85cc92ed8c14c38a0e0296be38397fe9b5c0596
source_last_modified: "2025-12-29T18:16:35.204852+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Taikai Anchor 可观测性运行手册

此门户副本反映了规范运行手册
[`docs/source/taikai_anchor_monitoring.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/taikai_anchor_monitoring.md)。
在排练 SN13-C 路由清单 (TRM) 锚点时使用它，以便 SoraFS/SoraNet
操作员可以将线轴伪影、Prometheus 遥测和治理关联起来
无需离开门户预览版本即可获得证据。

## 范围和所有者

- **程序：** SN13-C — Taikai 清单和 SoraNS 锚。
- **所有者：** 媒体平台工作组、DA 计划、网络 TL、文档/开发版本。
- **目标：** 为 Sev1/Sev2 警报、遥测提供确定性剧本
  Taikai 路由清单前滚时的验证和证据捕获
  跨别名。

## 快速入门 (Sev1/Sev2)

1. **捕获线轴工件** — 复制最新的
   `taikai-anchor-request-*.json`、`taikai-trm-state-*.json` 和
   `taikai-lineage-*.json` 文件来自
   重新启动工作程序之前的 `config.da_ingest.manifest_store_dir/taikai/`。
2. **转储 `/status` 遥测** — 记录
   `telemetry.taikai_alias_rotations` 数组来证明哪个清单窗口
   活跃：
   ```bash
   curl -sSf "$TORII/status" | jq '.telemetry.taikai_alias_rotations'
   ```
3. **检查仪表板和警报** — 加载
   `dashboards/grafana/taikai_viewer.json`（集群+流过滤器）和注释
   是否有任何规则
   `dashboards/alerts/taikai_viewer_rules.yml` 已解雇（`TaikaiLiveEdgeDrift`，
   `TaikaiIngestFailure`、`TaikaiCekRotationLag`、SoraFS 健康证明事件）。
4. **检查 Prometheus** — 运行§“指标参考”中的查询以确认
   摄取延迟/漂移和别名旋转计数器的行为符合预期。升级
   如果 `taikai_trm_alias_rotations_total` 因多个窗口而停顿或者如果
   错误计数器增加。

## 指标参考

|公制|目的|
| ---| ---|
| `taikai_ingest_segment_latency_ms` |每个集群/流的 CMAF 摄取延迟直方图（目标：p95<750ms，p99<900ms）。 |
| `taikai_ingest_live_edge_drift_ms` |编码器和锚定工作人员之间的实时边缘漂移（p99>1.5 秒的页面持续 10 分钟）。 |
| `taikai_ingest_segment_errors_total{reason}` |按原因列出的错误计数器（`decode`、`manifest_mismatch`、`lineage_replay`，...）。任何增加都会触发 `TaikaiIngestFailure`。 |
| `taikai_trm_alias_rotations_total{alias_namespace,alias_name}` |每当 `/v2/da/ingest` 接受别名的新 TRM 时递增；使用 `rate()` 验证旋转节奏。 |
| `/status → telemetry.taikai_alias_rotations[]` |包含 `window_start_sequence`、`window_end_sequence`、`manifest_digest_hex`、`rotations_total` 和证据包时间戳的 JSON 快照。 |
| `taikai_viewer_*`（再缓冲、CEK 轮换年龄、PQ 运行状况、警报）|查看器端 KPI，确保 CEK 轮换 + PQ 电路在锚定期间保持健康。 |

### PromQL 片段

```promql
histogram_quantile(
  0.99,
  sum by (le) (
    rate(taikai_ingest_segment_latency_ms_bucket{cluster=~"$cluster",stream=~"$stream"}[5m])
  )
)
```

```promql
sum by (reason) (
  rate(taikai_ingest_segment_errors_total{cluster=~"$cluster",stream=~"$stream"}[5m])
)
```

```promql
rate(
  taikai_trm_alias_rotations_total{alias_namespace="sora",alias_name="docs"}[15m]
)
```

## 仪表板和警报

- **Grafana 查看器板：** `dashboards/grafana/taikai_viewer.json` — p95/p99
  延迟、实时边缘漂移、段错误、CEK 旋转年龄、查看器警报。
- **Grafana 缓存板：** `dashboards/grafana/taikai_cache.json` — 热/暖/冷
  别名窗口轮换时的提升和 QoS 拒绝。
- **Alertmanager 规则：** `dashboards/alerts/taikai_viewer_rules.yml` — 漂移
  寻呼、摄取失败警告、CEK 轮换延迟和 SoraFS 健康证明
  惩罚/冷却时间。确保每个生产集群都存在接收器。

## 证据包清单

- 线轴文物（`taikai-anchor-request-*`、`taikai-trm-state-*`、
  `taikai-lineage-*`）。
- 运行 `cargo xtask taikai-anchor-bundle --spool <manifest_dir>/taikai --copy-dir <bundle_dir> --signing-key <ed25519_hex>` 以发出待处理/已交付信封的签名 JSON 库存，并将请求/SSM/TRM/沿袭文件复制到钻取包中。默认假脱机路径是 `torii.toml` 中的 `storage/da_manifests/taikai`。
- `/status` 快照覆盖 `telemetry.taikai_alias_rotations`。
- Prometheus 通过事件窗口导出上述指标 (JSON/CSV)。
- Grafana 屏幕截图，过滤器可见。
- 引用相关规则的 Alertmanager ID 会触发。
- 链接至 `docs/examples/taikai_anchor_lineage_packet.md` 描述
  规范证据包。

## 仪表板镜像和练习节奏

满足SN13-C路线图要求意味着证明Taikai
查看器/缓存仪表板反映在门户**和锚点内部**
证据演习按照可预测的节奏进行。

1. **门户镜像。** 每当 `dashboards/grafana/taikai_viewer.json` 或
   `dashboards/grafana/taikai_cache.json` 更改，总结增量
   `sorafs/taikai-monitoring-dashboards`（此门户）并记下 JSON
   门户 PR 描述中的校验和。突出显示新面板/阈值，以便
   审阅者可以与托管的 Grafana 文件夹关联。
2. **每月演习。**
   - 在每月第一个星期二 15:00 UTC 进行演习，以便提供证据
     在 SN13 治理同步之前着陆。
   - 捕获线轴文物、`/status` 遥测和 Grafana 内部屏幕截图
     `artifacts/sorafs_taikai/drills/<YYYYMMDD>/`。
   - 记录执行情况
     `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`。
3. **审核并发布。** 在 48 小时内，通过
   DA计划+NetOps，在演练日志中记录后续项目，并链接
   治理存储桶从 `docs/source/sorafs/runbooks-index.md` 上传。

如果仪表板或钻头落后，SN13-C 无法退出🈺；保留这个
每当节奏或证据预期发生变化时，该部分都会更新。

## 有用的命令

```bash
# Snapshot alias rotation telemetry to an artefact directory
curl -sSf "$TORII/status" \
  | jq '{timestamp: now | todate, aliases: .telemetry.taikai_alias_rotations}' \
  > artifacts/taikai/status_snapshots/$(date -u +%Y%m%dT%H%M%SZ).json

# List spool entries for a specific alias/event
find "$MANIFEST_DIR/taikai" -maxdepth 1 -type f -name 'taikai-*.json' | sort

# Inspect TRM mismatch reasons from the spool log
jq '.error_context | select(.reason == "lineage_replay")' \
  "$MANIFEST_DIR/taikai/taikai-ssm-20260405T153000Z.norito"
```

每当 Taikai 时，请保持此门户副本与规范操作手册同步
锚定遥测、仪表板或治理证据需求发生变化。