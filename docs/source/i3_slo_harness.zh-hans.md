---
lang: zh-hans
direction: ltr
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2025-12-29T18:16:35.966039+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Iroha 3 SLO 线束

Iroha 3 版本系列为关键 Nexus 路径提供显式 SLO：

- 最终时隙持续时间（NX-18 节奏）
- 证明验证（提交证书、JDG 证明、桥接证明）
- 证明端点处理（通过验证延迟的 Axum 路径代理）
- 费用和质押路径（付款人/赞助商和债券/斜线流量）

## 预算

预算位于 `benchmarks/i3/slo_budgets.json` 中并直接映射到替补席
I3套件中的场景。目标是每次调用的 p99 目标：

- 费用/质押：每次调用 50 毫秒（`fee_payer`、`fee_sponsor`、`staking_bond`、`staking_slash`）
- 提交证书/JDG/桥验证：80ms（`commit_cert_verify`、`jdg_attestation_verify`、
  `bridge_proof_verify`)
- 提交证书组装：80ms (`commit_cert_assembly`)
- 访问调度程序：50ms (`access_scheduler`)
- 证明端点代理：120ms (`torii_proof_endpoint`)

刻录率提示 (`burn_rate_fast`/`burn_rate_slow`) 编码 14.4/6.0
寻呼与工单警报的多窗口比率。

## 安全带

通过 `cargo xtask i3-slo-harness` 运行线束：

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

输出：

- `bench_report.json|csv|md` — 原始 I3 基准套件结果（git 哈希 + 场景）
- `slo_report.json|md` — 每个目标的通过/失败/预算比的 SLO 评估

该线束使用预算文件并强制执行 `benchmarks/i3/slo_thresholds.json`
在替补跑期间，当目标退步时，会快速失败。

## 遥测和仪表板

- 最终确定：`histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- 证明验证：`histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

Grafana 入门面板位于 `dashboards/grafana/i3_slo.json` 中。 Prometheus
`dashboards/alerts/i3_slo_burn.yml` 中提供了燃烧率警报，其中
上述预算已纳入（最终确定 2 秒，证明验证 80 毫秒，证明端点代理
120 毫秒）。

## 操作注意事项

- 夜间使用安全带；发布 `artifacts/i3_slo/<stamp>/slo_report.md`
  与治理证据的替补文物一起。
- 如果预算失败，请使用基准降价来确定场景，然后进行钻取
  进入匹配的 Grafana 面板/警报以与实时指标相关联。
- 证明端点 SLO 使用验证延迟作为代理以避免每条路由
  基数爆炸；基准目标 (120ms) 与保留/DoS 匹配
  证明 API 上的护栏。