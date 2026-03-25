---
lang: zh-hans
direction: ltr
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2025-12-29T18:16:35.932211+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 机密气体校准基线

该分类账跟踪机密气体校准的验证输出
基准。每行记录了使用捕获的发布质量测量集
`docs/source/confidential_assets.md#calibration-baselines--acceptance-gates` 中描述的过程。

|日期 (UTC) |提交 |简介 | `ns/op` | `gas/op` | `ns/gas` |笔记|
| --- | --- | --- | --- | --- | --- | --- |
| 2025年10月18日 | 3c70a7d3 | 3c70a7d3基线霓虹灯 | 2.93e5 | 2.93e5 1.57e2 | 1.57e2 1.87e3 | 1.87e3达尔文 25.0.0 arm64e（主机信息）； `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`； `cargo test -p iroha_core bench_repro -- --ignored`； `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`； `rustc 1.88.0 (6b00bc3)` |
| 2026-04-28 | 8ea9b2a7 | 8ea9b2a7基线-neon-20260428 | 4.29e6 | 4.29e6 1.57e2 | 1.57e2 2.73e4 | 2.73e4达尔文 25.0.0 arm64 (`rustc 1.91.0`)。命令：`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`；登录 `docs/source/confidential_assets_calibration_neon_20260428.log`。 x86_64 奇偶校验运行（SIMD 中性 + AVX2）计划于 2026 年 3 月 19 日苏黎世实验室时段进行；文物将与匹配的命令一起降落在 `artifacts/confidential_assets_calibration/2026-03-x86/` 下，并且一旦捕获就会合并到基线表中。 |
| 2026-04-28 | — |基线 simd 中性 | — | — | — | Apple Silicon 上的**豁免** - `ring` 强制执行平台 ABI 的 NEON，因此 `RUSTFLAGS="-C target-feature=-neon"` 在工作台运行之前会失败 (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`)。中性数据在 CI 主机 `bench-x86-neon0` 上保持门控状态。 |
| 2026-04-28 | — |基线 avx2 | — | — | — | **推迟**直到 x86_64 运行程序可用。 `arch -x86_64` 无法在此计算机上生成二进制文件（“可执行文件中的 CPU 类型错误”；请参阅 `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`）。 CI 主机 `bench-x86-avx2a` 仍然是记录源。 |

`ns/op` 聚合 Criterion 测量的每条指令的中位挂钟；
`gas/op` 是相应调度成本的算术平均值
`iroha_core::gas::meter_instruction`； `ns/gas` 将纳秒总和除以
九个指令样本集的气体总和。

*注意。* 当前的arm64主机不会发出标准`raw.csv`摘要
盒子；在标记之前使用 `CRITERION_OUTPUT_TO=csv` 或上游修复程序重新运行
发布，以便附上验收清单所需的工件。
如果 `--save-baseline` 之后仍然缺少 `target/criterion/`，则收集运行
在 Linux 主机上或将控制台输出序列化到发行包中作为
暂时的权宜之计。作为参考，最新运行的arm64控制台日志
住在 `docs/source/confidential_assets_calibration_neon_20251018.log`。

同一运行中的每条指令中位数 (`cargo bench -p iroha_core --bench isi_gas_calibration`)：

|说明 |中位数 `ns/op` |时间表 `gas` | `ns/gas` |
| --- | --- | --- | --- |
|注册域名 | 3.46e5 | 3.46e5 200 | 200 1.73e3 | 1.73e3
|注册帐号 | 3.15e5 | 3.15e5 200 | 200 1.58e3 | 1.58e3
|注册资产定义| 3.41e5 | 3.41e5 200 | 200 1.71e3 | 1.71e3
|设置AccountKV_small | 3.28e5 | 3.28e5 67 | 67 4.90e3 | 4.90e3
|授予帐户角色 | 3.33e5 | 3.33e5 96 | 96 3.47e3 | 3.47e3
|撤销帐户角色 | 3.12e5 | 3.12e5 96 | 96 3.25e3 | 3.25e3
|执行触发器_空_参数| 1.42e5 | 1.42e5 224 | 224 6.33e2 | 6.33e2
|薄荷资产 | 1.56e5 | 1.56e5 150 | 150 1.04e3 | 1.04e3
|转让资产 | 3.68e5 | 3.68e5 180 | 180 2.04e3 | 2.04e3

### 2026-04-28（Apple Silicon，启用 NEON）

2026 年 4 月 28 日刷新的平均延迟 (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`)：|说明 |中位数 `ns/op` |时间表 `gas` | `ns/gas` |
| ---| ---| ---| ---|
|注册域名 | 8.58e6 | 8.58e6 200 | 200 4.29e4 | 4.29e4
|注册帐号 | 4.40e6 | 4.40e6 200 | 200 2.20e4 | 2.20e4
|注册资产定义| 4.23e6 | 4.23e6 200 | 200 2.12e4 | 2.12e4
|设置AccountKV_small | 3.79e6 | 3.79e6 67 | 67 5.66e4 | 5.66e4
|授予帐户角色 | 3.60e6 | 3.60e6 96 | 96 3.75e4 | 3.75e4
|撤销帐户角色 | 3.76e6 | 3.76e6 96 | 96 3.92e4 | 3.92e4
|执行触发器_空_参数| 2.71e6 | 2.71e6 224 | 224 1.21e4 | 1.21e4
|薄荷资产 | 3.92e6 | 3.92e6 150 | 150 2.61e4 | 2.61e4
|转让资产 | 3.59e6 | 3.59e6 180 | 180 1.99e4 | 1.99e4

