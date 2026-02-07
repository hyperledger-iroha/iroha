---
lang: zh-hans
direction: ltr
source: docs/source/benchmarks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a5420a123c456aad264ceb70d744b20b09848f7dca23700b4ee1370144bb57c
source_last_modified: "2025-12-29T18:16:35.920013+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 基准测试报告

详细的每次运行快照和 FASTPQ WP5-B 历史记录位于
[`benchmarks/history.md`](benchmarks/history.md);附加时使用该索引
路线图审查或 SRE 审计的工件。重新生成它
每当新 GPU 捕获时，`python3 scripts/fastpq/update_benchmark_history.py`
或波塞冬显现陆地。

## 加速证据包

每个 GPU 或混合模式基准测试都必须包含应用的加速设置
因此 WP6-B/WP6-C 可以证明配置奇偶性以及时序伪影。

- 在每次运行之前/之后捕获运行时快照：
  `cargo xtask acceleration-state --format json > artifacts/acceleration_state_<stamp>.json`
  （使用 `--format table` 来获取人类可读的日志）。此记录为 `enable_{metal,cuda}`，
  Merkle 阈值、SHA-2 CPU 偏差限制、检测到的后端运行状况位以及任何
  粘性奇偶校验错误或禁用原因。
- 将 JSON 存储在包装的基准输出旁边
  （`artifacts/fastpq_benchmarks/*.json`、`benchmarks/poseidon/*.json`、默克尔扫描
  捕获等），以便审阅者可以一起比较计时和配置。
- 旋钮定义和默认值位于 `docs/source/config/acceleration.md` 中；当
  应用覆盖（例如，`ACCEL_MERKLE_MIN_LEAVES_GPU`、`ACCEL_ENABLE_CUDA`），
  在运行元数据中记下它们，以保持重新运行在主机之间的可重现性。

## Norito stage-1 基准测试 (WP5-B/C)

- 命令：`cargo xtask stage1-bench [--size <bytes|Nk|Nm>]... [--iterations <n>]`
  在 `benchmarks/norito_stage1/` 下发出 JSON + Markdown，并按大小计时
  对于标量与加速结构指数构建器。
- 最新运行（macOS aarch64，开发配置文件）位于
  `benchmarks/norito_stage1/latest.{json,md}` 和来自的新鲜转换 CSV
  `examples/stage1_cutover` (`benchmarks/norito_stage1/cutover.csv`) 显示 SIMD
  从 ~6–8KiB 开始获胜。 GPU/并行 Stage-1 现在默认为 **192KiB**
  截止（`NORITO_STAGE1_GPU_MIN_BYTES=<n>` 覆盖）以避免启动抖动
  在小文档上，同时为更大的有效负载启用加速器。

## 枚举与特征对象调度

- 编译时间（调试构建）：16.58s
- 运行时间（标准，越低越好）：
  - `enum`：386 ps（平均）
  - `trait_object`：1.56 ns（平均）

这些测量结果来自微基准测试，将基于枚举的调度与盒装特征对象实现进行比较。

## Poseidon CUDA 批处理

Poseidon 基准 (`crates/ivm/benches/bench_poseidon.rs`) 现在包括执行单哈希排列和新的批量帮助程序的工作负载。使用以下命令运行套件：

```bash
cargo bench -p ivm bench_poseidon -- --save-baseline poseidon_cuda
```

Criterion 将在 `target/criterion/poseidon*_many` 下记录结果。当 GPU Worker 可用时，导出 JSON 摘要（例如，将 `target/criterion/**/new/benchmark.json` 复制到 `benchmarks/poseidon/criterion_poseidon2_many_cuda.json`）（例如，将 `target/criterion/**/new/benchmark.json` 复制到 `benchmarks/poseidon/`），以便下游团队可以比较每个批量大小的 CPU 与 CUDA 吞吐量。在专用 GPU 通道上线之前，基准测试会回退到 SIMD/CPU 实现，并且仍然为批处理性能提供有用的回归数据。

