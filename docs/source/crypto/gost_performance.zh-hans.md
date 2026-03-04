---
lang: zh-hans
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2025-12-29T18:16:35.939573+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# GOST 性能工作流程

本说明记录了我们如何跟踪和执行绩效范围
TC26 GOST 签名后端。

## 本地运行

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

两个目标在幕后都调用 `scripts/gost_bench.sh`，其中：

1. 执行 `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot`。
2. 针对 `target/criterion` 运行 `gost_perf_check`，根据
   签入基线 (`crates/iroha_crypto/benches/gost_perf_baseline.json`)。
3. 将 Markdown 摘要注入 `$GITHUB_STEP_SUMMARY`（如果可用）。

要在批准回归/改进后刷新基线，请运行：

```bash
make gost-bench-update
```

或直接：

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` 运行工作台 + 检查器，覆盖基线 JSON，并打印
新的中位数。始终将更新的 JSON 与决策记录一起提交
`crates/iroha_crypto/docs/gost_backend.md`。

### 当前参考中位数

|算法|中值（微秒）|
|----------------------|-------------|
| ed25519 | 69.67 | 69.67
| gost256_paramset_a | gost256_paramset_a | 1136.96 | 1136.96
| gost256_paramset_b | gost256_paramset_b | gost256_paramset_b | gost256_paramset_b 1129.05 | 1129.05
| gost256_paramset_c | gost256_paramset_c | gost256_paramset_c 1133.25 | 1133.25
| gost512_paramset_a | gost512_paramset_a | 8944.39 |
| gost512_paramset_b | gost512_paramset_b | gost512_paramset_b | gost512_paramset_b 8963.60 |
| secp256k1 | 160.53 | 160.53

## CI

`.github/workflows/gost-perf.yml` 使用相同的脚本并运行 dudect 定时保护。
当测量的中值超出基线且超出配置的容差时，CI 失败
（默认为 20%）或当计时保护检测到泄漏时，会自动捕获回归。

## 摘要输出

`gost_perf_check` 本地打印对照表，并追加相同内容
`$GITHUB_STEP_SUMMARY`，因此 CI 作业日志和运行摘要共享相同的编号。