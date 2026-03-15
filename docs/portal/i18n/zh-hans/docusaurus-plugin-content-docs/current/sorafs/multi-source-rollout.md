---
id: multi-source-rollout
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Multi-Source Client Rollout & Blacklisting Runbook
sidebar_label: Multi-Source Rollout Runbook
description: Operational checklist for staged multi-source rollouts and emergency provider blacklisting.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
:::

## 目的

本操作手册指导 SRE 和待命工程师完成两个关键工作流程：

1. 在受控波浪中推出多源协调器。
2. 在不破坏现有会话稳定性的情况下将行为不当的提供商列入黑名单或取消优先级。

它假设已经部署了 SF-6 下交付的编排堆栈（`sorafs_orchestrator`、网关块范围 API、遥测导出器）。

> **另请参阅：** [Orchestrator Operations Runbook](./orchestrator-ops.md) 深入探讨每次运行的过程（记分板捕获、分阶段推出切换、回滚）。在实时更改期间一起使用这两个参考。

## 1. 飞行前验证

1. **确认治理投入。**
   - 所有候选提供商必须发布包含范围能力有效负载和流预算的 `ProviderAdvertV1` 信封。通过 `/v1/sorafs/providers` 进行验证并与预期的功能字段进行比较。
   - 在每次金丝雀运行之前，提供延迟/故障率的遥测快照应小于 15 分钟。
2. **舞台配置。**
   - 将 Orchestrator JSON 配置保留在分层 `iroha_config` 树中：

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     使用特定于部署的限制更新 JSON（`max_providers`，重试预算）。将相同的文件提供给暂存/生产，以便差异保持较小。
3. **练习规范装置。**
   - 填充清单/令牌环境变量并运行确定性提取：

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     环境变量应包含参与金丝雀的每个提供者的清单有效负载摘要（十六进制）和 Base64 编码的流令牌。
   - `artifacts/canary.scoreboard.json` 与先前版本的差异。任何新的不合格提供者或体重转移 >10% 都需要审查。
4. **验证遥测是否已接线。**
   - 在 `docs/examples/sorafs_fetch_dashboard.json` 中打开 Grafana 导出。确保在继续操作之前在暂存中填充 `sorafs_orchestrator_*` 指标。

## 2. 紧急服务提供商黑名单

当提供程序提供损坏的块、持续超时或未通过合规性检查时，请遵循此过程。

1. **获取证据。**
   - 导出最新的获取摘要（`--json-out` 输出）。记录失败的块索引、提供者别名和摘要不匹配。
   - 保存 `telemetry::sorafs.fetch.*` 目标的相关日志摘录。
2. **应用立即覆盖。**
   - 在分发给编排器的遥测快照中标记受到处罚的提供商（设置 `penalty=true` 或将 `token_health` 限制为 `0`）。下一个记分板构建将自动排除提供商。
   - 对于临时烟雾测试，将 `--deny-provider gw-alpha` 传递到 `sorafs_cli fetch`，以便在不等待遥测传播的情况下执行故障路径。
   - 将更新后的遥测/配置包重新部署到受影响的环境（暂存 → 金丝雀 → 生产）。在事件日志中记录更改。
3. **验证覆盖。**
   - 重新运行规范夹具获取。确认记分板将提供商标记为不合格，原因为 `policy_denied`。
   - 检查 `sorafs_orchestrator_provider_failures_total` 以确保被拒绝的提供商的计数器停止递增。
4. **升级长期禁令。**
   - 如果提供商将被阻止超过 24 小时，请提出治理票以轮换或暂停其广告。在投票通过之前，保留拒绝列表并刷新遥测快照，以便提供商不会重新进入记分板。
5. **回滚协议。**
   - 要恢复提供程序，请将其从拒绝列表中删除、重新部署并捕获新的记分板快照。将更改附加到事件事后分析中。

## 3. 分阶段推出计划

|相|范围 |所需信号|通过/不通过标准 |
|------|------|------------------|--------------------|
| **实验室** |专用集成集群|针对夹具有效负载进行手动 CLI 获取 |所有块均成功，提供程序失败计数器保持为 0，重试率 < 5%。 |
| **分期** |全面控制平面升级| Grafana 仪表板已连接；仅警告模式下的警报规则 | `sorafs_orchestrator_active_fetches` 每次测试运行后返回到零；没有 `warn/critical` 警报触发。 |
| **金丝雀** | ≤10% 的生产流量 |寻呼机静音但实时监控遥测 |重试率 < 10%，提供者故障与已知的噪音对等点隔离，延迟直方图与分段基线 ±20% 匹配。 |
| **全面上市** | 100% 推出 |寻呼机规则有效 | 24 小时内 `NoHealthyProviders` 错误为零，重试率稳定，仪表板 SLA 面板呈绿色。 |

对于每个阶段：

1. 使用预期的 `max_providers` 更新协调器 JSON 并重试预算。
2. 针对规范夹具和环境中的代表性清单运行 `sorafs_cli fetch` 或 SDK 集成测试套件。
3. 捕获记分板+摘要工件并将其附加到发布记录中。
4. 在进入下一阶段之前，与值班工程师一起检查遥测仪表板。

## 4. 可观察性和事件挂钩

- **指标：** 确保 Alertmanager 监控 `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` 和 `sorafs_orchestrator_retries_total`。突然的峰值通常意味着提供商在负载下性能下降。
- **日志：** 将 `telemetry::sorafs.fetch.*` 目标路由到共享日志聚合器。构建已保存的 `event=complete status=failed` 搜索以加快分类速度。
- **记分板：** 将每个记分板制品保留为长期存储。 JSON 还可以作为合规性审查和分阶段回滚的证据线索。
- **仪表板：** 将规范的 Grafana 板 (`docs/examples/sorafs_fetch_dashboard.json`) 克隆到生产文件夹中，其中包含来自 `docs/examples/sorafs_fetch_alerts.yaml` 的警报规则。

## 5. 沟通和文档

- 在操作变更日志中记录每个拒绝/提升更改，包括时间戳、操作员、原因和相关事件。
- 当提供商权重或重试预算发生变化时通知 SDK 团队，以符合客户端期望。
- GA 完成后，使用部署摘要更新 `status.md`，并在发行说明中存档此 Runbook 参考。