对于可重复捕获（并保留计时数据的奇偶证据），请运行

```bash
cargo xtask poseidon-cuda-bench --json-out benchmarks/poseidon/poseidon_cuda_latest.json \
  --markdown-out benchmarks/poseidon/poseidon_cuda_latest.md --allow-overwrite
```种子确定性 Poseidon2/6 批次，记录 CUDA 健康/禁用原因，检查
与标量路径进行奇偶校验，并与 Metal 一起发出每秒操作数 + 加速摘要
运行时状态（功能标志、可用性、最后一个错误）。仅使用 CPU 的主机仍会写入标量
参考并记下缺少的加速器，这样即使没有 GPU，CI 也可以发布工件
跑步者。

## FASTPQ Metal 基准测试（Apple Silicon）

GPU 通道捕获了 macOS 14 (arm64) 上 `fastpq_metal_bench` 的更新的端到端运行，其中包含通道平衡参数集、20,000 个逻辑行（填充到 32,768 个）和 16 个列组。被包裹的人工制品位于 `artifacts/fastpq_benchmarks/fastpq_metal_bench_20k_refresh.json`，金属痕迹与先前捕获的图像一起存储在 `traces/fastpq_metal_trace_*_rows20000_iter5.trace` 下。平均时间（来自 `benchmarks.operations[*]`）现在显示为：

|运营| CPU 平均值（毫秒）|金属平均值（毫秒）|加速比 (x) |
|----------|--------------|-----------------|------------------------|
| FFT（32,768 个输入）| 83.29 | 79.95 | 79.95 1.04 | 1.04
| IFFT（32,768 个输入）| 93.90 | 78.61 | 1.20 | 1.20
| LDE（262,144 个输入）| 669.54 | 669.54 657.67 | 657.67 1.02 | 1.02
| Poseidon 哈希列（524,288 个输入）| 29,087.53 | 29,087.53 30,004.90 | 30,004.90 0.97 | 0.97

观察结果：

- FFT/ IFFT 均受益于更新的 BN254 内核（IFFT 消除了之前的回归约 20%）。
- LDE 仍接近平价；零填充现在记录了 33,554,432 个填充字节，平均值为 18.66 毫秒，因此 JSON 包可以捕获队列影响。
- Poseidon 哈希在此硬件上仍然受 CPU 限制；继续与 Poseidon 微基准清单进行比较，直到 Metal 路径采用最新的队列控制。
- 现在每次捕获都会记录 `AccelerationSettings.runtimeState().metal.lastError`，让
  工程师用特定的禁用原因（策略切换、
  奇偶校验失败，无设备）直接在基准工件中。

要重现运行，请构建 Metal 内核并执行：

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 --output fastpq_metal_bench_20k.json
```

将生成的 JSON 与 Metal 跟踪一起提交到 `artifacts/fastpq_benchmarks/` 下，以便确定性证据保持可重现。

## FASTPQ CUDA 自动化

CUDA 主机可以通过以下方式一步运行并包装 SM80 基准测试：

```bash
cargo xtask fastpq-cuda-suite \
  --rows 20000 --iterations 5 --columns 16 \
  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \
  --label device_class=xeon-rtx --device rtx-ada
