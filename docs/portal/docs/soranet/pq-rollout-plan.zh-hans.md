---
lang: zh-hans
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9774dca76eff9ff13fcad9bf1fa7f084b95a987c392727cf0e6a74a4844e2b8e
source_last_modified: "2026-01-22T14:45:01.375247+00:00"
translation_last_reviewed: 2026-02-07
id: pq-rollout-plan
title: SNNet-16G Post-Quantum Rollout Playbook
sidebar_label: PQ Rollout Plan
description: Operational guide for promoting the SoraNet hybrid X25519+ML-KEM handshake from canary to default across relays, clients, and SDKs.
translator: machine-google-reviewed
---

:::注意规范来源
:::

SNNet-16G 完成了 SoraNet 传输的后量子部署。 `rollout_phase` 旋钮使操作员能够协调从现有 A 阶段防护要求到 B 阶段多数覆盖率和 C 阶段严格 PQ 姿势的确定性升级，而无需为每个表面编辑原始 JSON/TOML。

本剧本涵盖：

- 阶段定义和新的配置旋钮（`sorafs.gateway.rollout_phase`、`sorafs.rollout_phase`）连接到代码库（`crates/iroha_config/src/parameters/actual.rs:2230`、`crates/iroha/src/config/user.rs:251`）中。
- SDK 和 CLI 标志映射，以便每个客户端都可以跟踪部署。
- 中继/客户端金丝雀调度期望以及控制促销的治理仪表板 (`dashboards/grafana/soranet_pq_ratchet.json`)。
- 回滚钩子和对防火练习操作手册的引用（[PQ 棘轮操作手册](./pq-ratchet-runbook.md)）。

## 相位图

| `rollout_phase` |有效匿名阶段|默认效果 |典型用法|
|-----------------|----------------------------------------|----------------|------------------------|
| `canary` | `anon-guard-pq`（A 阶段）|当舰队热身时，每个回路至少需要一名 PQ 警卫。 |基线和早期金丝雀周。 |
| `ramp` | `anon-majority-pq`（B 阶段）|偏向 PQ 继电器选择，覆盖范围 >= 三分之二；经典中继仍然是后备方案。 |逐个区域的中继金丝雀； SDK 预览切换。 |
| `default` | `anon-strict-pq`（C 阶段）|实施仅限 PQ 的电路并加强降级警报。 |一旦遥测和治理签字完成，最终晋升。 |

如果表面还设置了显式 `anonymity_policy`，它将覆盖该组件的相位。现在省略显式阶段遵循 `rollout_phase` 值，因此操作员可以在每个环境中翻转阶段一次并让客户端继承它。

## 配置参考

### 协调器 (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Orchestrator 加载程序在运行时解析回退阶段 (`crates/sorafs_orchestrator/src/lib.rs:2229`)，并通过 `sorafs_orchestrator_policy_events_total` 和 `sorafs_orchestrator_pq_ratio_*` 来显示它。有关可立即应用的代码片段，请参阅 `docs/examples/sorafs_rollout_stage_b.toml` 和 `docs/examples/sorafs_rollout_stage_c.toml`。

### Rust 客户端 / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` 现在记录解析的阶段 (`crates/iroha/src/client.rs:2315`)，因此帮助程序命令（例如 `iroha_cli app sorafs fetch`）可以报告当前阶段以及默认的匿名策略。

## 自动化

两个 `cargo xtask` 帮助程序自动生成计划和捕获工件。

1. **生成区域时间表**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   持续时间接受 `s`、`m`、`h` 或 `d` 后缀。该命令发出 `artifacts/soranet_pq_rollout_plan.json` 和 Markdown 摘要 (`artifacts/soranet_pq_rollout_plan.md`)，可以随更改请求一起提供。

2. **捕获带有签名的钻孔制品**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   该命令将提供的文件复制到 `artifacts/soranet_pq_rollout/<timestamp>_<label>/`，计算每个工件的 BLAKE3 摘要，并在有效负载上写入包含元数据和 Ed25519 签名的 `rollout_capture.json`。使用签署消防演习记录的同一私钥，以便治理可以快速验证捕获。

