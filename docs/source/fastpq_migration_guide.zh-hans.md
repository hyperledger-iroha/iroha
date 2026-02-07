---
lang: zh-hans
direction: ltr
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-05T09:28:12.006936+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#！ FASTPQ 生产迁移指南

本运行手册介绍了如何验证 Stage6 生产 FASTPQ 证明器。
作为此迁移计划的一部分，确定性占位符后端已被删除。
它补充了 `docs/source/fastpq_plan.md` 中的分阶段计划，并假设您已经跟踪
`status.md` 中的工作区状态。

## 受众和范围
- 验证器操作员在临时或主网环境中推出生产验证器。
- 发布工程师创建将随生产后端一起提供的二进制文件或容器。
- SRE/可观测性团队连接新的遥测信号和警报。

超出范围：Kotodama 合同编写和 IVM ABI 更改（请参阅 `docs/source/nexus.md` 了解
执行模型）。

## 特征矩阵
|路径|货运功能启用|结果 |何时使用 |
| ---- | ----------------------- | ------ | ----------- |
|生产验证机（默认）| _无_ | Stage6 FASTPQ 后端，带有 FFT/LDE 规划器和 DEEP-FRI 管道。【crates/fastpq_prover/src/backend.rs:1144】 |所有生产二进制文件的默认值。 |
|可选 GPU 加速 | `fastpq_prover/fastpq-gpu` |启用具有自动 CPU 回退功能的 CUDA/Metal 内核。【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 |具有支持的加速器的主机。 |

## 构建过程
1. **仅 CPU 构建**
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   默认编译生产后端；不需要额外的功能。

2. **支持 GPU 的构建（可选）**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   GPU 支持需要 SM80+ CUDA 工具包，并在构建期间提供 `nvcc`。【crates/fastpq_prover/Cargo.toml:11】

3. **自检**
   ```bash
   cargo test -p fastpq_prover
   ```
   每个发布版本运行一次，以在打包之前确认 Stage6 路径。

### Metal 工具链准备 (macOS)
1. 在构建之前安装 Metal 命令行工具：`xcode-select --install`（如果缺少 CLI 工具）和 `xcodebuild -downloadComponent MetalToolchain` 以获取 GPU 工具链。构建脚本直接调用 `xcrun metal`/`xcrun metallib`，如果二进制文件不存在，将会快速失败。【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121】
2. 要在 CI 之前验证管道，您可以在本地镜像构建脚本：
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   当成功时，构建会发出 `FASTPQ_METAL_LIB=<path>`；运行时读取该值以确定性地加载metallib。【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
3、不使用Metal工具链交叉编译时设置`FASTPQ_SKIP_GPU_BUILD=1`；构建打印警告，规划器保留在 CPU 路径上。【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
4. 如果 Metal 不可用（缺少框架、不支持 GPU 或空 `FASTPQ_METAL_LIB`），节点会自动回退到 CPU；构建脚本清除环境变量，规划器记录降级。【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】### 发布清单（第 6 阶段）
保持 FASTPQ 发布票证处于锁定状态，直到完成并附加以下所有项目。

1. **亚秒级证明指标** — 检查新捕获的 `fastpq_metal_bench_*.json` 并
   确认 `benchmarks.operations` 条目，其中 `operation = "lde"`（以及镜像
   `report.operations` 示例）针对 20000 行工作负载（32768 填充
   行）。在签署清单之前，超出上限的捕获需要重新运行。
2. **签名清单** — 运行
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   因此释放票同时带有清单及其独立签名
   （`artifacts/fastpq_bench_manifest.sig`）。审阅者之前验证摘要/签名对
   促进发布。【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】矩阵清单（构建
   通过 `scripts/fastpq/capture_matrix.sh`）已经对 20k 行地板进行了编码，并且
   调试回归。
3. **证据附件** — 上传 Metal 基准 JSON、stdout 日志（或 Instruments 跟踪）、
   CUDA/Metal 清单输出，以及发布票证的独立签名。检查清单条目
   应链接到所有工件以及用于签名的公钥指纹，以便下游审计
   可以重播验证步骤。【artifacts/fastpq_benchmarks/README.md:65】### 金属验证工作流程