```

帮助器调用 `fastpq_cuda_bench`，通过标签/设备/注释进行线程，荣誉
`--require-gpu`，并且（默认情况下）通过 `scripts/fastpq/wrap_benchmark.py` 换行/签名。
输出包括原始 JSON、`artifacts/fastpq_benchmarks/` 下的打包包、
输出旁边有一个 `<name>_plan.json`，记录了确切的命令/env，因此
第 7 阶段捕获在 GPU 运行器之间保持可重现。添加 `--sign-output` 和
`--gpg-key <id>`（需要签名时）；使用 `--dry-run` 仅发出
计划/路径而不执行工作台。

### GA 发布捕获（macOS 14 arm64，通道平衡）

为了满足 WP2-D，我们还在同一主机上记录了 GA-ready 的发布版本
队列启发式并将其发布为
`fastpq_metal_bench_20k_release_macos14_arm64.json`。该文物捕获了两个
列批次（通道平衡，填充到 32,768 行）并包括 Poseidon
用于仪表板消耗的微基准样本。|运营| CPU 平均值（毫秒）|金属平均值（毫秒）|加速|笔记|
|----------|--------------|-----------------|---------|--------|
| FFT（32,768 个输入）| 12.741 | 12.741 10.963 | 10.963 1.16× | GPU 内核跟踪刷新的队列阈值。 |
| IFFT（32,768 个输入）| 17.499 | 17.499 25.688 | 25.688 0.68× |回归可追溯到保守的队列扇出；继续调整启发式。 |
| LDE（262,144 个输入）| 68.389 | 68.389 65.701 | 65.701 1.04× |两个批次的零填充在 9.651 毫秒内记录了 33,554,432 字节。 |
| Poseidon 哈希列（524,288 个输入）| 1,728.835 | 1,728.835 1,447.076 | 1,447.076 1.19× |经过 Poseidon 队列调整后，GPU 最终击败了 CPU。 |

JSON 中嵌入的 Poseidon 微基准值显示 1.10 倍加速（默认通道
596.229ms 与标量 656.251ms 经过五次迭代），因此仪表板现在可以绘制图表
主板凳旁边每条车道的改进。使用以下命令重现运行：

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 \
  --output fastpq_metal_bench_20k_release_macos14_arm64.json
```

将封装的 JSON 和 `FASTPQ_METAL_TRACE_CHILD=1` 跟踪记录保存在下面
`artifacts/fastpq_benchmarks/` 因此后续的 WP2-D/WP2-E 评论可以区分 GA
针对较早的刷新运行进行捕获，而无需重新运行工作负载。

每个新的 `fastpq_metal_bench` 捕获现在也写入一个 `bn254_metrics` 块，
它公开了 CPU 的 `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` 条目
基线和处于活动状态的 GPU 后端（Metal/CUDA），**和**
`bn254_dispatch` 记录观察到的线程组宽度、逻辑线程的块
单列 BN254 FFT/LDE 调度的计数和管道限制。的
基准测试包装器将两个映射复制到 `benchmarks.bn254_*` 中，因此仪表板和
Prometheus 导出器可以抓取标记的延迟和几何形状，而无需重新解析
原始操作数组。 `FASTPQ_METAL_THREADGROUP` 覆盖现在适用于
BN254 内核，使线程组扫描可以通过一个旋钮重现。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1448】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:3155】【scripts/fastpq/wrap_benchmark.py:1037】

为了使下游仪表板保持简单，请运行 `python3 scripts/benchmarks/export_csv.py`
捕获一个包后。助手将 `poseidon_microbench_*.json` 展平为
匹配 `.csv` 文件，以便自动化作业可以区分默认通道和标量通道，而无需
自定义解析器。

## Poseidon 微型工作台（金属）

`fastpq_metal_bench` 现在在 `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` 下重新执行自身，并将计时提升到 `benchmarks.poseidon_microbench`。我们使用 `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <wrapped_json>` 导出最新的 Metal 捕获，并通过 `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json` 聚合它们。以下摘要位于 `benchmarks/poseidon/` 下：

|总结|包裹包裹|默认平均值（毫秒）|标量平均值（毫秒）|加速比与标量 |列 x 状态 |迭代|
|--------------------|----------------|--------------------|------------------|--------------------|--------------------|------------|
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 1,990.49 | 1,990.49 1,994.53 | 1,994.53 1.002 | 1.002 64 x 262,144 | 64 x 262,144 5 |
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2,167.66 | 2,167.66 2,152.18 | 2,152.18 0.993 | 0.993 64 x 262,144 | 64 x 262,144 5 |两者都通过一次预热迭代捕获每次运行的散列 262,144 个状态（跟踪 log2 = 12）。 “默认”通道对应于调整后的多状态内核，而“标量”将内核锁定到每个通道一个状态以进行比较。

