---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c14c7c9e998078f3f713b334556944759cd414fd8b7e22312f8731eadaf9345f
source_last_modified: "2026-01-22T14:45:01.319734+00:00"
translation_last_reviewed: 2026-02-07
id: orchestrator-tuning
title: Orchestrator Rollout & Tuning
sidebar_label: Orchestrator Tuning
description: Practical defaults, tuning guidance, and audit checkpoints for taking the multi-source orchestrator to GA.
translator: machine-google-reviewed
---

:::注意规范来源
:::

# Orchestrator 推出和调整指南

本指南以[配置参考](orchestrator-config.md) 和
[多源部署操作手册](multi-source-rollout.md)。它解释了
如何为每个推出阶段调整协调器，如何解释
记分板文物，以及哪些遥测信号必须在之前就位
扩大交通。跨 CLI、SDK 和应用一致地应用建议
自动化，因此每个节点都遵循相同的确定性获取策略。

## 1. 基线参数集

从共享配置模板开始并调整一小组旋钮：
推广工作正在进行中。下表列出了推荐值
最常见的阶段；未列出的值将回退到默认值
`OrchestratorConfig::default()` 和 `FetchOptions::default()`。

|相| `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` |笔记|
|--------------------|-----------------|--------------------------------------------|------------------------------------|--------------------------------------------|------------------------------------|------|
| **实验室/CI** | `3` | `2` | `2` | `2500` | `300` |严格的延迟上限和宽限窗口快速表面嘈杂的遥测。保持较低的重试次数以更快地暴露无效清单。 |
| **分期** | `4` | `3` | `3` | `4000` | `600` |反映生产默认情况，同时为探索性同行留出空间。 |
| **金丝雀** | `6` | `3` | `3` | `5000` | `900` |匹配默认值；设置 `telemetry_region` 以便仪表板可以基于金丝雀流量。 |
| **全面上市** | `None`（使用所有符合条件的）| `4` | `4` | `5000` | `900` |增加重试和失败阈值以吸收瞬态故障，同时审计继续加强确定性。 |

- `scoreboard.weight_scale` 保持默认 `10_000`，除非下游
  系统需要不同的整数分辨率。扩大规模并不
  更改提供商订购；它只会发出更密集的信用分布。
- 在阶段之间迁移时，保留 JSON 包并使用
  `--scoreboard-out` 因此审计跟踪记录了确切的参数集。

## 2.记分牌卫生

记分板结合了清单要求、提供商广告和遥测。
前滚之前：

1. **验证遥测新鲜度。** 确保引用的快照
   `--telemetry-json` 是在配置的宽限窗口内捕获的。参赛作品
   早于配置的 `telemetry_grace_secs` 失败并显示
   `TelemetryStale { last_updated }`。将其视为硬停止并刷新
   在继续之前导出遥测数据。
2. **检查资格原因。** 通过以下方式保留文物
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`。每个条目
   带有包含确切故障原因的 `eligibility` 块。不要覆盖
   能力不匹配或广告过期；修复上游有效负载。
3. **查看权重增量。** 将 `normalised_weight` 字段与
   以前的版本。体重变化 >10% 应与故意广告相关
   或遥测更改，必须在推出日志中确认。
4. **存档工件。** 配置 `scoreboard.persist_path`，以便每次运行都会发出
   最终记分牌快照。将工件附加到发布记录
   与清单和遥测包一起。
5. **记录提供者混合证据。** `scoreboard.json` 元数据_和_
   匹配 `summary.json` 必须公开 `provider_count`，
   `gateway_provider_count`，以及派生的 `provider_mix` 标签，因此审阅者
   可以证明运行是否为 `direct-only`、`gateway-only` 或 `mixed`。
   网关因此捕获报告 `provider_count=0` plus
   `provider_mix="gateway-only"`，而混合运行需要非零计数
   两个来源。 `cargo xtask sorafs-adoption-check` 强制执行这些字段（并且
   当计数/标签不一致时会失败），所以总是一起运行
   `ci/check_sorafs_orchestrator_adoption.sh` 或您定制的捕获脚本
   生成 `adoption_report.json` 证据包。当 Torii 网关为
   参与，将 `gateway_manifest_id`/`gateway_manifest_cid` 保留在记分牌中
   元数据，以便采用门可以将清单信封与
   捕获的提供商组合。

详细字段定义参见
`crates/sorafs_car/src/scoreboard.rs` 和 CLI 摘要结构公开
`sorafs_cli fetch --json-out`。

## CLI 和 SDK 标志参考

`sorafs_cli fetch`（参见 `crates/sorafs_car/src/bin/sorafs_cli.rs`）和
`iroha_cli app sorafs fetch` 包装器 (`crates/iroha_cli/src/commands/sorafs.rs`)
共享相同的协调器配置表面。使用以下标志时
捕获推出证据或重播规范赛程：

共享多源标志参考（仅通过编辑此文件来保持 CLI 帮助和文档同步）：

- `--max-peers=<count>` 限制了有多少合格的提供商能够通过记分板过滤器。保留未设置以从每个符合条件的提供商进行流式传输，仅在有意执行单源回退时设置为 `1`。镜像 SDK 中的 `maxPeers` 旋钮（`SorafsGatewayFetchOptions.maxPeers`、`SorafsGatewayFetchOptions.max_peers`）。
- `--retry-budget=<count>` 转发至 `FetchOptions` 强制执行的每块重试限制。使用调整指南中的卷展表来获取推荐值；收集证据的 CLI 运行必须与 SDK 默认值匹配才能保持奇偶性。
- `--telemetry-region=<label>` 使用区域/环境标签标记 `sorafs_orchestrator_*` Prometheus 系列（和 OTLP 继电器），以便仪表板可以分离实验室、登台、金丝雀和 GA 流量。
- `--telemetry-json=<path>` 注入记分板引用的快照。将 JSON 保留在记分板旁边，以便审核员可以重播运行（因此 `cargo xtask sorafs-adoption-check --require-telemetry` 可以证明哪个 OTLP 流提供了捕获）。
- `--local-proxy-*`（`--local-proxy-mode`、`--local-proxy-norito-spool`、`--local-proxy-kaigi-spool`、`--local-proxy-kaigi-policy`）启用桥观察器挂钩。设置后，编排器通过本地 Norito/Kaigi 代理流式传输块，以便浏览器客户端、防护缓存和 Kaigi 房间收到 Rust 发出的相同收据。
- `--scoreboard-out=<path>`（可选与 `--scoreboard-now=<unix_secs>` 配对）为审核员保留资格快照。始终将持久化的 JSON 与发布票证中引用的遥测和清单工件配对。
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` 在广告元数据之上应用确定性调整。仅在排练时使用这些标志；生产降级必须通过治理工件进行，以便每个节点都应用相同的策略包。
- `--provider-metrics-out` / `--chunk-receipts-out` 保留推出清单引用的每个提供商的运行状况指标和块收据；提交收养证据时附上这两件物品。