1. 在启用 GPU 的构建之后，确认 `FASTPQ_METAL_LIB` 指向 `.metallib` (`echo $FASTPQ_METAL_LIB`)，以便运行时可以确定性地加载它。【crates/fastpq_prover/build.rs:188】
2. 在强制启用 GPU 通道的情况下运行奇偶校验套件：\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`。如果检测失败，后端将执行 Metal 内核并记录确定性 CPU 回退。【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
3. 捕获仪表板的基准示例：\
   找到已编译的 Metal 库 (`fd -g 'fastpq.metallib' target/release/build | head -n1`)，
   通过 `FASTPQ_METAL_LIB` 导出它，然后运行\
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`。
  规范的 `fastpq-lane-balanced` 配置文件现在将每次捕获填充到 32,768 行 (215)，因此 JSON 携带 `rows` 和 `padded_rows` 以及 Metal LD​​E 延迟；如果 `zero_fill` 或队列设置使 GPU LDE 超出 AppleM 系列主机上的 950 毫秒（<1 秒）目标，请重新运行捕获。将生成的 JSON/日志与其他发布证据一起存档；每晚 macOS 工作流程执行相同的运行并上传其工件进行比较。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【.github/workflows/fastpq-metal-nightly.yml:1】
  当您需要仅限 Poseidon 的遥测（例如，记录 Instruments 跟踪）时，请将 `--operation poseidon_hash_columns` 添加到上面的命令中；该工作台仍将尊重 `FASTPQ_GPU=gpu`，发出 `metal_dispatch_queue.poseidon`，并包含新的 `poseidon_profiles` 块，因此发布包明确记录了 Poseidon 瓶颈。
  现在证据包括 `zero_fill.{bytes,ms,queue_delta}` 和 `kernel_profiles`（每个内核
  占用率、估计 GB/s 和持续时间统计数据），因此可以绘制 GPU 效率图表，而无需
  重新处理原始跟踪，以及 `twiddle_cache` 块（命中/未命中 + `before_ms`/`after_ms`）
  证明缓存的旋转上传有效。 `--trace-dir` 重新启动线束
  `xcrun xctrace record` 和
  将带时间戳的 `.trace` 文件与 JSON 一起存储；您仍然可以提供定制服务
  `--trace-output`（可选 `--trace-template` / `--trace-seconds`）
  自定义位置/模板。 JSON记录`metal_trace_{template,seconds,output}`用于审计。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】每次捕获后运行 `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json`，以便发布包含 Grafana 板/警报包（`dashboards/grafana/fastpq_acceleration.json`、`dashboards/alerts/fastpq_acceleration_rules.yml`）的主机元数据（现在包括 `metadata.metal_trace`）。现在，报告为每个操作携带一个 `speedup` 对象（`speedup.ratio`、`speedup.delta_ms`），包装器提升 `zero_fill_hotspots`（字节、延迟、导出的 GB/s 和 Metal 队列增量计数器），将 `kernel_profiles` 展平为`benchmarks.kernel_summary`，保持 `twiddle_cache` 块完整，复制新的 `post_tile_dispatches` 块/摘要，以便审阅者可以证明在捕获期间运行的多通道内核，现在将 Poseidon 微基准证据汇总到 `benchmarks.poseidon_microbench` 中，以便仪表板可以引用标量与默认延迟，而无需重新解析原始报告。清单门读取相同的块并拒绝忽略它的 GPU 证据包，每当跳过后平铺路径或配置错误。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】【xtask/src/fastpq.rs:280】
  Poseidon2 Metal 内核共享相同的旋钮：`FASTPQ_METAL_POSEIDON_LANES`（32-256，2 的幂）和 `FASTPQ_METAL_POSEIDON_BATCH`（每通道 1-32 状态）让您无需重建即可固定启动宽度和每通道工作；主机在每次调度之前通过 `PoseidonArgs` 将这些值线程化。默认情况下，运行时会检查 `MTLDevice::{is_low_power,is_headless,location}`，以使离散 GPU 偏向 VRAM 分层启动（报告 ≥ 48GiB 时为 `256×24`，报告 32GiB 时为 `256×20`，否则为 `256×16`），而低功耗 SoC 仍保留在 `256×8`（及更早版本）上128/64 通道部分坚持每通道 8/6 个状态），因此大多数操作员永远不需要手动设置环境变量。【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】 `fastpq_metal_bench` 现在在 `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` 下重新执行自身并发出信号`poseidon_microbench` 块记录启动配置文件以及测量的加速与标量通道的比较，因此发布包可以证明新内核实际上缩小了 `poseidon_hash_columns`，并且它包含 `poseidon_pipeline` 块，因此 Stage7 证据捕获了块深度/重叠旋钮以及新的占用层。对于正常运行，请保持环境未设置；该工具会自动管理重新执行，如果子捕获无法运行则记录失败，并在设置 `FASTPQ_GPU=gpu` 但没有可用的 GPU 后端时立即退出，因此静默的 CPU 回退永远不会潜入性能人工制品。【板条箱/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【板条箱/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】包装器拒绝缺少 `metal_dispatch_queue.poseidon` 增量、共享 `column_staging` 计数器或 `poseidon_profiles`/`poseidon_microbench` 证据块的 Poseidon 捕获，因此操作员必须刷新无法证明重叠分段或标量与默认值的任何捕获加速。【scripts/fastpq/wrap_benchmark.py:732】当您需要仪表板或 CI 增量的独立 JSON 时，请运行 `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>`；帮助器接受包装的工件和原始 `fastpq_metal_bench*.json` 捕获，使用默认/标量计时、调整元数据和记录的加速来发出 `benchmarks/poseidon/poseidon_microbench_<timestamp>.json`。【scripts/fastpq/export_poseidon_microbench.py:1】
  通过执行 `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json` 来完成运行，以便 Stage6 发布清单强制实施 `<1 s` LDE 上限，并发出随发布票证一起提供的签名清单/摘要包。【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