## SDK 和 CLI 标志矩阵

|表面|金丝雀（A 阶段）|坡道（B 阶段）|默认（C 阶段）|
|--------------------|--------------------------------|----------------|--------------------|
| `sorafs_cli` 获取 | `--anonymity-policy stage-a` 还是靠相| `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Orchestrator 配置 JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust 客户端配置 (`iroha.toml`) | `rollout_phase = "canary"`（默认）| `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` 签名命令 | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`，可选 `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`，可选 `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`，可选 `.ANON_STRICT_PQ` |
| JavaScript 协调器助手 | `rolloutPhase: "canary"` 或 `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
|斯威夫特 `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

所有 SDK 切换都映射到编排器 (`crates/sorafs_orchestrator/src/lib.rs:365`) 使用的同一阶段解析器，因此混合语言部署与配置的阶段保持同步。

## 金丝雀调度清单

1. **飞行前（T 减 2 周）**

- 确认 A 阶段过去两周的限电率 <1%，并且每个区域的 PQ 覆盖率 >=70% (`sorafs_orchestrator_pq_candidate_ratio`)。
   - 安排批准金丝雀窗口的治理审查时段。
   - 在暂存中更新 `sorafs.gateway.rollout_phase = "ramp"`（编辑编排器 JSON 并重新部署）并试运行升级管道。

2. **接力金丝雀（T日）**

   - 通过在协调器和参与的中继清单上设置 `rollout_phase = "ramp"`，一次升级一个区域。
   - 在 PQ Ratchet 仪表板（现在具有推出面板）中监视“每个结果的策略事件”和“掉电率”，以获得两倍的保护缓存 TTL。
   - 在运行之前和之后剪切 `sorafs_cli guard-directory fetch` 快照以用于审核存储。

3. **客户端/SDK 金丝雀（T 加 1 周）**

   - 在客户端配置中翻转 `rollout_phase = "ramp"` 或为指定的 SDK 群组传递 `stage-b` 覆盖。
   - 捕获遥测差异（`sorafs_orchestrator_policy_events_total` 由 `client_id` 和 `region` 分组）并将其附加到推出事件日志中。

4. **默认促销（T+3周）**

   - 治理结束后，将协调器和客户端配置切换到 `rollout_phase = "default"`，并将签名的准备清单轮换到发布工件中。

## 治理和证据清单

|相变|促销门|证据包|仪表板和警报 |
|--------------|----------------|-----------------|---------------------|
|金丝雀 → 坡道 *（B 阶段预览）* |过去 14 天内 A 阶段的管制率 <1%，每个促销区域 `sorafs_orchestrator_pq_candidate_ratio` ≥ 0.7，Argon2 票证验证 p95 < 50 毫秒，并且预订了促销的治理时段。 | `cargo xtask soranet-rollout-plan` JSON/Markdown 对、配对的 `sorafs_cli guard-directory fetch` 快照（之前/之后）、签名的 `cargo xtask soranet-rollout-capture --label canary` 捆绑包以及引用 [PQ 棘轮运行手册](./pq-ratchet-runbook.md) 的金丝雀分钟。 | `dashboards/grafana/soranet_pq_ratchet.json`（策略事件 + 掉电率）、`dashboards/grafana/soranet_privacy_metrics.json`（SN16 降级比率）、`docs/source/soranet/snnet16_telemetry_plan.md` 中的遥测参考。 |
|斜坡 → 默认 *（C 阶段实施）* | 30 天的 SN16 遥测老化测试满足要求，`sn16_handshake_downgrade_total` 在基线上持平，`sorafs_orchestrator_brownouts_total` 在客户端金丝雀期间为零，并且记录了代理切换排练。 | `sorafs_cli proxy set-mode --mode gateway|direct` 转录本、`promtool test rules dashboards/alerts/soranet_handshake_rules.yml` 输出、`sorafs_cli guard-directory verify` 日志和签名的 `cargo xtask soranet-rollout-capture --label default` 捆绑包。 |相同的 PQ Ratchet 板加上 `docs/source/sorafs_orchestrator_rollout.md` 和 `dashboards/grafana/soranet_privacy_metrics.json` 中记录的 SN16 降级面板。 |
|紧急降级/回滚准备|当降级计数器激增、保护目录验证失败或 `/policy/proxy-toggle` 缓冲区记录持续降级事件时触发。 | `docs/source/ops/soranet_transport_rollback.md`、`sorafs_cli guard-directory import` / `guard-cache prune` 日志、`cargo xtask soranet-rollout-capture --label rollback`、事件凭单和通知模板中的清单。 | `dashboards/grafana/soranet_pq_ratchet.json`、`dashboards/grafana/soranet_privacy_metrics.json` 以及两个警报包（`dashboards/alerts/soranet_handshake_rules.yml`、`dashboards/alerts/soranet_privacy_rules.yml`）。 |

