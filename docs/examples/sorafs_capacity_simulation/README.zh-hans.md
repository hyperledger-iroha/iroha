---
lang: zh-hans
direction: ltr
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-12-29T18:16:35.080764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS 容量模拟工具包

该目录为 SF-2c 容量市场提供可复制的制品
模拟。该工具包执行配额协商、故障转移处理和削减
使用生产 CLI 帮助程序和轻量级分析脚本进行修复。

## 先决条件

- Rust 工具链能够为工作区成员运行 `cargo run`。
- Python 3.10+（仅限标准库）。

## 快速入门

```bash
# 1. Generate canonical CLI artefacts
./run_cli.sh ./artifacts

# 2. Aggregate the results and emit Prometheus metrics
./analyze.py --artifacts ./artifacts
```

`run_cli.sh` 脚本调用 `sorafs_manifest_stub capacity` 来构建：

- 配额谈判固定集的确定性提供商声明。
- 与协商场景匹配的复制顺序。
- 故障转移窗口的遥测快照。
- 捕获削减请求的争议有效负载。

该脚本写入 Norito 字节 (`*.to`)、base64 有效负载 (`*.b64`)、Torii 请求
所选工件下的正文和人类可读的摘要 (`*_summary.json`)
目录。

`analyze.py` 使用生成的摘要，生成汇总报告
(`capacity_simulation_report.json`)，并发出 Prometheus 文本文件
(`capacity_simulation.prom`)携带：

- `sorafs_simulation_quota_*` 描述协商容量和分配的仪表
  每个提供商的份额。
- `sorafs_simulation_failover_*` 仪表突出显示停机时间增量和所选内容
  替代提供商。
- `sorafs_simulation_slash_requested` 记录提取的修复百分比
  来自争议有效负载。

在 `dashboards/grafana/sorafs_capacity_simulation.json` 中导入 Grafana 包
并将其指向 Prometheus 数据源，该数据源会抓取生成的文本文件（例如
例如通过节点导出器文本文件收集器）。运行手册位于
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` 完整走一遍
工作流程，包括 Prometheus 配置提示。

## 赛程

- `scenarios/quota_negotiation/` — 提供商声明规范和复制顺序。
- `scenarios/failover/` — 主要中断和故障转移提升的遥测窗口。
- `scenarios/slashing/` — 引用相同复制顺序的争议规范。

这些装置在 `crates/sorafs_car/tests/capacity_simulation_toolkit.rs` 中进行了验证
以保证它们与 CLI 模式保持同步。