4. 在推出前验证遥测：curl Prometheus 端点 (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) 并检查 `telemetry::fastpq.execution_mode` 日志中是否有意外的 `resolved="cpu"`条目。【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
5. 通过有意强制（`FASTPQ_GPU=cpu` 或 `zk.fastpq.execution_mode = "cpu"`）来记录 CPU 后备路径，以便 SRE 剧本与确定性行为保持一致。【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】6. 可选调整：默认情况下，主机为短迹线选择 16 个通道，为中迹线选择 32 个通道，一次 `log_len ≥ 10/14` 为 64/128，当 `log_len ≥ 18` 时为 256，现在，它为小迹线选择 5 个阶段的共享内存块，为 `log_len ≥ 12` 一次选择 4 个通道，为 `log_len ≥ 12` 保留 12/14/16 个阶段。 `log_len ≥ 18/20/22` 在将工作踢到后平铺内核之前。在运行上述步骤之前导出 `FASTPQ_METAL_FFT_LANES`（8 和 256 之间的 2 的幂）和/或 `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) 以覆盖这些启发式方法。 FFT/IFFT 和 LDE 列批量大小均源自解析的线程组宽度（每次调度约 2048 个逻辑线程，上限为 32 列，现在随着域的增长逐渐下降到 32→16→8→4→2→1），而 LDE 路径仍然强制执行其域上限；当您需要跨主机进行逐位比较时，设置 `FASTPQ_METAL_FFT_COLUMNS` (1–32) 以固定确定性 FFT 批量大小，并设置 `FASTPQ_METAL_LDE_COLUMNS` (1–32) 以将相同的覆盖应用于 LDE 调度程序。 LDE 切片深度也反映了 FFT 启发式 — 在将宽蝴蝶交给后切片内核之前，`log₂ ≥ 18/20/22` 的跟踪仅运行 12/10/8 共享内存阶段 — 并且您可以通过 `FASTPQ_METAL_LDE_TILE_STAGES` (1–32) 覆盖该限制。运行时通过 Metal 内核参数对所有值进行线程化，限制不支持的覆盖，并记录解析的值，以便实验保持可重复性，而无需重建 Metallib；基准 JSON 显示已解析的调整和通过 LDE 统计信息捕获的主机零填充预算 (`zero_fill.{bytes,ms,queue_delta}`)，因此队列增量直接与每个捕获相关联，现在添加了 `column_staging` 块（批次扁平化、flatten_ms、wait_ms、wait_ratio），以便审阅者可以验证双缓冲管道引入的主机/设备重叠。当 GPU 拒绝报告零填充遥测时，该工具现在会从主机端缓冲区清除中合成确定性时序，并将其注入到 `zero_fill` 块中，因此释放证据永远不会在没有字段.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/bin/fastpq_met al_bench.rs:575】【板条箱/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【板条箱/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. 多队列调度在独立 Mac 上是自动的：当 `Device::is_low_power()` 返回 false 或 Metal 设备报告插槽/外部位置时，主机实例化两个 `MTLCommandQueue`，仅在工作负载承载 ≥16 列（按扇出缩放）时扇出，并跨队列循环列批处理，以便长跟踪使两个 GPU 通道保持繁忙，而不会影响确定性。每当您需要跨机器进行可重复捕获时，请使用 `FASTPQ_METAL_QUEUE_FANOUT`（1–4 个队列）和 `FASTPQ_METAL_COLUMN_THRESHOLD`（扇出前的最小总列数）覆盖该策略；奇偶校验测试强制这些覆盖，以便多 GPU Mac 保持覆盖，并且解析的扇出/阈值记录在队列深度遥测旁边。【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】### 归档证据
|文物 |捕捉|笔记|
|----------|---------|--------|
| `.metallib` 捆绑包 | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` 和 `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"` 后面是 `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` 和 `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`。 |证明 Metal CLI/工具链已安装并为此提交生成了确定性库。【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
|环境快照 |构建后的 `echo $FASTPQ_METAL_LIB`；保留您的发行票的绝对路径。 |输出为空意味着 Metal 被禁用；记录运输工件上 GPU 通道仍然可用的有价值的文件。【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】 |
| GPU 奇偶校验日志 | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` 并存档包含 `backend="metal"` 或降级警告的代码段。 |演示内核在升级构建之前运行（或确定性回退）。【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】 |
|基准输出 | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`；通过 `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]` 包装并签名。 |包装的 JSON 记录 `speedup.ratio`、`speedup.delta_ms`、FFT 调谐、填充行 (32,768)、丰富的 `zero_fill`/`kernel_profiles`、扁平的 `kernel_summary`、验证的`metal_dispatch_queue.poseidon`/`poseidon_profiles` 块（当使用 `--operation poseidon_hash_columns` 时）和跟踪元数据，因此 GPU LDE 平均值保持 ≤950ms，Poseidon 保持 <1s；将捆绑包和生成的 `.json.asc` 签名与发布票一起保留，以便仪表板和审核员可以验证工件，而无需重新运行工作负载。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】|
|替补清单 | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`。 |验证两个 GPU 工件，如果 LDE 均值突破 `<1 s` 上限则失败，记录 BLAKE3/SHA-256 摘要，并发出签名清单，以便发布清单在没有可验证指标的情况下无法推进。【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】 |
| CUDA 捆绑包 |在 SM80 实验室主机上运行 `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json`，将 JSON 包装/签名为 `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json`（使用 `--label device_class=xeon-rtx-sm80`，以便仪表板选择正确的类），添加 `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` 的路径，并保留 `.json`/`.asc` 对重新生成清单之前的金属制品。签入的 `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` 说明了审核员期望的确切捆绑格式。【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】 |
|遥测证明 | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` 加上启动时发出的 `telemetry::fastpq.execution_mode` 日志。 |确认 Prometheus/OTEL 在启用流量之前公开 `device_class="<matrix>", backend="metal"`（或降级日志）。【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】 ||强制CPU钻|使用 `FASTPQ_GPU=cpu` 或 `zk.fastpq.execution_mode = "cpu"` 运行一小批并捕获降级日志。 |使 SRE Runbook 与确定性回退路径保持一致，以防发布中期需要回滚。【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】 |
|跟踪捕获（可选）|使用 `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` 重复奇偶校验测试并保存发出的调度跟踪。 |保留占用/线程组证据，以便以后进行分析审查，而无需重新运行基准测试。【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】 |

