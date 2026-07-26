---
lang: zh-hant
direction: ltr
source: docs/source/benchmarks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a5420a123c456aad264ceb70d744b20b09848f7dca23700b4ee1370144bb57c
source_last_modified: "2025-12-29T18:16:35.920013+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 基準測試報告

詳細的每次運行快照和 FASTPQ WP5-B 歷史記錄位於
[`benchmarks/history.md`](benchmarks/history.md);附加時使用該索引
路線圖審查或 SRE 審計的工件。重新生成它
每當新 GPU 捕獲時，`python3 scripts/fastpq/update_benchmark_history.py`
或波塞冬顯現陸地。

## 加速證據包

每個 GPU 或混合模式基準測試都必須包含應用的加速設置
因此 WP6-B/WP6-C 可以證明配置奇偶性以及時序偽影。

- 在每次運行之前/之後捕獲運行時快照：
  `cargo xtask acceleration-state --format json > artifacts/acceleration_state_<stamp>.json`
  （使用 `--format table` 來獲取人類可讀的日誌）。此記錄為 `enable_{metal,cuda}`，
  Merkle 閾值、SHA-2 CPU 偏差限制、檢測到的後端運行狀況位以及任何
  粘性奇偶校驗錯誤或禁用原因。
- 將 JSON 存儲在包裝的基準輸出旁邊
  （`artifacts/fastpq_benchmarks/*.json`、`benchmarks/poseidon/*.json`、默克爾掃描
  捕獲等），以便審閱者可以一起比較計時和配置。
- 旋鈕定義和默認值位於 `docs/source/config/acceleration.md` 中；當
  應用覆蓋（例如，`ACCEL_MERKLE_MIN_LEAVES_GPU`、`ACCEL_ENABLE_CUDA`），
  在運行元數據中記下它們，以保持重新運行在主機之間的可重現性。

## Norito stage-1 基準測試 (WP5-B/C)

- 命令：`cargo xtask stage1-bench [--size <bytes|Nk|Nm>]... [--iterations <n>]`
  在 `benchmarks/norito_stage1/` 下發出 JSON + Markdown，並按大小計時
  對於標量與加速結構指數構建器。
- 最新運行（macOS aarch64，開發配置文件）位於
  `benchmarks/norito_stage1/latest.{json,md}` 和來自的新鮮轉換 CSV
  `examples/stage1_cutover` (`benchmarks/norito_stage1/cutover.csv`) 顯示 SIMD
  從 ~6–8KiB 開始獲勝。 GPU/並行 Stage-1 現在默認為 **192KiB**
  截止（`NORITO_STAGE1_GPU_MIN_BYTES=<n>` 覆蓋）以避免啟動抖動
  在小文檔上，同時為更大的有效負載啟用加速器。

## 枚舉與特徵對象調度

- 編譯時間（調試構建）：16.58s
- 運行時間（標準，越低越好）：
  - `enum`：386 ps（平均）
  - `trait_object`：1.56 ns（平均）

這些測量結果來自微基準測試，將基於枚舉的調度與盒裝特徵對象實現進行比較。

## Poseidon CUDA 批處理

Poseidon 基準 (`crates/ivm/benches/bench_poseidon.rs`) 現在包括執行單哈希排列和新的批量幫助程序的工作負載。使用以下命令運行套件：

```bash
cargo bench -p ivm bench_poseidon -- --save-baseline poseidon_cuda
```

Criterion 將在 `target/criterion/poseidon*_many` 下記錄結果。當 GPU Worker 可用時，導出 JSON 摘要（例如，將 `target/criterion/**/new/benchmark.json` 複製到 `benchmarks/poseidon/criterion_poseidon2_many_cuda.json`）（例如，將 `target/criterion/**/new/benchmark.json` 複製到 `benchmarks/poseidon/`），以便下游團隊可以比較每個批量大小的 CPU 與 CUDA 吞吐量。在專用 GPU 通道上線之前，基準測試會回退到 SIMD/CPU 實現，並且仍然為批處理性能提供有用的回歸數據。

