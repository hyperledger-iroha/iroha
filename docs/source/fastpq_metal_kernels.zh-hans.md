---
lang: zh-hans
direction: ltr
source: docs/source/fastpq_metal_kernels.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0022f5f9c53445d26876f0097635092b5c685d332bfa25b13243c584d358dfe
source_last_modified: "2026-01-05T09:28:12.006723+00:00"
translation_last_reviewed: 2026-02-07
title: FASTPQ Metal Kernel Suite
translator: machine-google-reviewed
---

# FASTPQ 金属内核套件

Apple Silicon 后端提供了一个 `fastpq.metallib`，其中包含每个
证明者使用的金属着色语言 (MSL) 内核。这个注释解释了
可用的入口点、它们的线程组限制和确定性
保证 GPU 路径与标量后备可互换。

规范的实现位于
`crates/fastpq_prover/metal/kernels/` 编译为
只要 macOS 上启用了 `fastpq-gpu`，就会出现 `crates/fastpq_prover/build.rs`。
运行时元数据 (`metal_kernel_descriptors`) 镜像以下信息，以便
基准测试和诊断可以通过编程方式显示相同的事实。【crates/fastpq_prover/metal/kernels/ntt_stage.metal:1】【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/build.rs:1】【crates/fastpq_prover/src/metal.rs:248】

## 内核清单|切入点|运营|线程组上限 |瓷砖舞台帽|笔记|
| ----------- | ---------| ---------------- | -------------- | -----|
| `fastpq_fft_columns` |跟踪列上的正向 FFT | 256 线程 | 32 个阶段 |在第一阶段使用共享内存块，并在规划器请求 IFFT 模式时应用反向缩放。【crates/fastpq_prover/metal/kernels/ntt_stage.metal:223】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_fft_post_tiling` |达到图块深度后完成 FFT/IFFT/LDE | 256 线程 | — |直接从设备内存中运行剩余的蝴蝶，并在返回主机之前处理最终的陪集/逆因子。【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_lde_columns` |跨列低度扩展 | 256 线程 | 32 个阶段 |将系数复制到评估缓冲区中，使用配置的陪集执行平铺阶段，并在需要时将最终阶段留给`fastpq_fft_post_tiling`。【crates/fastpq_prover/metal/kernels/ntt_stage.metal:341】【crates/fastpq_prover/src/metal.rs:262】
| `poseidon_trace_fused` |一次性哈希列并计算深度 1 父级 | 256 线程 | — |运行与 `poseidon_hash_columns` 相同的吸收/排列，将叶消化直接存储到输出缓冲区中，并立即将每个 `(left,right)` 对折叠在 `fastpq:v1:trace:node` 域下，以便 `(⌈columns / 2⌉)` 父母在叶切片之后着陆。奇数列计数复制设备上的最终叶子，消除了第一个 Merkle 层的后续内核和 CPU 回退。【crates/fastpq_prover/metal/kernels/poseidon2.metal:384】【crates/fastpq_prover/src/metal.rs:2407】
| `poseidon_permute` | Poseidon2 排列 (STATE_WIDTH = 3) | 256 线程 | — |线程组将轮常量/MDS 行缓存在线程组内存中，将 MDS 行复制到每个线程寄存器中，并在 4 状态块中处理状态，以便每个轮常量获取在前进之前在多个状态中重复使用。回合保持完全展开，每个通道仍然走多个状态，保证每次调度 ≥ 4096 个逻辑线程。 `FASTPQ_METAL_POSEIDON_LANES` / `FASTPQ_METAL_POSEIDON_BATCH` 固定启动宽度和每通道批次，无需重建 Metallib。【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】

描述符可在运行时通过
`fastpq_prover::metal_kernel_descriptors()` 用于想要显示的工具
相同的元数据。

## 确定性金发姑娘算法- 所有内核都在 Goldilocks 字段上工作，并使用中定义的帮助程序
  `field.metal`（模块化加/乘/减，逆，`pow5`）。【crates/fastpq_prover/metal/kernels/field.metal:1】
- FFT/LDE 阶段重用 CPU 规划器生成的相同旋转表。
  `compute_stage_twiddles` 预先计算每个阶段和主机的一次旋转
  每次调度前通过缓冲槽 1 上传数组，保证
  GPU路径使用相同的统一根。【crates/fastpq_prover/src/metal.rs:1527】
- LDE 的陪集乘法被融合到最后阶段，因此 GPU 永远不会
  与 CPU 迹线布局不同；主机对评估缓冲区进行零填充
  在调度之前，保持填充行为确定性。【crates/fastpq_prover/metal/kernels/ntt_stage.metal:288】【crates/fastpq_prover/src/metal.rs:898】

## Metallib 生成

`build.rs` 将各个 `.metal` 源编译为 `.air` 对象，然后
将它们链接到 `fastpq.metallib`，导出上面列出的每个入口点。
将 `FASTPQ_METAL_LIB` 设置为该路径（构建脚本执行此操作
自动）允许运行时确定性地加载库，无论
`cargo` 放置构建工件的位置。【crates/fastpq_prover/build.rs:45】

为了与 CI 运行保持一致，您可以手动重新生成库：

```bash
export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
```

## 线程组大小启发式

`metal_config::fft_tuning` 线程设备执行宽度和每个线程的最大线程数
将线程组放入规划器中，以便运行时调度遵守硬件限制。
随着日志大小的增加，默认限制为 32/64/128/256 通道，并且
平铺深度现在从 `log_len ≥ 12` 的五个阶段变为四个阶段，然后保持
一旦跟踪交叉，共享内存传递将在 12/14/16 阶段处于活动状态
`log_len ≥ 18/20/22` 在将工作交给后平铺内核之前。操作员
覆盖（`FASTPQ_METAL_FFT_LANES`、`FASTPQ_METAL_FFT_TILE_STAGES`）流经
`FftArgs::threadgroup_lanes`/`local_stage_limit` 并由内核应用
以上，无需重建metallib。【crates/fastpq_prover/src/metal_config.rs:12】【crates/fastpq_prover/src/metal.rs:599】

使用 `fastpq_metal_bench` 捕获解析的调整值并验证
之前已执行多遍内核（JSON 中的 `post_tile_dispatches`）
发送基准测试包。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】