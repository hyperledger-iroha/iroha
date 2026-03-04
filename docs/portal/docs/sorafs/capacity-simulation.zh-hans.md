---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: de9c2a162d97ab896e51af36054e0f3342522b241bfba3ded9c1ec764e590159
source_last_modified: "2026-01-22T14:35:36.797990+00:00"
translation_last_reviewed: 2026-02-07
id: capacity-simulation
title: SoraFS Capacity Simulation Runbook
sidebar_label: Capacity Simulation Runbook
description: Exercising the SF-2c capacity marketplace simulation toolkit with reproducible fixtures, Prometheus exports, and Grafana dashboards.
translator: machine-google-reviewed
---

:::注意规范来源
:::

本运行手册介绍了如何运行 SF-2c 容量市场模拟套件并可视化结果指标。它使用 `docs/examples/sorafs_capacity_simulation/` 中的确定性装置来验证配额协商、故障转移处理和端到端削减修复。容量有效负载仍使用`sorafs_manifest_stub capacity`；将 `iroha app sorafs toolkit pack` 用于清单/CAR 包装流程。

## 1. 生成 CLI 工件

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` 包装 `sorafs_manifest_stub capacity` 以发出 Norito 有效负载、base64 blob、Torii 请求正文以及 JSON 摘要：

- 参与配额谈判场景的三个提供商声明。
- 在这些提供者之间分配暂存清单的复制顺序。
- 断电前基线、断电间隔和故障转移恢复的遥测快照。
- 模拟中断后请求削减的争议有效负载。

所有工件都位于 `./artifacts` 下（通过传递不同的目录作为第一个参数来覆盖）。检查 `_summary.json` 文件以获取人类可读的上下文。

## 2. 聚合结果并发出指标

```bash
./analyze.py --artifacts ./artifacts
```

分析仪产生：

- `capacity_simulation_report.json` - 聚合分配、故障转移增量和争议元数据。
- `capacity_simulation.prom` - Prometheus 文本文件指标 (`sorafs_simulation_*`) 适用于节点导出器文本文件收集器或独立的抓取作业。

Prometheus 抓取配置示例：

```yaml
scrape_configs:
  - job_name: sorafs-capacity-sim
    scrape_interval: 15s
    static_configs:
      - targets: ["localhost:9100"]
        labels:
          scenario: "capacity-sim"
    metrics_path: /metrics
    params:
      format: ["prometheus"]
```

将文本文件收集器指向 `capacity_simulation.prom`（当使用节点导出器时，将其复制到通过 `--collector.textfile.directory` 传递的目录中）。

## 3.导入Grafana仪表板

1. 在Grafana中，导入`dashboards/grafana/sorafs_capacity_simulation.json`。
2. 将 `Prometheus` 数据源变量绑定到上面配置的抓取目标。
3. 验证面板：
   - **配额分配 (GiB)** 显示每个提供商的承诺/分配余额。
   - 当中断指标流入时，**故障转移触发器**翻转为*故障转移活动*。
   - **中断期间的正常运行时间下降**绘制了提供商 `alpha` 的损失百分比。
   - **请求的削减百分比** 直观地显示从争议装置中提取的补救比率。

## 4. 预期检查

- `sorafs_simulation_quota_total_gib{scope="assigned"}` 等于 `600`，而承诺总数仍 >=600。
- `sorafs_simulation_failover_triggered` 报告 `1`，替换提供商指标突出显示 `beta`。
- `sorafs_simulation_slash_requested` 报告 `alpha` 提供程序标识符的 `0.15`（15% 斜线）。

运行 `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` 以确认 CLI 架构仍接受这些装置。