對於可重複捕獲（並保留計時數據的奇偶證據），請運行

```bash
cargo xtask poseidon-cuda-bench --json-out benchmarks/poseidon/poseidon_cuda_latest.json \
  --markdown-out benchmarks/poseidon/poseidon_cuda_latest.md --allow-overwrite
```種子確定性 Poseidon2/6 批次，記錄 CUDA 健康/禁用原因，檢查
與標量路徑進行奇偶校驗，並與 Metal 一起發出每秒操作數 + 加速摘要
運行時狀態（功能標誌、可用性、最後一個錯誤）。僅使用 CPU 的主機仍會寫入標量
參考並記下缺少的加速器，這樣即使沒有 GPU，CI 也可以發布工件
跑步者。

## FASTPQ Metal 基準測試（Apple Silicon）

GPU 通道捕獲了 macOS 14 (arm64) 上 `fastpq_metal_bench` 的更新的端到端運行，其中包含通道平衡參數集、20,000 個邏輯行（填充到 32,768 個）和 16 個列組。被包裹的人工製品位於 `artifacts/fastpq_benchmarks/fastpq_metal_bench_20k_refresh.json`，金屬痕跡與先前捕獲的圖像一起存儲在 `traces/fastpq_metal_trace_*_rows20000_iter5.trace` 下。平均時間（來自 `benchmarks.operations[*]`）現在顯示為：

|運營| CPU 平均值（毫秒）|金屬平均值（毫秒）|加速比 (x) |
|----------|--------------|-----------------|------------------------|
| FFT（32,768 個輸入）| 83.29 | 79.95 | 79.95 1.04 | 1.04
| IFFT（32,768 個輸入）| 93.90 | 78.61 | 1.20 | 1.20
| LDE（262,144 個輸入）| 669.54 | 669.54 657.67 | 657.67 1.02 | 1.02
| Poseidon 哈希列（524,288 個輸入）| 29,087.53 | 29,087.53 30,004.90 | 30,004.90 0.97 | 0.97

觀察結果：

- FFT/ IFFT 均受益於更新的 BN254 內核（IFFT 消除了之前的回歸約 20%）。
- LDE 仍接近平價；零填充現在記錄了 33,554,432 個填充字節，平均值為 18.66 毫秒，因此 JSON 包可以捕獲隊列影響。
- Poseidon 哈希在此硬件上仍然受 CPU 限制；繼續與 Poseidon 微基準清單進行比較，直到 Metal 路徑採用最新的隊列控制。
- 現在每次捕獲都會記錄 `AccelerationSettings.runtimeState().metal.lastError`，讓
  工程師用特定的禁用原因（策略切換、
  奇偶校驗失敗，無設備）直接在基準工件中。

要重現運行，請構建 Metal 內核並執行：

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 --output fastpq_metal_bench_20k.json
```

將生成的 JSON 與 Metal 跟踪一起提交到 `artifacts/fastpq_benchmarks/` 下，以便確定性證據保持可重現。

## FASTPQ CUDA 自動化

CUDA 主機可以通過以下方式一步運行並包裝 SM80 基準測試：

```bash
cargo xtask fastpq-cuda-suite \
  --rows 20000 --iterations 5 --columns 16 \
  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \
  --label device_class=xeon-rtx --device rtx-ada
