---
id: pq-ratchet-runbook
lang: zh-hans
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet PQ Ratchet Fire Drill
sidebar_label: PQ Ratchet Runbook
description: On-call rehearsal steps for promoting or demoting the staged PQ anonymity policy with deterministic telemetry validation.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
:::

## 目的

本操作手册指导 SoraNet 分阶段后量子 (PQ) 匿名策略的防火演习顺序。当 PQ 供应下降时，运营商会排练升级（阶段 A -> 阶段 B -> 阶段 C）并受控降级回阶段 B/A。该演练验证遥测挂钩（`sorafs_orchestrator_policy_events_total`、`sorafs_orchestrator_brownouts_total`、`sorafs_orchestrator_pq_ratio_*`）并收集事件排练日志的工件。

## 先决条件

- 具有能力加权的最新 `sorafs_orchestrator` 二进制文件（在 `docs/source/soranet/reports/pq_ratchet_validation.md` 中显示的钻取参考处或之后提交）。
- 访问服务于 `dashboards/grafana/soranet_pq_ratchet.json` 的 Prometheus/Grafana 堆栈。
- 名义保护目录快照。在练习之前获取并验证副本：

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

如果源目录仅发布 JSON，请在运行轮换助手之前使用 `soranet-directory build` 将其重新编码为 Norito 二进制文件。

- 使用 CLI 捕获元数据和前期发行人轮换工件：

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- 经网络和可观察性待命团队批准的更改窗口。

## 促销步骤

1. **阶段审核**

   记录起始阶段：

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   升级前预计为 `anon-guard-pq`。

2. **晋升至B阶段（多数PQ）**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - 等待 >=5 分钟以刷新清单。
   - 在 Grafana（`SoraNet PQ Ratchet Drill` 仪表板）中，确认“策略事件”面板显示 `outcome=met` 和 `stage=anon-majority-pq`。
   - 捕获屏幕截图或面板 JSON 并将其附加到事件日志中。

3. **晋升至C阶段（严格PQ）**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - 验证 `sorafs_orchestrator_pq_ratio_*` 直方图趋势为 1.0。
   - 确认掉电计数器保持平稳；否则请遵循降级步骤。

## 降级/限电演习

1. **引起合成PQ短缺**

   通过将守卫目录仅修剪为经典条目来禁用 Playground 环境中的 PQ 中继，然后重新加载 Orchestrator 缓存：

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **观察断电遥测**

   - 仪表板：面板“掉电率”峰值高于 0。
   - PromQL：`sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` 应报告 `anonymity_outcome="brownout"` 和 `anonymity_reason="missing_majority_pq"`。

3. **降级至B阶段/A阶段**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   如果PQ供应仍然不足，则降级为`anon-guard-pq`。一旦限电计数器稳定并且可以重新应用促销活动，演练就完成了。

4. **恢复守卫目录**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## 遥测和文物

- **仪表板：** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus 警报：** 确保 `sorafs_orchestrator_policy_events_total` 掉电警报保持在配置的 SLO 以下（在任何 10 分钟窗口内 <5%）。
- **事件日志：** 将捕获的遥测片段和操作员注释附加到 `docs/examples/soranet_pq_ratchet_fire_drill.log`。
- **签名捕获：**使用 `cargo xtask soranet-rollout-capture` 将演练日志和记分板复制到 `artifacts/soranet_pq_rollout/<timestamp>/`，计算 BLAKE3 摘要，并生成签名的 `rollout_capture.json`。

示例：

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

将生成的元数据和签名附加到治理数据包中。

## 回滚

如果演习发现真正的 PQ 短缺，请留在阶段 A，通知网络 TL，并将收集的指标以及防护目录差异附加到事件跟踪器。使用之前捕获的guard目录导出来恢复正常服务。

:::提示回归覆盖率
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` 提供了支持此演练的综合验证。
:::