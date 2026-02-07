---
id: nexus-transition-notes
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/transition-notes.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus transition notes
description: Mirror of `docs/source/nexus_transition_notes.md`, covering Phase B transition evidence, audit schedule, and mitigations.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus 过渡注释

本日志追踪了挥之不去的 **B 阶段 — Nexus 过渡基础** 工作
直到多通道启动清单完成。它补充了里程碑
`roadmap.md` 中的条目并将 B1–B4 引用的证据保留在一个地方
因此治理、SRE 和 SDK 领导可以共享相同的事实来源。

## 范围和节奏

- 涵盖路由跟踪审计和遥测护栏 (B1/B2)、
  治理批准的配置增量集（B3），以及多通道启动
  排练跟进（B4）。
- 替换以前住在这里的临时节奏音符；截至 2026 年
  Q1 审核详细报告位于
  `docs/source/nexus_routed_trace_audit_report_2026q1.md`，而此页面拥有
  运行时间表和缓解寄存器。
- 在每个路由跟踪窗口、治理投票或启动后更新表
  排练。每当文物移动时，都会镜像此页面内的新位置
  因此下游文档（状态、仪表板、SDK 门户）可以链接到稳定的
  锚。

## 证据快照（2026 年第一季度至第二季度）

|工作流程 |证据|所有者 |状态 |笔记|
|------------|----------|----------|--------|--------|
| **B1 — 路由跟踪审计** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`、`docs/examples/nexus_audit_outcomes/` | @telemetry-ops，@governance | ✅ 完成（2026 年第一季度）|记录了三个审核窗口； `TRACE-CONFIG-DELTA` 的 TLS 滞后在第二季度重新运行期间关闭。 |
| **B2 — 遥测修复和护栏** | `docs/source/nexus_telemetry_remediation_plan.md`、`docs/source/telemetry.md`、`dashboards/alerts/nexus_audit_rules.yml` | @sre-core，@telemetry-ops | ✅ 完整 |已发货警报包、diff bot 策略和 OTLP 批量大小调整（`nexus.scheduler.headroom` 日志 + Grafana 余量面板）；没有公开的豁免。 |
| **B3 — 配置增量批准** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`、`defaults/nexus/config.toml`、`defaults/nexus/genesis.json` | @release-eng、@governance | ✅ 完整 | GOV-2026-03-19 投票已捕获；签名的捆绑包提供下面提到的遥测包。 |
| **B4 — 多车道发射排练** | `docs/source/runbooks/nexus_multilane_rehearsal.md`、`docs/source/project_tracker/nexus_rehearsal_2026q1.md`、`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`、`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`、`artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core，@sre-core | ✅ 完成（2026 年第 2 季度）|第二季度金丝雀重新运行关闭了 TLS 延迟缓解措施；验证器清单 + `.sha256` 捕获槽范围 912–936、工作负载种子 `NEXUS-REH-2026Q2` 以及重新运行时记录的 TLS 配置文件哈希。 |

## 每季度路由跟踪审核计划

|跟踪 ID |窗口 (UTC) |结果|笔记|
|----------|--------------|---------|--------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | ✅ 通过 |队列准入 P95 远低于 ≤750 毫秒的目标。无需采取任何行动。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | ✅ 通过 | OTLP 重放哈希值附加到 `status.md`； SDK diff bot 奇偶校验确认零漂移。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | ✅ 已解决 | TLS 配置文件滞后在第二季度重新运行期间关闭； `NEXUS-REH-2026Q2` 的遥测包记录 TLS 配置文件哈希 `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`（请参阅 `artifacts/nexus/tls_profile_rollout_2026q2/`）和零落后者。 |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12–10:14 | ✅ 通过 |工作负载种子 `NEXUS-REH-2026Q2`；遥测包 + `artifacts/nexus/rehearsals/2026q1/` 下的清单/摘要（槽位范围 912–936），议程位于 `artifacts/nexus/rehearsals/2026q2/` 中。 |

未来几个季度应该添加新行并移动
当表增长超出当前范围时已完成附录条目
季度。从路由跟踪报告或治理会议记录中引用此部分
使用 `#quarterly-routed-trace-audit-schedule` 锚点。

## 缓解和积压项目

