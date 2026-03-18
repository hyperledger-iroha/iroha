---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9174f3d9559c8656bbe8062e64fc22d93ad654347409798f29231d09ba1628e6
source_last_modified: "2026-01-05T09:28:11.903551+00:00"
translation_last_reviewed: 2026-02-07
id: orchestrator-ops
title: SoraFS Orchestrator Operations Runbook
sidebar_label: Orchestrator Ops Runbook
description: Step-by-step operational guide for rolling out, monitoring, and rolling back the multi-source orchestrator.
translator: machine-google-reviewed
---

:::注意规范来源
:::

本操作手册指导 SRE 准备、部署和操作多源获取编排器。它对开发人员指南进行了补充，提供了针对生产部署进行调整的程序，包括分阶段启用和对等黑名单。

> **另请参阅：** [多源推出运行手册](./multi-source-rollout.md) 重点关注整个舰队的推出浪潮和紧急提供商拒绝。在使用本文档进行日常协调器操作时，请参考它进行治理/分阶段协调。

## 1. 飞行前检查清单

1. **收集提供商的输入**
   - 目标车队的最新提供商广告 (`ProviderAdvertV1`) 和遥测快照。
   - 有效负载计划 (`plan.json`) 源自测试中的清单。
2. **渲染确定性记分板**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```

   - 验证 `artifacts/scoreboard.json` 将每个生产提供商列为 `eligible`。
   - 将摘要 JSON 与记分板一起存档；审核员在验证变更请求时依赖块重试计数器。
3. **使用固定装置进行试运行** — 对 `docs/examples/sorafs_ci_sample/` 中的公共固定装置执行相同的命令，以确保编排器二进制文件在接触生产负载之前与预期版本匹配。

## 2. 分阶段推出程序

1. **金丝雀阶段（≤2个提供商）**
   - 重建记分板并使用 `--max-peers=2` 运行以将协调器限制为一个小子集。
   - 监控：
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - 一旦完整清单获取的重试率保持在 1% 以下并且没有提供商累积失败，则继续。
2. **斜坡阶段（50%提供商）**
   - 增加 `--max-peers` 并使用新的遥测快照重新运行。
   - 坚持每次运行 `--provider-metrics-out` 和 `--chunk-receipts-out`。文物保留≥7天。
3. **全面推出**
   - 删除 `--max-peers`（或将其设置为完整的合格计数）。
   - 在客户端部署中启用协调器模式：通过配置管理系统分发持久记分板和配置 JSON。
   - 更新仪表板以显示 `sorafs_orchestrator_fetch_duration_ms` p95/p99 并重试每个区域的直方图。

## 3. 对等黑名单和提升

使用 CLI 的评分策略覆盖来对不健康的提供商进行分类，而无需等待治理更新。

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```

- `--deny-provider` 从当前会话的考虑中删除列出的别名。
- `--boost-provider=<alias>=<weight>` 提高了提供商的调度程序权重。这些值会添加到标准化记分板权重中，并且仅适用于本地运行。
- 在事件票证中记录覆盖并附加 JSON 输出，以便所有者团队在根本问题得到解决后可以协调状态。

对于永久性更改，请在清除 CLI 覆盖之前修改源遥测（将违规者标记为受处罚）或使用更新的流预算刷新广告。

## 4. 故障分类

当获取失败时：

1. 在重新运行之前捕获以下工件：
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. 检查 `session.summary.json` 中是否有人类可读的错误字符串：
   - `no providers were supplied` → 验证提供商路径和广告。
   - `retry budget exhausted ...` → 增加 `--retry-budget` 或删除不稳定的对等点。
   - `no compatible providers available ...` → 审核违规提供商的范围能力元数据。
3. 将提供商名称与 `sorafs_orchestrator_provider_failures_total` 相关联，并在指标出现峰值时创建后续票证。
4. 使用 `--scoreboard-json` 和捕获的遥测数据重放离线获取，以确定性地重现故障。

## 5.回滚

要恢复 Orchestrator 推出：

2. 删除所有 `--boost-provider` 覆盖，以便记分板恢复为中性权重。
3. 继续抓取 Orchestrator 指标至少一天，以确认没有正在进行的剩余提取。

保持严格的工件捕获和分阶段部署可确保多源编排器可以跨异构提供商群安全运行，同时保持可观察性和审计要求完好无损。