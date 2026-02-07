---
lang: zh-hant
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

:::注意規範來源
:::

本運行手冊介紹瞭如何運行 SF-2c 容量市場模擬套件並可視化結果指標。它使用 `docs/examples/sorafs_capacity_simulation/` 中的確定性裝置來驗證配額協商、故障轉移處理和端到端削減修復。容量有效負載仍使用`sorafs_manifest_stub capacity`；將 `iroha app sorafs toolkit pack` 用於清單/CAR 包裝流程。

## 1. 生成 CLI 工件

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` 包裝 `sorafs_manifest_stub capacity` 以發出 Norito 有效負載、base64 blob、Torii 請求正文以及 JSON 摘要：

- 參與配額談判場景的三個提供商聲明。
- 在這些提供者之間分配暫存清單的複制順序。
- 斷電前基線、斷電間隔和故障轉移恢復的遙測快照。
- 模擬中斷後請求削減的爭議有效負載。

所有工件都位於 `./artifacts` 下（通過傳遞不同的目錄作為第一個參數來覆蓋）。檢查 `_summary.json` 文件以獲取人類可讀的上下文。

## 2. 聚合結果並發出指標

```bash
./analyze.py --artifacts ./artifacts
```

分析儀產生：

- `capacity_simulation_report.json` - 聚合分配、故障轉移增量和爭議元數據。
- `capacity_simulation.prom` - Prometheus 文本文件指標 (`sorafs_simulation_*`) 適用於節點導出器文本文件收集器或獨立的抓取作業。

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

將文本文件收集器指向 `capacity_simulation.prom`（當使用節點導出器時，將其複製到通過 `--collector.textfile.directory` 傳遞的目錄中）。

## 3.導入Grafana儀表板

1. 在Grafana中，導入`dashboards/grafana/sorafs_capacity_simulation.json`。
2. 將 `Prometheus` 數據源變量綁定到上面配置的抓取目標。
3. 驗證面板：
   - **配額分配 (GiB)** 顯示每個提供商的承諾/分配餘額。
   - 當中斷指標流入時，**故障轉移觸發器**翻轉為*故障轉移活動*。
   - **中斷期間的正常運行時間下降**繪製了提供商 `alpha` 的損失百分比。
   - **請求的削減百分比** 直觀地顯示從爭議裝置中提取的補救比率。

## 4. 預期檢查

- `sorafs_simulation_quota_total_gib{scope="assigned"}` 等於 `600`，而承諾總數仍 >=600。
- `sorafs_simulation_failover_triggered` 報告 `1`，替換提供商指標突出顯示 `beta`。
- `sorafs_simulation_slash_requested` 報告 `alpha` 提供程序標識符的 `0.15`（15% 斜線）。

運行 `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` 以確認 CLI 架構仍接受這些裝置。