|项目 |描述 |业主|目标|状态/注释|
|------|-------------|--------|--------------------|----------------|
| `NEXUS-421` |完成 `TRACE-CONFIG-DELTA` 期间滞后的 TLS 配置文件的传播，捕获重新运行证据，并关闭缓解日志。 | @release-eng，@sre-core | 2026 年第 2 季度路由跟踪窗口 | ✅ 已关闭 — 在 `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` 中捕获的 TLS 配置文件哈希 `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`；重播确认没有掉队者。 |
| `TRACE-MULTILANE-CANARY` 准备 |安排第二季度排练，将固定装置连接到遥测包，并确保 SDK 线束重用经过验证的助手。 | @telemetry-ops，SDK 程序 |规划电话 2026-04-30 | ✅ 已完成 — 议程存储在 `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` 中，并包含插槽/工作负载元数据；跟踪器中记录了安全带的重复使用情况。 |
|遥测包摘要轮换 |在每次排练/发布之前运行 `scripts/telemetry/validate_nexus_telemetry_pack.py`，并在配置增量跟踪器旁边记录摘要。 | @遥测操作 |每个候选版本 | ✅ 已完成 — `telemetry_manifest.json` + `.sha256` 在 `artifacts/nexus/rehearsals/2026q1/` 中发出（时隙范围 `912-936`，种子 `NEXUS-REH-2026Q2`）；复制到跟踪器和证据索引中的摘要。 |