- 使用生成的 `rollout_capture.json` 将每个工件存储在 `artifacts/soranet_pq_rollout/<timestamp>_<label>/` 下，以便治理数据包包含记分板、promtool 跟踪和摘要。
- 将上传证据的 SHA256 摘要（会议纪要 PDF、捕获包、警卫快照）附加到晋升会议纪要中，以便无需访问临时集群即可重播议会批准。
- 参考促销票中的遥测计划，以证明 `docs/source/soranet/snnet16_telemetry_plan.md` 仍然是降级词汇和警报阈值的规范来源。

## 仪表板和遥测更新

`dashboards/grafana/soranet_pq_ratchet.json` 现在附带一个“推出计划”注释面板，该面板链接回此剧本并显示当前阶段，以便治理审查可以确认哪个阶段处于活动状态。使面板描述与配置旋钮的未来更改保持同步。

对于警报，请确保现有规则使用 `stage` 标签，以便金丝雀阶段和默认阶段触发单独的策略阈值 (`dashboards/alerts/soranet_handshake_rules.yml`)。

## 回滚钩子

### 默认 → 斜坡（阶段 C → 阶段 B）

1. 使用 `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` 降级协调器（并在 SDK 配置中镜像相同的阶段），以便阶段 B 在整个队列范围内恢复。
2. 通过 `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` 强制客户端进入安全传输配置文件，捕获记录，以便 `/policy/proxy-toggle` 修复工作流程保持可审核状态。
3. 运行 `cargo xtask soranet-rollout-capture --label rollback-default` 以在 `artifacts/soranet_pq_rollout/` 下存档保护目录差异、promtool 输出和仪表板屏幕截图。

### 坡道→金丝雀（B 阶段→A 阶段）

1. 使用 `sorafs_cli guard-directory import --guard-directory guards.json` 导入升级前捕获的保护目录快照，然后重新运行 `sorafs_cli guard-directory verify`，以便降级数据包包含哈希值。
2. 在协调器和客户端配置上设置 `rollout_phase = "canary"`（或用 `anonymity_policy stage-a` 覆盖），然后重播 [PQ 棘轮运行手册](./pq-ratchet-runbook.md) 中的 PQ 棘轮演练以证明降级管道。
3. 在通知治理之前，将更新的 PQ Ratchet 和 SN16 遥测屏幕截图以及警报结果附加到事件日志中。

### 护栏提醒- 每当发生降级时参考 `docs/source/ops/soranet_transport_rollback.md`，并将任何临时缓解措施记录为部署跟踪器中的 `TODO:` 项目以进行后续工作。
- 在回滚之前和之后将 `dashboards/alerts/soranet_handshake_rules.yml` 和 `dashboards/alerts/soranet_privacy_rules.yml` 保持在 `promtool test rules` 覆盖范围内，以便将警报漂移与捕获包一起记录下来。