示例（使用已发布的夹具）：

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

SDK 通过 Rust 中的 `SorafsGatewayFetchOptions` 使用相同的配置
客户端（`crates/iroha/src/client.rs`），JS 绑定
(`javascript/iroha_js/src/sorafs.js`) 和 Swift SDK
（`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`）。让那些帮手留在里面
与 CLI 默认值同步，以便操作员可以跨自动化复制策略
没有定制的翻译层。

## 3. 获取策略调整

`FetchOptions` 控制重试行为、并发性和验证。当
调整：

- **重试：** 将 `per_chunk_retry_limit` 提高到超过 `4` 可增加恢复
  时间，但有掩盖提供商错误的风险。更喜欢保留 `4` 作为上限
  依靠供应商轮换来发现表现不佳的人员。
- **故障阈值：** `provider_failure_threshold` 控制何时
  提供者在会话的剩余时间内被禁用。将此值与
  重试策略：低于重试预算的阈值强制协调器
  在所有重试用尽之前弹出对等点。
- **并发：** 保留 `global_parallel_limit` 未设置 (`None`)，除非
  特定环境无法满足所公布的范围。设置后，确保
  该值≤提供商流预算的总和，以避免饥饿。
- **验证切换：** `verify_lengths` 和 `verify_digests` 必须保留
  在生产中启用。当混合供应商车队时，它们保证确定性
  正在比赛中；仅在隔离的模糊测试环境中禁用它们。

## 4. 传输和匿名分期

使用 `rollout_phase`、`anonymity_policy` 和 `transport_policy` 字段
代表隐私姿态：- 首选 `rollout_phase="snnet-5"` 并允许默认匿名策略
  跟踪 SNNet-5 里程碑。仅通过 `anonymity_policy_override` 覆盖
  当治理发布签署的指令时。
- 保持 `transport_policy="soranet-first"` 作为基线，而 SNNet-4/5/5a/5b/6a/7/8/12/13 是🈺
  （参见 `roadmap.md`）。仅将 `transport_policy="direct-only"` 用于记录
  降级/合规演习，并等待 PQ 覆盖率审查
  升级到 `transport_policy="soranet-strict"` — 如果出现以下情况，该层将快速失败
  只剩下经典的继电器。
- `write_mode="pq-only"` 仅应在每个写入路径（SDK、
  协调器、治理工具）可以满足 PQ 要求。期间
  推出保留 `write_mode="allow-downgrade"`，以便紧急响应可以依靠
  在直接路线上，而遥测标记降级。
- 防护选择和电路分级依赖于 SoraNet 目录。供应
  签署 `relay_directory` 快照并保留 `guard_set` 缓存以保护
  流失率保持在商定的保留窗口内。记录的缓存指纹
  `sorafs_cli fetch` 构成了推出证据的一部分。

## 5. 降级和合规挂钩