```

幫助器調用 `fastpq_cuda_bench`，通過標籤/設備/註釋進行線程，榮譽
`--require-gpu`，並且（默認情況下）通過 `scripts/fastpq/wrap_benchmark.py` 換行/簽名。
輸出包括原始 JSON、`artifacts/fastpq_benchmarks/` 下的打包包、
輸出旁邊有一個 `<name>_plan.json`，記錄了確切的命令/env，所以
第 7 階段捕獲在 GPU 運行器之間保持可重現。添加 `--sign-output` 和
`--gpg-key <id>`（需要簽名時）；使用 `--dry-run` 僅發出
計劃/路徑而不執行工作台。

### GA 發布捕獲（macOS 14 arm64，通道平衡）

為了滿足 WP2-D，我們還在同一主機上記錄了 GA-ready 的發布版本
隊列啟發式並將其發佈為
`fastpq_metal_bench_20k_release_macos14_arm64.json`。該文物捕獲了兩個
列批次（通道平衡，填充到 32,768 行）並包括 Poseidon
用於儀表板消耗的微基準樣本。|運營| CPU 平均值（毫秒）|金屬平均值（毫秒）|加速|筆記|
|----------|--------------|-----------------|---------|--------|
| FFT（32,768 個輸入）| 12.741 | 12.741 10.963 | 10.963 1.16× | GPU 內核跟踪刷新的隊列閾值。 |
| IFFT（32,768 個輸入）| 17.499 | 17.499 25.688 | 25.688 0.68× |回歸可追溯到保守的隊列扇出；繼續調整啟發式。 |
| LDE（262,144 個輸入）| 68.389 | 68.389 65.701 | 65.701 1.04× |兩個批次的零填充在 9.651 毫秒內記錄了 33,554,432 字節。 |
| Poseidon 哈希列（524,288 個輸入）| 1,728.835 | 1,728.835 1,447.076 | 1,447.076 1.19× |經過 Poseidon 隊列調整後，GPU 最終擊敗了 CPU。 |

JSON 中嵌入的 Poseidon 微基準值顯示 1.10 倍加速（默認通道
596.229ms 與標量 656.251ms 經過五次迭代），因此儀表板現在可以繪製圖表
主板凳旁邊每條車道的改進。使用以下命令重現運行：

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 \
  --output fastpq_metal_bench_20k_release_macos14_arm64.json
```

將封裝的 JSON 和 `FASTPQ_METAL_TRACE_CHILD=1` 跟踪記錄保存在下面
`artifacts/fastpq_benchmarks/` 因此後續的 WP2-D/WP2-E 評論可以區分 GA
針對較早的刷新運行進行捕獲，而無需重新運行工作負載。

每個新的 `fastpq_metal_bench` 捕獲現在也寫入一個 `bn254_metrics` 塊，
它公開了 CPU 的 `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` 條目
基線和處於活動狀態的 GPU 後端（Metal/CUDA），**和**
`bn254_dispatch` 記錄觀察到的線程組寬度、邏輯線程的塊
單列 BN254 FFT/LDE 調度的計數和管道限制。的
基準測試包裝器將兩個映射複製到 `benchmarks.bn254_*` 中，因此儀表板和
Prometheus 導出器可以抓取標記的延遲和幾何形狀，而無需重新解析
原始操作數組。 `FASTPQ_METAL_THREADGROUP` 覆蓋現在適用於
BN254 內核，使線程組掃描可以通過一個旋鈕重現。 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1448】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:3155】【scripts/fastpq/wrap_benchmark.py:1037】

為了使下游儀表板保持簡單，請運行 `python3 scripts/benchmarks/export_csv.py`
捕獲一個包後。助手將 `poseidon_microbench_*.json` 展平為
匹配 `.csv` 文件，以便自動化作業可以區分默認通道和標量通道，而無需
自定義解析器。

## Poseidon 微型工作台（金屬）

`fastpq_metal_bench` 現在在 `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` 下重新執行自身，並將計時提升到 `benchmarks.poseidon_microbench`。我們使用 `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <wrapped_json>` 導出最新的 Metal 捕獲，並通過 `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json` 聚合它們。以下摘要位於 `benchmarks/poseidon/` 下：

|總結|包裹包裹|默認平均值（毫秒）|標量平均值（毫秒）|加速比與標量 |列 x 狀態 |迭代|
|--------------------|----------------|--------------------|------------------|--------------------|--------------------|------------|
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 1,990.49 | 1,990.49 1,994.53 | 1,994.53 1.002 | 1.002 64 x 262,144 | 64 x 262,144 5 |
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2,167.66 | 2,167.66 2,152.18 | 2,152.18 0.993 | 0.993 64 x 262,144 | 64 x 262,144 5 |兩者都通過一次預熱迭代捕獲每次運行的散列 262,144 個狀態（跟踪 log2 = 12）。 “默認”通道對應於調整後的多狀態內核，而“標量”將內核鎖定到每個通道一個狀態以進行比較。

