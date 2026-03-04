---
lang: zh-hans
direction: ltr
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2025-12-29T18:16:35.965528+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 3 长凳套件

Iroha 3 工作台套件乘以我们在质押期间依赖的热路径、费用
计费、证明验证、调度和证明端点。它运行为
`xtask` 具有确定性固定装置的命令（固定种子、固定密钥材料、
和稳定的请求负载），因此结果可以跨主机重现。

## 运行套件

```bash
cargo xtask i3-bench-suite \
  --iterations 64 \
  --sample-count 5 \
  --json-out benchmarks/i3/latest.json \
  --csv-out benchmarks/i3/latest.csv \
  --markdown-out benchmarks/i3/latest.md \
  --threshold benchmarks/i3/thresholds.json \
  --allow-overwrite
```

标志：

- `--iterations` 控制每个场景样本的迭代（默认值：64）。
- `--sample-count` 重复每个场景以计算中位数（默认值：5）。
- `--json-out|--csv-out|--markdown-out` 选择输出工件（全部可选）。
- `--threshold` 将中位数与基线界限进行比较（设置 `--no-threshold`
  跳过）。
- `--flamegraph-hint` 用 `cargo flamegraph` 注释 Markdown 报告
  用于分析场景的命令。

CI 胶水位于 `ci/i3_bench_suite.sh` 中，默认为上面的路径；设置
`I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES` 用于调整夜间运行时间。

## 场景

- `fee_payer` / `fee_sponsor` / `fee_insufficient` — 付款人与赞助商借方
  和不足拒绝。
- `staking_bond` / `staking_slash` — 有和没有的绑定/取消绑定队列
  砍伐。
- `commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` —
  对提交证书、JDG 证明和桥接进行签名验证
  证明有效负载。
- `commit_cert_assembly` — 提交证书的摘要汇编。
- `access_scheduler` — 冲突感知访问集调度。
- `torii_proof_endpoint` — Axum 证明端点解析 + 验证往返。

每个场景都会记录每次迭代的中值纳秒、吞吐量和
用于快速回归的确定性分配计数器。阈值生活在
`benchmarks/i3/thresholds.json`；当硬件发生变化时，碰撞会限制在那里
将新工件与报告一起提交。

## 故障排除

- 收集证据时固定 CPU 频率/调速器以避免噪音回归。
- 使用 `--no-threshold` 进行探索性运行，然后在基线达到后重新启用
  神清气爽。
- 要分析单个场景，请设置 `--iterations 1` 并在下重新运行
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`。