两个协调器子系统有助于在无需人工干预的情况下执行策略：

- **降级修复** (`downgrade_remediation`)：监视器
  `handshake_downgrade_total` 事件，配置后的 `threshold` 为
  超出 `window_secs` 内，强制本地代理进入 `target_mode`
  （默认情况下仅元数据）。保留默认值（`threshold=3`、`window=300`、
  `cooldown=900`）除非事件审查显示不同的模式。记录任何
  覆盖推出日志并确保仪表板跟踪
  `sorafs_proxy_downgrade_state`。
- **合规政策** (`compliance`)：管辖权和清单豁免
  流经治理管理的选择退出列表。切勿内联临时覆盖
  在配置包中；相反，请求签名更新
  `governance/compliance/soranet_opt_outs.json` 并重新部署生成的 JSON。

对于这两个系统，保留生成的配置包并将其包含在
发布证据，以便审计人员可以追踪降档是如何触发的。

## 6. 遥测和仪表板

在扩大推出之前，请确认以下信号已在
目标环境：

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  金丝雀完成后应为零。
- `sorafs_orchestrator_retries_total` 和
  `sorafs_orchestrator_retry_ratio` — 期间应稳定在 10% 以下
  金丝雀并在 GA 后保持在 5% 以下。
- `sorafs_orchestrator_policy_events_total` — 验证预期的
  推出阶段处于活动状态（`stage` 标签）并通过 `outcome` 记录限电。
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — 跟踪 PQ 继电器电源
  政策预期。
- `telemetry::sorafs.fetch.*` 日志目标 — 必须流式传输到共享日志
  已保存 `status=failed` 搜索的聚合器。

从以下位置加载规范的 Grafana 仪表板
`dashboards/grafana/sorafs_fetch_observability.json`（在门户中导出
在 **SoraFS → Fetch Observability** 下），因此区域/清单选择器，
提供者重试热图、块延迟直方图和停顿计数器匹配
SRE 在老化期间会审查什么。将 Alertmanager 规则连接到
`dashboards/alerts/sorafs_fetch_rules.yml` 并验证 Prometheus 语法
与 `scripts/telemetry/test_sorafs_fetch_alerts.sh` （助手自动
在本地或 Docker 中运行 `promtool test rules`）。警报切换需要
与脚本打印相同的路由块，以便操作员可以将证据固定到
推出票。

### 遥测预烧工作流程

路线图项目 **SF-6e** 在翻转之前需要进行 30 天的遥测老化
多源编排器恢复其 GA 默认值。使用存储库脚本
在窗口中每天捕获可复制的工件包：

1.用烧机环境运行`ci/check_sorafs_orchestrator_adoption.sh`
   旋钮设置。示例：

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   助手重播 `fixtures/sorafs_orchestrator/multi_peer_parity_v1`，
   写入 `scoreboard.json`、`summary.json`、`provider_metrics.json`、
   `chunk_receipts.json` 和 `adoption_report.json` 下
   `artifacts/sorafs_orchestrator/<timestamp>/`，并强制执行最小数量
   通过 `cargo xtask sorafs-adoption-check` 列出符合资格的提供商。
2. 当存在预烧变量时，脚本也会发出
   `burn_in_note.json`，捕获标签、日期索引、清单 ID、遥测
   来源和人工制品摘要。将此 JSON 附加到推出日志中，以便它是
   很明显，在 30 天的窗口内，每天的捕获都令人满意。
3.导入更新的Grafana板（`dashboards/grafana/sorafs_fetch_observability.json`）
   进入暂存/生产工作区，用老化标签对其进行标记，然后
   确认每个面板都显示正在测试的清单/区域的样本。
4.运行`scripts/telemetry/test_sorafs_fetch_alerts.sh`（或`promtool test rules …`）
   每当 `dashboards/alerts/sorafs_fetch_rules.yml` 更改以记录该信息时
   警报路由与预烧期间导出的指标相匹配。
5. 归档生成的仪表板快照、警报测试输出和日志尾部
   从 `telemetry::sorafs.fetch.*` 与协调器一起搜索
   人工制品，以便治理可以重播证据，而无需从中提取指标
   实时系统。

## 7. 推出清单

1. 使用候选配置并捕获在 CI 中重新生成记分板
   版本控制下的工件。
2. 在每个环境（实验室、暂存、
   金丝雀，生产）并附上 `--scoreboard-out` 和 `--json-out`
   发布记录中的工件。
3. 与值班工程师一起审查遥测仪表板，确保所有指标
   上面有活体样本。
4. 记录最终的配置路径（通常通过`iroha_config`）和
   用于广告和合规性的治理注册表的 git commit。
5. 更新推出跟踪器并通知 SDK 团队新的默认值，以便客户端
   集成保持一致。

遵循本指南可以保持协调器部署的确定性和可审计性
在提供清晰的反馈循环来调整重试预算的同时，提供商
容量和隐私状况。