## Merkle 閾值掃描

`merkle_threshold` 示例 (`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`) 強調 Metal-vs-CPU Merkle 哈希路徑。最新的 AppleSilicon 捕獲（Darwin 25.0.0 arm64，`ivm::metal_available()=true`）位於 `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` 中，並具有匹配的 CSV 導出。對於沒有 Metal 的主機，僅 CPU 的 macOS 14 基線仍低於 `benchmarks/merkle_threshold/macos14_arm64_{cpu,metal}.json`。

|葉子| CPU 最佳（毫秒）|金屬最佳（毫秒）|加速|
|--------|-------------|-----------------|---------|
| 1,024 | 1,024 23.01 | 19.69 | 19.69 1.17× |
| 4,096 | 4,096 50.87 | 62.12 | 62.12 0.82× |
| 8,192 | 8,192 95.77 | 95.77 96.57 | 96.57 0.99× |
| 16,384 | 16,384 64.48 | 58.98 | 1.09× |
| 32,768 | 109.49 | 109.49 87.68 | 1.25× |
| 65,536 | 65,536 177.72 | 137.93 | 137.93 1.29× |

較大的葉子數量受益於金屬 (1.09–1.29×)；較小的存​​儲桶在 CPU 上的運行速度仍然更快，因此 CSV 保留兩列進行分析。 CSV 幫助程序在每個配置文件旁邊保留 `metal_available` 標誌，以保持 GPU 與 CPU 回歸儀表板保持一致。

複製步驟：

```bash
cargo run --release -p ivm --features metal --example merkle_threshold -- --json \
  > benchmarks/merkle_threshold/<hostname>_$(uname -r)_$(uname -m).json
```

如果主機需要顯式啟用 Metal，請設置 `FASTPQ_METAL_LIB`/`FASTPQ_GPU`，並保持選中 CPU + GPU 捕獲，以便 WP1-F 可以繪製策略閾值。

從無頭 shell 運行時，設置 `IVM_DEBUG_METAL_ENUM=1` 來記錄設備枚舉，設置 `IVM_FORCE_METAL_ENUM=1` 來繞過 `MTLCreateSystemDefaultDevice()`。 CLI 在請求默認 Metal 設備之前**預熱 CoreGraphics 會話，並在 `MTLCopyAllDevices()` 返回零時回退到 `MTLCreateSystemDefaultDevice()`；如果主機仍然報告沒有設備，則捕獲將保留 `metal_available=false`（有用的 CPU 基線位於 `macos14_arm64_*` 下），而 GPU 主機應保持 `FASTPQ_GPU=metal` 啟用，以便捆綁包記錄所選後端。

`fastpq_metal_bench` 通過 `FASTPQ_DEBUG_METAL_ENUM=1` 公開類似的旋鈕，在後端決定是否保留在 GPU 路徑上之前打印 `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` 結果。每當 `FASTPQ_GPU=gpu` 仍然在包裝的 JSON 中報告 `backend="none"` 時啟用它，以便捕獲包準確記錄主機如何枚舉 Metal 硬件；當設置了 `FASTPQ_GPU=gpu` 但沒有檢測到加速器時，線束會立即中止，指向調試旋鈕，因此發布包永遠不會隱藏強制 GPU 運行後面的 CPU 回退。 【crates/fastpq_prover/src/backend.rs:665】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】

CSV 幫助程序發出每個配置文件的表（例如 `macos14_arm64_*.csv` 和 `takemiyacStudio.lan_25.0.0_arm64.csv`），保留 `metal_available` 標誌，以便回歸儀表板可以獲取 CPU 和 GPU 測量結果，而無需定制解析器。