多语言 `fastpq_plan.*` 文件引用了此清单，因此登台和生产操作员遵循相同的证据线索。【docs/source/fastpq_plan.md:1】

## 可重复的构建
使用固定容器工作流程来生成可重现的 Stage6 工件：

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

帮助程序脚本构建 `rust:1.88.0-slim-bookworm` 工具链映像（以及用于 GPU 的 `nvidia/cuda:12.2.2-devel-ubuntu22.04`），在容器内运行构建，并将 `manifest.json`、`sha256s.txt` 和编译的二进制文件写入目标输出目录。【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/run_inside_repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.gpu:1】

环境覆盖：
- `FASTPQ_RUST_IMAGE`、`FASTPQ_RUST_TOOLCHAIN` – 固定明确的 Rust 基础/标签。
- `FASTPQ_CUDA_IMAGE` – 生成 GPU 工件时交换 CUDA 基础。
- `FASTPQ_CONTAINER_RUNTIME` – 强制特定的运行时间；默认 `auto` 尝试 `FASTPQ_CONTAINER_RUNTIME_FALLBACKS`。
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` – 用于运行时自动检测的逗号分隔优先顺序（默认为 `docker,podman,nerdctl`）。

## 配置更新
1. 在 TOML 中设置运行时执行模式：
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   该值通过 `FastpqExecutionMode` 解析并在启动时线程到后端。【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. 如果需要，在启动时覆盖：
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   CLI 覆盖会在节点启动之前改变已解析的配置。【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. 开发者可以通过导出来临时强制检测，而无需触及配置
   启动二进制文件之前的 `FASTPQ_GPU={auto,cpu,gpu}`；记录覆盖并且管道
   仍然显示解析模式。【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## 验证清单
1. **启动日志**
   - 期望来自目标 `telemetry::fastpq.execution_mode` 的 `FASTPQ execution mode resolved`
     `requested`、`resolved` 和 `backend` 标签。【crates/fastpq_prover/src/backend.rs:208】
   - 自动 GPU 检测时，来自 `fastpq::planner` 的辅助日志报告最终通道。
   - 当 Metallib 成功加载时，Metal 主机会显示 `backend="metal"`；如果编译或加载失败，构建脚本会发出警告，清除 `FASTPQ_METAL_LIB`，规划器在停留之前记录 `GPU acceleration unavailable` CPU.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover/src/metal.rs:43】2. **Prometheus 指标**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   计数器通过 `record_fastpq_execution_mode` 递增（现在标记为
   `{device_class,chip_family,gpu_kind}`) 每当节点解析其执行时
   模式.【crates/iroha_telemetry/src/metrics.rs:8887】
   - 对于金属覆盖确认
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     与部署仪表板一起递增。【crates/iroha_telemetry/src/metrics.rs:5397】
   - 使用 `irohad --features fastpq-gpu` 编译的 macOS 节点另外暴露
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     和
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` 所以 Stage7 仪表板
     可以从实时 Prometheus 抓取中跟踪占空比和队列余量。【crates/iroha_telemetry/src/metrics.rs:4436】【crates/irohad/src/main.rs:2345】