## Merkle 阈值扫描

`merkle_threshold` 示例 (`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`) 强调 Metal-vs-CPU Merkle 哈希路径。最新的 AppleSilicon 捕获（Darwin 25.0.0 arm64，`ivm::metal_available()=true`）位于 `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` 中，并具有匹配的 CSV 导出。对于没有 Metal 的主机，仅 CPU 的 macOS 14 基线仍低于 `benchmarks/merkle_threshold/macos14_arm64_{cpu,metal}.json`。

|叶子| CPU 最佳（毫秒）|金属最佳（毫秒）|加速|
|--------|-------------|-----------------|---------|
| 1,024 | 1,024 23.01 | 19.69 | 19.69 1.17× |
| 4,096 | 4,096 50.87 | 62.12 | 62.12 0.82× |
| 8,192 | 8,192 95.77 | 95.77 96.57 | 96.57 0.99× |
| 16,384 | 16,384 64.48 | 58.98 | 1.09× |
| 32,768 | 109.49 | 109.49 87.68 | 1.25× |
| 65,536 | 65,536 177.72 | 137.93 | 137.93 1.29× |

较大的叶子数量受益于金属 (1.09–1.29×)；较小的存​​储桶在 CPU 上的运行速度仍然更快，因此 CSV 保留两列进行分析。 CSV 帮助程序在每个配置文件旁边保留 `metal_available` 标志，以保持 GPU 与 CPU 回归仪表板保持一致。

复制步骤：

```bash
cargo run --release -p ivm --features metal --example merkle_threshold -- --json \
  > benchmarks/merkle_threshold/<hostname>_$(uname -r)_$(uname -m).json
```

如果主机需要显式启用 Metal，请设置 `FASTPQ_METAL_LIB`/`FASTPQ_GPU`，并保持选中 CPU + GPU 捕获，以便 WP1-F 可以绘制策略阈值。

从无头 shell 运行时，设置 `IVM_DEBUG_METAL_ENUM=1` 来记录设备枚举，设置 `IVM_FORCE_METAL_ENUM=1` 来绕过 `MTLCreateSystemDefaultDevice()`。 CLI 在请求默认 Metal 设备之前**预热 CoreGraphics 会话，并在 `MTLCopyAllDevices()` 返回零时回退到 `MTLCreateSystemDefaultDevice()`；如果主机仍然报告没有设备，则捕获将保留 `metal_available=false`（有用的 CPU 基线位于 `macos14_arm64_*` 下），而 GPU 主机应保持 `FASTPQ_GPU=metal` 启用，以便捆绑包记录所选后端。

`fastpq_metal_bench` 通过 `FASTPQ_DEBUG_METAL_ENUM=1` 公开类似的旋钮，在后端决定是否保留在 GPU 路径上之前打印 `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` 结果。每当 `FASTPQ_GPU=gpu` 仍然在包装的 JSON 中报告 `backend="none"` 时启用它，以便捕获包准确记录主机如何枚举 Metal 硬件；当设置了 `FASTPQ_GPU=gpu` 但没有检测到加速器时，线束会立即中止，指向调试旋钮，因此发布包永远不会隐藏强制 GPU 运行后面的 CPU 回退。【crates/fastpq_prover/src/backend.rs:665】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】

CSV 帮助程序发出每个配置文件的表（例如 `macos14_arm64_*.csv` 和 `takemiyacStudio.lan_25.0.0_arm64.csv`），保留 `metal_available` 标志，以便回归仪表板可以获取 CPU 和 GPU 测量结果，而无需定制解析器。