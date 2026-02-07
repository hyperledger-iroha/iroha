---
lang: zh-hant
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

# GPU 基準測試捕獲歷史記錄 (FASTPQ WP5-B)

該文件由 `python3 scripts/fastpq/update_benchmark_history.py` 生成。
它通過跟踪每個包裝的 GPU 來滿足 FASTPQ Stage 7 WP5-B 可交付成果
基準工件、Poseidon 微基準清單和輔助掃描
`benchmarks/`。更新底層捕獲並在新捕獲時重新運行腳本
捆綁土地或遙測需要新的證據。

## 範圍和更新過程

- 生成或包裝新的 GPU 捕獲（通過 `scripts/fastpq/wrap_benchmark.py`），
  將它們附加到捕獲矩陣，然後重新運行該生成器以刷新
  表。
- 當 Poseidon 微基準數據存在時，將其導出
  `scripts/fastpq/export_poseidon_microbench.py` 並使用重建清單
  `scripts/fastpq/aggregate_poseidon_microbench.py`。
- 通過將 JSON 輸出存儲在下面來記錄 Merkle 閾值掃描
  `benchmarks/merkle_threshold/`；該生成器列出了已知文件，以便進行審核
  可以交叉引用 CPU 與 GPU 的可用性。

## FASTPQ 第 7 階段 GPU 基準測試

|捆綁 |後端 |模式| GPU 後端 | GPU 可用 |設備類別|圖形處理器 | LDE 毫秒 (CPU/GPU/SU) | Poseidon MS（CPU/GPU/SU）|
|--------|---------|------|-------------|-------------|--------------|-----|------------------------------------|----------------------------|
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | CUDA |圖形處理器| cuda-sm80 |是的 |至強 RTX | NVIDIA RTX 6000 Ada | 1512.9/880.7/1.72 | —/—/—|
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` |金屬|圖形處理器|無 |是的 |蘋果-m4 |蘋果GPU 40核| 785.6/735.6/1.07 | 1803.8/1897.5/0.95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` |金屬|圖形處理器|金屬|是的 |蘋果-m2-ultra |蘋果M2 Ultra | 1581.1/1604.5/0.98 | 3589.9/3697.3/0.97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` |金屬|圖形處理器|金屬|是的 |蘋果-m2-ultra |蘋果M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` |金屬|圖形處理器|金屬|是的 |蘋果-m2-ultra |蘋果M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` |開放式 |圖形處理器|開放式 |是的 | Neoverse-mi300 | AMD Instinct MI300A | AMD Instinct MI300A | AMD Instinct MI300A 4518.5/688.9/6.56 | 2780.4/905.6/3.07 |

> 列：`Backend` 源自捆綁包名稱； `Mode`/`GPU backend`/`GPU available`
> 從包裝的 `benchmarks` 塊中復制以暴露 CPU 回退或缺少 GPU
> 發現（例如，`gpu_backend=none`，儘管 `Mode=gpu`）。 SU = 加速比（CPU/GPU）。

## Poseidon Microbench 快照

`benchmarks/poseidon/manifest.json` 聚合默認與標量 Poseidon
從每個 Metal 包導出的 microbench 運行。下表刷新為
生成器腳本，因此 CI 和治理審查可以區分歷史加速
無需解壓打包的 FASTPQ 報告。

|總結|捆綁 |時間戳|默認毫秒 |標量 ms |加速|
|--------|--------|------------|------------|------------|---------|
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06:11:01Z | 2167.7 | 2167.7 2152.2 | 2152.2 0.99 | 0.99
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06:04:07Z | 1990.5 | 1994.5 | 1.00 |

## Merkle 閾值掃描通過收集的參考捕獲
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
生活在 `benchmarks/merkle_threshold/` 下。列表條目顯示主機是否
掃描運行時暴露的金屬設備；支持 GPU 的捕獲應該報告
`metal_available=true`。

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

Apple Silicon capture (`takemiyacStudio.lan_25.0.0_arm64`) 是 `docs/source/benchmarks.md` 中使用的規範 GPU 基準；對於無法公開 Metal 設備的環境，macOS 14 條目仍保留為僅 CPU 基準。

## 行使用快照

通過 `scripts/fastpq/check_row_usage.py` 捕獲的見證解碼證明傳輸
小工具的行效率。將 JSON 工件保留在 `artifacts/fastpq_benchmarks/` 下
該生成器將為審計員總結記錄的轉移比率。

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` — 批次=2，傳輸比率平均值=0.629（最小值=0.625，最大值=0.633）
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — 批次=2，transfer_ratio avg=0.619（最小值=0.613，最大值=0.625）