3. **遥测导出**
   - OTEL 构建发出具有相同标签的 `fastpq.execution_mode_resolutions_total`；确保您的
     当 GPU 应处于活动状态时，仪表板或警报会监视意外的 `resolved="cpu"`。

4. **健全性证明/验证**
   - 通过 `iroha_cli` 或集成工具运行小批量，并在
     同行使用相同的参数编译。

## 故障排除
- **解析模式将 CPU 保留在 GPU 主机上** — 检查二进制文件是否是使用以下命令构建的
  `fastpq_prover/fastpq-gpu`，CUDA库在加载器路径上，并且`FASTPQ_GPU`不强制
  `cpu`。
- **Metal 在 Apple Silicon 上不可用** — 验证 CLI 工具是否已安装 (`xcode-select --install`)，重新运行 `xcodebuild -downloadComponent MetalToolchain`，并确保构建生成非空 `FASTPQ_METAL_LIB` 路径；空值或缺失值会根据设计禁用后端。【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
- **`Unknown parameter` 错误** — 确保证明者和验证者使用相同的规范目录
  由 `fastpq_isi` 发出；表面不匹配为 `Error::UnknownParameter`。【crates/fastpq_prover/src/proof.rs:133】
- **意外的 CPU 回退** — 检查 `cargo tree -p fastpq_prover --features` 并
  确认 GPU 版本中存在 `fastpq_prover/fastpq-gpu`；验证 `nvcc`/CUDA 库位于搜索路径上。
- **遥测计数器丢失** — 验证节点是否使用 `--features telemetry` 启动（默认）
  并且 OTEL 导出（如果启用）包括指标管道。【crates/iroha_telemetry/src/metrics.rs:8887】

## 后备程序
确定性占位符后端已被删除。如果回归需要回滚，
重新部署以前已知的良好发布工件并在重新发布 Stage6 之前进行调查
二进制文件。记录变更管理决策并确保前滚仅在变更后完成
回归是可以理解的。

3. 监控遥测以确保 `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` 反映预期
   占位符执行。

## 硬件基线
|简介 |中央处理器|图形处理器 |笔记|
| -------- | ---| ---| -----|
|参考（第6阶段）| AMD EPYC7B12（32 核）、256GiB RAM | NVIDIA A10040GB (CUDA12.2) | 20000行合成批次必须完成≤1000ms。【docs/source/fastpq_plan.md:131】 |
|仅 CPU | ≥32 个物理核心，AVX2 | – | 20000 行预计约为 0.9–1.2 秒；保留 `execution_mode = "cpu"` 以实现确定性。 |## 回归测试
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu`（在 GPU 主机上）
- 可选的黄金夹具检查：
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

在您的操作手册中记录与此清单的任何偏差，并在执行后更新 `status.md`
迁移窗口完成。