## 配置 Delta Bundle 集成

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` 仍然是
  规范差异摘要。当新的 `defaults/nexus/*.toml` 或创世发生变化时
  着陆后，首先更新该跟踪器，然后在此处镜像高光。
- 签名的配置包提供排练遥测包。包装，已验证
  作者：`scripts/telemetry/validate_nexus_telemetry_pack.py`，必须发布
  与配置增量证据一起，以便操作员可以重放确切的
  B4期间使用的文物。
- Iroha 2 个捆绑包保持无通道：现在使用 `nexus.enabled = false` 进行配置
  拒绝通道/数据空间/路由覆盖，除非启用 Nexus 配置文件
  (`--sora`)，因此从单通道模板中剥离 `nexus.*` 部分。
- 保持治理投票日志 (GOV-2026-03-19) 与跟踪器和
  此注释以便将来的投票可以复制格式而无需重新发现
  批准仪式。

## 启动排练后续行动

- `docs/source/runbooks/nexus_multilane_rehearsal.md` 捕获金丝雀计划，
  参与者名册和回滚步骤；每当车道刷新运行手册
  拓扑或遥测导出器发生变化。
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` 列出了所有文物
  在 4 月 9 日排练期间进行了检查，现在带有第二季度的准备笔记/议程。
  将未来的排练附加到同一个跟踪器，而不是一次性开放
  跟踪器保持证据的单调性。
- 发布 OTLP 收集器片段和 Grafana 导出（请参阅 `docs/source/telemetry.md`）
  每当出口商批次指导发生变化时；第一季度更新提高了
  批量大小为 256 个样本，以防止出现余量警报。
- 多通道 CI/测试证据现在存在于
  `integration_tests/tests/nexus/multilane_pipeline.rs` 并运行在
  `Nexus Multilane Pipeline` 工作流程
  (`.github/workflows/integration_tests_multilane.yml`)，取代已退役的
  `pytests/nexus/test_multilane_pipeline.py` 参考；保留哈希值
  `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b
  `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) 同步
  刷新排练包时使用跟踪器。

## 运行时通道生命周期

- 运行时通道生命周期计划现在验证数据空间绑定并在以下情况下中止
  Kura/分层存储协调失败，目录保持不变。的
  帮助者修剪已退役通道的缓存通道中继，以便合并账本合成
  不重复使用过时的证明。
- 通过 Nexus 配置/生命周期帮助程序应用计划（`State::apply_lane_lifecycle`、
  `Queue::apply_lane_lifecycle`) 无需重新启动即可添加/删除通道；路由，
  TEU 快照和舱单注册表会在计划成功后自动重新加载。
- 操作员指南：当计划失败时，检查是否丢失数据空间或存储
  无法创建的根（分层冷根/Kura Lane 目录）。修复
  返回路径并重试；成功的计划重新发射车道/数据空间遥测数据
  diff，以便仪表板反映新的拓扑。

## NPoS 遥测和背压证据

PhaseB 的启动排练回顾要求确定性遥测捕获
证明 NPoS 起搏器和八卦层保持在其背压范围内
限制。集成线束位于
`integration_tests/tests/sumeragi_npos_performance.rs` 练习那些
场景并发出 JSON 摘要 (`sumeragi_baseline_summary::<scenario>::…`)
每当新指标落地时。使用以下命令在本地运行它：

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

设置 `SUMERAGI_NPOS_STRESS_PEERS`、`SUMERAGI_NPOS_STRESS_COLLECTORS_K`，或
`SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` 探索更高应力的拓扑；的
默认镜像 B4 中使用的 1s/`k=3` 收集器配置文件。

|场景/测试|覆盖范围|关键遥测 |
| ---| ---| ---|
| `npos_baseline_1s_k3_captures_metrics` |在序列化证据包之前，使用排练块时间来阻止 12 轮，以记录 EMA 延迟包络、队列深度和冗余发送量规。 | `sumeragi_phase_latency_ema_ms`、`sumeragi_collectors_k`、`sumeragi_redundant_send_r`、`sumeragi_bg_post_queue_depth*`。 |
| `npos_queue_backpressure_triggers_metrics` |淹没事务队列以确保准入延迟确定性地启动，并且队列导出容量/饱和计数器。 | `sumeragi_tx_queue_depth`、`sumeragi_tx_queue_capacity`、`sumeragi_tx_queue_saturated`、`sumeragi_pacemaker_backpressure_deferrals_total`、`sumeragi_rbc_backpressure_deferrals_total`。 |
| `npos_pacemaker_jitter_within_band` |对起搏器抖动进行采样并查看超时，直到证明已强制执行配置的 ±125‰ 频带。 | `sumeragi_pacemaker_jitter_ms`、`sumeragi_pacemaker_view_timeout_target_ms`、`sumeragi_pacemaker_jitter_frac_permille`。 |
| `npos_rbc_store_backpressure_records_metrics` |将大型 RBC 有效负载推至软/硬存储限制，以显示会话和字节计数器在不超出存储的情况下爬升、后退和稳定。 | `sumeragi_rbc_store_pressure`、`sumeragi_rbc_store_sessions`、`sumeragi_rbc_store_bytes`、`sumeragi_rbc_backpressure_deferrals_total`。 |
| `npos_redundant_send_retries_update_metrics` |强制重新传输，以便冗余发送比率计量器和目标收集器计数器前进，证明复古请求的遥测是端到端有线的。 | `sumeragi_collectors_targeted_current`、`sumeragi_redundant_sends_total`。 |
| `npos_rbc_chunk_loss_fault_reports_backlog` |删除确定性间隔的块以验证积压监视器会引发故障，而不是默默地耗尽有效负载。 | `sumeragi_rbc_backlog_sessions_pending`、`sumeragi_rbc_backlog_chunks_total`、`sumeragi_rbc_backlog_chunks_max`。 |将线束打印的 JSON 行与 Prometheus 刮擦一起附加
每当治理要求提供反压证据时，就会在运行期间捕获
警报与排练拓扑相匹配。

## 更新清单

1. 当季度滚动时，附加新的路由跟踪窗口并淘汰旧的窗口。
2. 在每次 Alertmanager 跟进后更新缓解表，即使
   操作是关闭票证。
3. 当配置增量发生变化时，更新跟踪器、此注释和遥测数据
   将摘要列表打包在同一拉取请求中。
4. 在此处链接任何新的排练/遥测工件，以便了解未来的路线图状态
   更新可以引用单个文档，而不是分散的临时注释。

## 证据索引

|资产|地点 |笔记|
|--------|----------|--------|
|路由跟踪审计报告（2026 年第一季度）| `docs/source/nexus_routed_trace_audit_report_2026q1.md` | PhaseB1 证据的规范来源；镜像为 `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md` 下的门户。 |
|配置增量跟踪器 | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |包含 TRACE-CONFIG-DELTA 差异摘要、审阅者姓名缩写和 GOV-2026-03-19 投票日志。 |
|遥测修复计划| `docs/source/nexus_telemetry_remediation_plan.md` |记录与 B2 相关的警报包、OTLP 批量大小和出口预算护栏。 |
|多车道排练跟踪器 | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` |列出 4 月 9 日的排练工件、验证器清单/摘要、第二季度准备笔记/议程和回滚证据。 |
|遥测包清单/摘要（最新）| `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) |记录插槽范围 912–936、种子 `NEXUS-REH-2026Q2` 以及治理包的工件哈希值。 |
| TLS 配置文件清单 | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) |在第二季度重新运行期间捕获的已批准 TLS 配置文件的哈希值；在路由跟踪附录中引用。 |
|跟踪多车道金丝雀议程 | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` |第二季度排练的规划说明（窗口、时段范围、工作负载种子、操作所有者）。 |
|启动排练手册| `docs/source/runbooks/nexus_multilane_rehearsal.md` |暂存→执行→回滚的操作清单；当车道拓扑或出口商指导发生变化时更新。 |
|遥测包验证器 | `scripts/telemetry/validate_nexus_telemetry_pack.py` | B4 Retro 引用的 CLI；每当包发生变化时，存档摘要都会与跟踪器一起进行。 |
|多车道回归 | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` |证明 `nexus.enabled = true` 的多通道配置，保留 Sora 目录哈希，并在发布工件摘要之前通过 `ConfigLaneRouter` 提供通道本地 Kura/合并日志路径 (`blocks/lane_{id:03}_{slug}`)。 |