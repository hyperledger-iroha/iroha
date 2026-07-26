---
lang: zh-hans
direction: ltr
source: docs/source/benchmarks/history.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3aad1366bd823bddaca32dc82573d41ec6572a6d9f969dc1e0c6146ea068e03e
source_last_modified: "2025-12-29T18:16:35.920451+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# GPU 基准测试捕获历史记录 (FASTPQ WP5-B)

该文件由 `python3 scripts/fastpq/update_benchmark_history.py` 生成。
它通过跟踪每个包装的 GPU 来满足 FASTPQ Stage 7 WP5-B 可交付成果
基准工件、Poseidon 微基准清单和辅助扫描
`benchmarks/`。更新底层捕获并在新捕获时重新运行脚本
捆绑土地或遥测需要新的证据。

## 范围和更新过程

- 生成或包装新的 GPU 捕获（通过 `scripts/fastpq/wrap_benchmark.py`），
  将它们附加到捕获矩阵，然后重新运行该生成器以刷新
  表。
- 当 Poseidon 微基准数据存在时，将其导出
  `scripts/fastpq/export_poseidon_microbench.py` 并使用重建清单
  `scripts/fastpq/aggregate_poseidon_microbench.py`。
- 通过将 JSON 输出存储在下面来记录 Merkle 阈值扫描
  `benchmarks/merkle_threshold/`；该生成器列出了已知文件，以便进行审核
  可以交叉引用 CPU 与 GPU 的可用性。

## FASTPQ 第 7 阶段 GPU 基准测试

|捆绑 |后端 |模式| GPU 后端 | GPU 可用 |设备类别|图形处理器 | LDE 毫秒 (CPU/GPU/SU) | Poseidon MS（CPU/GPU/SU）|
|--------|---------|------|-------------|-------------|--------------|-----|------------------------------------|----------------------------|
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | CUDA |图形处理器| cuda-sm80 |是的 |至强 RTX | NVIDIA RTX 6000 Ada | 1512.9/880.7/1.72 | —/—/—|
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` |金属|图形处理器|无 |是的 |苹果-m4 |苹果GPU 40核| 785.6/735.6/1.07 | 1803.8/1897.5/0.95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` |金属|图形处理器|金属|是的 |苹果-m2-ultra |苹果M2 Ultra | 1581.1/1604.5/0.98 | 3589.9/3697.3/0.97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` |金属|图形处理器|金属|是的 |苹果-m2-ultra |苹果M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` |金属|图形处理器|金属|是的 |苹果-m2-ultra |苹果M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` |开放式 |图形处理器|开放式 |是的 | Neoverse-mi300 | AMD Instinct MI300A | AMD Instinct MI300A | AMD Instinct MI300A 4518.5/688.9/6.56 | 2780.4/905.6/3.07 |

> 列：`Backend` 源自捆绑包名称； `Mode`/`GPU backend`/`GPU available`
> 从包装的 `benchmarks` 块中复制以暴露 CPU 回退或缺少 GPU
> 发现（例如，`gpu_backend=none`，尽管 `Mode=gpu`）。 SU = 加速比（CPU/GPU）。

## Poseidon Microbench 快照

`benchmarks/poseidon/manifest.json` 聚合默认与标量 Poseidon
从每个 Metal 包导出的 microbench 运行。下表刷新为
生成器脚本，因此 CI 和治理审查可以区分历史加速
无需解压打包的 FASTPQ 报告。

|总结|捆绑 |时间戳|默认毫秒 |标量 ms |加速|
|--------|--------|------------|------------|------------|---------|
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06:11:01Z | 2167.7 | 2167.7 2152.2 | 2152.2 0.99 | 0.99
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06:04:07Z | 1990.5 | 1994.5 | 1.00 |

## Merkle 阈值扫描通过收集的参考捕获
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
生活在 `benchmarks/merkle_threshold/` 下。列表条目显示主机是否
扫描运行时暴露的金属设备；支持 GPU 的捕获应该报告
`metal_available=true`。

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

Apple Silicon capture (`takemiyacStudio.lan_25.0.0_arm64`) 是 `docs/source/benchmarks.md` 中使用的规范 GPU 基准；对于无法公开 Metal 设备的环境，macOS 14 条目仍保留为仅 CPU 基准。

## 行使用快照

通过 `scripts/fastpq/check_row_usage.py` 捕获的见证解码证明传输
小工具的行效率。将 JSON 工件保留在 `artifacts/fastpq_benchmarks/` 下
该生成器将为审计员总结记录的转移比率。

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` — 批次=2，传输比率平均值=0.629（最小值=0.625，最大值=0.633）
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — 批次=2，transfer_ratio avg=0.619（最小值=0.613，最大值=0.625）