上表中的 `ns/op` 和 `ns/gas` 聚合源自以下总和
这些中位数（九个指令集的总 `3.85717e7`ns 和 1,413
气体单位）。

时间表列由 `gas::tests::calibration_bench_gas_snapshot` 强制执行
（九个指令集中总共 1,413 个 Gas）并且如果将来的补丁出现问题将会跳闸
无需更新校准装置即可更改计量。

## 承诺树遥测证据 (M2.2)

根据路线图任务 **M2.2**，每次校准运行都必须捕获新的
承诺树指标和驱逐计数器来证明 Merkle 边界保持不变
在配置的范围内：

- `iroha_confidential_tree_commitments{asset_id}`
- `iroha_confidential_tree_depth{asset_id}`
- `iroha_confidential_root_history_entries{asset_id}`
- `iroha_confidential_frontier_checkpoints{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_height{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
- `iroha_confidential_root_evictions_total{asset_id}`
- `iroha_confidential_frontier_evictions_total{asset_id}`
- `iroha_zk_verifier_cache_events_total{cache,event}`

记录校准工作负载之前和之后的值。一个
每个资产单个命令就足够了； `4cuvDVPuLBKJyN6dPbRQhmLh68sU` 的示例：

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="4cuvDVPuLBKJyN6dPbRQhmLh68sU"}'
```

将原始输出（或 Prometheus 快照）附加到校准票，以便
治理审核者可以确认根历史上限和检查点间隔
很荣幸。 `docs/source/telemetry.md#confidential-tree-telemetry-m22` 中的遥测指南
扩展了警报期望和相关的 Grafana 面板。

在同一个抓取中包含验证者缓存计数器，以便审核者可以确认
未命中率保持在 40% 警告阈值以下：

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

在校准记录中记录导出的比率 (`miss / (hit + miss)`)
展示 SIMD 中性成本建模练习重用热缓存而不是
破坏 Halo2 验证程序注册表。

## 中立和 AVX2 豁免

SDK 委员会授予 PhaseC 门的临时豁免，要求
`baseline-simd-neutral` 和 `baseline-avx2` 测量：

- **SIMD 中性：** 在 Apple Silicon 上，`ring` 加密后端强制使用 NEON
  ABI 正确性。禁用该功能 (`RUSTFLAGS="-C target-feature=-neon"`)
  在生成基准二进制文件之前中止构建 (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`)。
- **AVX2:** 本地工具链无法生成 x86_64 二进制文件 (`arch -x86_64 rustc -V`
  →“可执行文件中的 CPU 类型错误”；看到
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`）。

直到 CI 主机 `bench-x86-neon0` 和 `bench-x86-avx2a` 上线，NEON 才会运行
上述加上遥测证据满足 PhaseC 验收标准。
该豁免记录在 `status.md` 中，一旦 x86 硬件发布，将重新审视该豁免。
可用。