---
lang: zh-hant
direction: ltr
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-12-29T18:16:35.080764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS 容量模擬工具包

該目錄為 SF-2c 容量市場提供可複制的製品
模擬。該工具包執行配額協商、故障轉移處理和削減
使用生產 CLI 幫助程序和輕量級分析腳本進行修復。

## 先決條件

- Rust 工具鏈能夠為工作區成員運行 `cargo run`。
- Python 3.10+（僅限標準庫）。

## 快速入門

```bash
# 1. Generate canonical CLI artefacts
./run_cli.sh ./artifacts

# 2. Aggregate the results and emit Prometheus metrics
./analyze.py --artifacts ./artifacts
```

`run_cli.sh` 腳本調用 `sorafs_manifest_stub capacity` 來構建：

- 配額談判固定集的確定性提供商聲明。
- 與協商場景匹配的複制順序。
- 故障轉移窗口的遙測快照。
- 捕獲削減請求的爭議有效負載。

該腳本寫入 Norito 字節 (`*.to`)、base64 有效負載 (`*.b64`)、Torii 請求
所選工件下的正文和人類可讀的摘要 (`*_summary.json`)
目錄。

`analyze.py` 使用生成的摘要，生成匯總報告
(`capacity_simulation_report.json`)，並發出 Prometheus 文本文件
(`capacity_simulation.prom`)攜帶：

- `sorafs_simulation_quota_*` 描述協商容量和分配的儀表
  每個提供商的份額。
- `sorafs_simulation_failover_*` 儀表突出顯示停機時間增量和所選內容
  替代提供商。
- `sorafs_simulation_slash_requested` 記錄提取的修復百分比
  來自爭議有效負載。

在 `dashboards/grafana/sorafs_capacity_simulation.json` 中導入 Grafana 包
並將其指向 Prometheus 數據源，該數據源會抓取生成的文本文件（例如
例如通過節點導出器文本文件收集器）。運行手冊位於
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` 完整走一遍
工作流程，包括 Prometheus 配置提示。

## 賽程

- `scenarios/quota_negotiation/` — 提供商聲明規範和復制順序。
- `scenarios/failover/` — 主要中斷和故障轉移提升的遙測窗口。
- `scenarios/slashing/` — 引用相同複製順序的爭議規範。

這些裝置在 `crates/sorafs_car/tests/capacity_simulation_toolkit.rs` 中進行了驗證
以保證它們與 CLI 模式保持同步。