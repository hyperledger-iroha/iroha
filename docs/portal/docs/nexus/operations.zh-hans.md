---
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dea5098afdf15e92c78aba363b38f3ec8ce2018672a3d34bb1b505e2ee2f5869
source_last_modified: "2025-12-29T18:16:35.144858+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-operations
title: Nexus operations runbook
description: Field-ready summary of the Nexus operator workflow, mirroring `docs/source/nexus_operations.md`.
translator: machine-google-reviewed
---

使用此页面作为快速参考同级页面
`docs/source/nexus_operations.md`。它提炼出操作清单、更改
Nexus 操作员必须执行的管理挂钩和遥测覆盖要求
跟随。

## 生命周期清单

|舞台|行动|证据|
|--------|--------|----------|
|飞行前 |验证发布哈希/签名，确认 `profile = "iroha3"`，并准备配置模板。 | `scripts/select_release_profile.py` 输出、校验和日志、签名清单包。 |
|目录对齐 |根据理事会发布的清单更新 `[nexus]` 目录、路由策略和 DA 阈值，然后捕获 `--trace-config`。 | `irohad --sora --config … --trace-config` 输出与登机单一起存储。 |
|烟雾和切换 |运行 `irohad --sora --config … --trace-config`，执行 CLI Smoke (`FindNetworkStatus`)，验证遥测导出并请求准入。 |冒烟测试日志 + Alertmanager 确认。 |
|稳态|监控仪表板/警报，根据治理节奏轮换密钥，并在清单发生变化时同步配置/运行手册。 |季度审核记录、仪表板屏幕截图、轮换票 ID。 |

详细的入职培训（密钥更换、路由模板、发布配置文件步骤）
保留在 `docs/source/sora_nexus_operator_onboarding.md` 中。

## 变更管理

1. **发布更新** – 跟踪 `status.md`/`roadmap.md` 中的公告；附加
   每个版本 PR 的入门清单。
2. **通道清单更改** – 验证来自空间目录的签名包并
   将它们存档在 `docs/source/project_tracker/nexus_config_deltas/` 下。
3. **配置增量** – 每个 `config/config.toml` 更改都需要票证
   引用车道/数据空间。存储有效配置的编辑副本
   每当节点加入或升级时。
4. **回滚演习** – 每季度排练停止/恢复/烟雾程序；日志
   `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md` 下的结果。
5. **合规性批准** – 私有/CBDC通道必须确保合规性签字
   在修改 DA 策略或遥测编辑旋钮之前（请参阅
   `docs/source/cbdc_lane_playbook.md`）。

## 遥测和 SLO

- 仪表板：`dashboards/grafana/nexus_lanes.json`、`nexus_settlement.json`，以及
  SDK 特定视图（例如，`android_operator_console.json`）。
- 警报：`dashboards/alerts/nexus_audit_rules.yml` 和 Torii/Norito 传输
  规则（`dashboards/alerts/torii_norito_rpc_rules.yml`）。
- 值得关注的指标：
  - `nexus_lane_height{lane_id}` – 三个插槽的进度为零时发出警报。
  - `nexus_da_backlog_chunks{lane_id}` – 高于车道特定阈值的警报
    （默认 64 个公共/8 个私有）。
  - `nexus_settlement_latency_seconds{lane_id}` – 当 P99 超过 900ms 时发出警报
    （公共）或 1200 毫秒（私人）。
  - `torii_request_failures_total{scheme="norito_rpc"}` – 如果出现 5 分钟错误则发出警报
    比例>2%。
  - `telemetry_redaction_override_total` – 立即 Sev2；确保覆盖
    有合规票。
- 运行遥测修复清单
  [Nexus 遥测修复计划](./nexus-telemetry-remediation) 至少
  每季度一次，并将填写好的表格附加到运营审查记录中。

## 事件矩阵

|严重性 |定义 |回应 |
|----------|------------|----------|
|严重程度1 |数据空间隔离违规、结算暂停>15分钟或治理投票腐败。 | Nexus 页 主要 + 发布工程 + 合规性、冻结准入、收集文物、发布通信 ≤60 分钟，RCA ≤5 个工作日。 |
|严重程度2 |车道积压 SLA 违规、遥测盲点 >30 分钟、清单推出失败。 |页 Nexus 主要 + SRE，缓解 ≤4 小时，2 个工作日内归档跟进。 |
|严重程度3 |非阻塞漂移（文档、警报）。 |登录跟踪器，安排冲刺内的修复。 |

事件单必须记录受影响的车道/数据空间 ID、清单哈希值、
时间表、支持指标/日志以及后续任务/所有者。

## 证据存档

- 将捆绑包/清单/遥测导出存储在 `artifacts/nexus/<lane>/<date>/` 下。
- 保留每个版本的编辑配置+ `--trace-config` 输出。
- 当配置或清单更改土地时，附上理事会会议记录+签署的决定。
- 将与 Nexus 指标相关的每周 Prometheus 快照保留 12 个月。
- 在 `docs/source/project_tracker/nexus_config_deltas/README.md` 中记录操作手册编辑
  这样审计人员就知道职责何时发生变化。

## 相关材料

- 概述：[Nexus 概述](./nexus-overview)
- 规格：[Nexus 规格](./nexus-spec)
- 车道几何形状：[Nexus 车道模型](./nexus-lane-model)
- 转换和布线垫片：[Nexus 转换说明](./nexus-transition-notes)
- 操作员入门：[Sora Nexus 操作员入门](./nexus-operator-onboarding)
- 遥测修复：[Nexus 遥测修复计划](./nexus-telemetry-remediation)