---
lang: zh-hans
direction: ltr
source: docs/source/metal_neon_acceleration_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 628eb2c7776bf818a310dd4bae51e3fc655f92e885d0cd9da7ff487fd9128102
source_last_modified: "2025-12-29T18:16:35.976997+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Metal & NEON 加速计划 (Swift & Rust)

本文档记录了启用确定性硬件的共享计划
加速（Metal GPU + NEON/Accelerate SIMD + StrongBox 集成）
Rust 工作区和 Swift SDK。它解决了跟踪的路线图项目
在 **硬件加速工作流 (macOS/iOS)** 下并提供交接
Rust IVM 团队、Swift 桥所有者和遥测工具的工件。

> 最后更新: 2026-01-12  
> 所有者：IVM Performance TL，Swift SDK 负责人

## 目标

1. 通过 Metal 在 Apple 硬件上重用 Rust GPU 内核 (Poseidon/BN254/CRC64)
   计算着色器与 CPU 路径的确定性奇偶校验。
2. 暴露端到端加速开关 (`AccelerationConfig`) 以便 Swift 应用程序
   可以选择 Metal/NEON/StrongBox，同时保留 ABI/奇偶校验保证。
3. 使用 CI + 仪表板来显示奇偶校验/基准数据和标记
   CPU 与 GPU/SIMD 路径之间的回归。
4. 在 Android (AND2) 和 Swift 之间共享 StrongBox/secure-enclave 课程
   (IOS4) 保持签名流程确定性一致。

**更新（CRC64 + Stage-1 刷新）：** CRC64 GPU 助手现已连接到具有 192KiB 默认截止值的 `norito::core::hardware_crc64`（通过 `NORITO_GPU_CRC64_MIN_BYTES` 或显式助手路径 `NORITO_CRC64_GPU_LIB` 覆盖），同时保留 SIMD 和标量回退。 JSON Stage-1 转换重新进行了基准测试（`examples/stage1_cutover` → `benchmarks/norito_stage1/cutover.csv`），将标量转换保持在 4KiB，并将 Stage-1 GPU 默认值调整为 192KiB (`NORITO_STAGE1_GPU_MIN_BYTES`)，因此小文档保留在 CPU 上，而大负载可以分摊 GPU 启动成本。

## 可交付成果和所有者

|里程碑|可交付成果 |所有者 |目标|
|------------|-------------|----------|--------|
| Rust WP2-A/B |镜像 CUDA 内核的金属着色器接口 | IVM 性能 TL | 2026 年 2 月 |
| Rust WP2-C |金属 BN254 奇偶校验测试和 CI 通道 | IVM 性能 TL | 2026 年第二季度 |
|斯威夫特IOS6 |桥接开关有线 (`connect_norito_set_acceleration_config`) + SDK API + 示例 |斯威夫特大桥业主|完成（2026 年 1 月）|
|斯威夫特IOS5 |演示配置用法的示例应用程序/文档 |斯威夫特 DX TL | 2026 年第二季度 |
|遥测|带有加速奇偶校验 + 基准指标的仪表板提要 | Swift 程序 PM / 遥测 | 2026 年第二季度试点数据 |
| CI | XCFramework 烟雾线束在设备池上对比 CPU 与 Metal/NEON | Swift 质量保证主管 | 2026 年第二季度 |
|保险箱 |硬件支持的签名奇偶校验测试（共享向量）| Android 加密 TL / Swift 安全 | 2026 年第三季度 |

## 接口和 API 合约### 铁锈 (`ivm::AccelerationConfig`)
- 保留现有字段（`enable_simd`、`enable_metal`、`enable_cuda`、`max_gpus`、阈值）。
- 添加显式 Metal 预热以避免首次使用延迟 (Rust #15875)。
- 提供返回仪表板状态/诊断的奇偶校验 API：
  - 例如`ivm::vector::metal_status()` -> {已启用，奇偶校验，最后一个错误}。
- 输出基准测试指标（Merkle 树时序、CRC 吞吐量）
  `ci/xcode-swift-parity` 的遥测挂钩。
- Metal 主机现在加载已编译的 `fastpq.metallib`，调度 FFT/IFFT/LDE
  和 Poseidon 内核，并且每当
  Metallib 或设备队列不可用。

### C FFI (`connect_norito_bridge`)
- 新结构 `connect_norito_acceleration_config`（已完成）。
- Getter 覆盖范围现在包括 `connect_norito_get_acceleration_config`（仅配置）和 `connect_norito_get_acceleration_state`（配置 + 奇偶校验）以镜像 setter。
- SPM/CocoaPods 使用者的标头注释中的文档结构布局。

### 斯威夫特 (`AccelerationSettings`)
- 默认值：Metal 启用，CUDA 禁用，阈值为零（继承）。
- 忽略负值； `apply()` 由 `IrohaSDK` 自动调用。
- `AccelerationSettings.runtimeState()` 现在显示 `connect_norito_get_acceleration_state`
  有效负载（配置 + Metal/CUDA 奇偶校验状态），以便 Swift 仪表板发出相同的遥测数据
  如 Rust (`supported/configured/available/parity`)。当以下情况时，助手返回 `nil`
  缺少桥梁以保持测试的可移植性。
- `AccelerationBackendStatus.lastError` 复制禁用/错误原因
  `connect_norito_get_acceleration_state` 并在字符串被释放后释放本机缓冲区
  实现后，移动奇偶校验仪表板可以注释为什么 Metal/CUDA 被禁用
  每个主机。
- `AccelerationSettingsLoader` (`IrohaSwift/Sources/IrohaSwift/AccelerationSettingsLoader.swift`,
  现在在 `IrohaSwift/Tests/IrohaSwiftTests/AccelerationSettingsLoaderTests.swift` 下测试）
  以与 Norito 演示相同的优先级顺序解析操作符清单：honor
  `NORITO_ACCEL_CONFIG_PATH`，搜索捆绑 `acceleration.{json,toml}` / `client.{json,toml}`，
  记录所选来源，并回退到默认值。应用程序不再需要定制加载器
  镜像 Rust `iroha_config` 表面。
- 更新示例应用程序和自述文件以显示切换和遥测集成。

### 遥测（仪表板 + 导出器）
- 奇偶校验源 (mobile_parity.json)：
  - `acceleration.metal/neon/strongbox` -> {启用、奇偶校验、perf_delta_pct}。
  - 接受 `perf_delta_pct` 基准 CPU 与 GPU 比较。
  - `acceleration.metal.disable_reason` 镜子 `AccelerationBackendStatus.lastError`
    因此 Swift 自动化可以以与 Rust 相同的保真度来标记禁用的 GPU
    仪表板。
- CI 源 (mobile_ci.json)：
  - `acceleration_bench.metal_vs_cpu_merkle_ms` -> {CPU，金属}
  - `acceleration_bench.neon_crc64_throughput_mb_s` -> 双。
- 导出器必须从 Rust 基准测试或 CI 运行中获取指标（例如，运行
  Metal/CPU microbench 作为 `ci/xcode-swift-parity` 的一部分）。### 配置旋钮和默认值 (WP6-C)
- `AccelerationConfig` 默认值：macOS 版本上的 `enable_metal = true`、编译 CUDA 功能时的 `enable_cuda = true`、`max_gpus = None`（无上限）。 Swift `AccelerationSettings` 包装器通过 `connect_norito_set_acceleration_config` 继承相同的默认值。
- Norito Merkle 启发式（GPU 与 CPU）：`merkle_min_leaves_gpu = 8192` 支持对叶子数≥8192 的树进行 GPU 哈希处理；除非明确设置，否则后端覆盖（`merkle_min_leaves_metal`、`merkle_min_leaves_cuda`）默认为相同阈值。
- CPU 优先启发式（SHA2 ISA 存在）：在 AArch64 (ARMv8 SHA2) 和 x86/x86_64 (SHA-NI) 上，CPU 路径在 `prefer_cpu_sha2_max_leaves_* = 32_768` 离开之前保持优先状态；高于该值时，GPU 阈值适用。这些值可通过 `AccelerationConfig` 进行配置，并且只能根据基准证据进行调整。

## 测试策略

1. **单元奇偶校验测试 (Rust)**：确保 Metal 内核与 CPU 输出匹配
   确定性向量；在 `cargo test -p ivm --features metal` 下运行。
   `crates/fastpq_prover/src/metal.rs` 现在提供仅限 macOS 的奇偶校验测试
   针对标量参考练习 FFT/IFFT/LDE 和 Poseidon。
2. **Swift Smoke Harness**：扩展 IOS6 测试运行器来执行 CPU 与 Metal
   在模拟器和 StrongBox 设备上进行编码 (Merkle/CRC64)；比较
   结果和日志奇偶校验状态。
3. **CI**：更新`norito_bridge_ios.yml`（已调用`make swift-ci`）推送
   工件的加速指标；确保运行确认 Buildkite
   `ci/xcframework-smoke:<lane>:device_tag` 发布线束更改之前的元数据，
   并在奇偶性/基准漂移上失败。
4. **仪表板**：新字段现在在 CLI 输出中呈现。确保出口商生产
   实时翻转仪表板之前的数据。

## WP2-A 金属着色器计划（波塞冬管道）

第一个 WP2 里程碑涵盖了 Poseidon Metal 内核的规划工作
反映了 CUDA 的实现。该计划将工作分成几个核心，
主机调度和共享常量暂存，以便以后的工作可以纯粹专注于
实施和测试。

### 内核范围

1. `poseidon_permute`：排列 `state_count` 独立状态。每个线程
   拥有 `STATE_CHUNK`（4 个状态）并使用以下命令运行所有 `TOTAL_ROUNDS` 迭代
   在调度时上演的线程组共享轮常量。
2. `poseidon_hash_columns`：读取稀疏的`PoseidonColumnSlice`目录并
   对每一列执行 Merkle 友好的哈希（与 CPU 的匹配）
   `PoseidonColumnBatch` 布局）。它使用相同的线程组常量缓冲区
   作为置换内核，但在 `(states_per_lane * block_count)` 上循环
   输出，以便内核可以分摊队列提交。
3. `poseidon_trace_fused`：计算跟踪表的父/叶摘要
   在一次通过中。融合内核消耗 `PoseidonFusedArgs`，因此主机
   可以描述非连续区域和 `leaf_offset`/`parent_offset`，并且
   它与其他内核共享所有回合/MDS 表。

### 命令调度和主机合约- 每个内核调度都通过 `MetalPipelines::command_queue` 运行，这
  强制执行自适应调度程序（目标~2 ms）和队列扇出控制
  通过 `FASTPQ_METAL_QUEUE_FANOUT` 暴露和
  `FASTPQ_METAL_COLUMN_THRESHOLD`。 `with_metal_state` 中的预热路径
  预先编译所有三个 Poseidon 内核，因此第一次调度不会
  支付管道创建罚款。
- 线程组大小调整反映了现有的 Metal FFT/LDE 默认值：目标是
  每次提交 8,192 个线程，每组硬上限为 256 个线程。的
  主机可以通过以下方式降低低功耗设备的 `states_per_lane` 乘法器
  拨打环境覆盖 (`FASTPQ_METAL_POSEIDON_STATES_PER_BATCH`
  在 WP2-B 中添加）而不修改着色器逻辑。
- 列分段遵循 FFT 已使用的相同双缓冲池
  管道。 Poseidon 内核接受原始指针到暂存缓冲区
  并且永远不会触及全局堆分配，这保持了内存确定性
  与 CUDA 主机对齐。

### 共享常量

- `PoseidonSnapshot` 清单中描述
  `docs/source/fastpq/poseidon_metal_shared_constants.md` 现在是规范的
  舍入常数和 MDS 矩阵的来源。均为金属 (`poseidon2.metal`)
  每当清单出现时，必须重新生成 CUDA (`fastpq_cuda.cu`) 内核
  变化。
- WP2-B 将添加一个小型主机加载器，在运行时读取清单并
  将 SHA-256 发送到遥测中 (`acceleration.poseidon_constants_sha`)，因此
  奇偶校验仪表板可以断言着色器常量与发布的一致
  快照。
- 在预热期间，我们将把 `TOTAL_ROUNDS x STATE_WIDTH` 常量复制到
  `MTLBuffer` 并为每个设备上传一次。然后每个内核复制数据
  在处理其块之前进入线程组内存，确保确定性
  即使多个命令缓冲区同时运行也能进行排序。

### 验证挂钩

- 单元测试（`cargo test -p fastpq_prover --features fastpq-gpu`）将增长
  哈希嵌入的着色器常量并将它们与
  在执行 GPU 夹具套件之前检查清单的 SHA。
- 现有的内核统计信息切换（`FASTPQ_METAL_TRACE_DISPATCH`，
  `FASTPQ_METAL_QUEUE_FANOUT`，队列深度遥测）成为必需证据
  对于 WP2 退出：每次测试运行都必须证明调度程序从未违反
  配置扇出并且融合跟踪内核将队列保持在
  自适应窗口。
- Swift XCFramework 烟雾线束和 Rust 基准测试运行程序将启动
  导出 `acceleration.poseidon.permute_p90_ms{cpu,metal}` 以便 WP2-D 可以绘制图表
  Metal 与 CPU 的差异无需重新发明新的遥测源。

## WP2-B Poseidon 清单加载器和自检奇偶校验- `fastpq_prover::poseidon_manifest()` 现在嵌入和解析
  `artifacts/offline_poseidon/constants.ron`，计算其 SHA-256
  (`poseidon_manifest_sha256()`)，并根据 CPU 验证快照
  在任何 GPU 工作运行之前的poseidon 表。 `build_metal_context()` 记录
  在预热期间进行摘要，以便遥测出口商可以发布
  `acceleration.poseidon_constants_sha`。
- 清单解析器拒绝不匹配的宽度/速率/轮数元组和
  确保清单 MDS 矩阵等于标量实现，防止
  重新生成规范表时会出现无声漂移。
- 添加了 `crates/fastpq_prover/tests/poseidon_manifest_consistency.rs`，其中
  解析嵌入在 `poseidon2.metal` 中的 Poseidon 表并
  `fastpq_cuda.cu` 并断言两个内核序列化完全相同
  常量作为清单。如果有人编辑着色器/CUDA，CI 现在会失败
  文件而不重新生成规范清单。
- 未来的奇偶校验挂钩 (WP2-C/D) 可以重用 `poseidon_manifest()` 来暂存
  将常量舍入到 GPU 缓冲区中并通过 Norito 公开摘要
  遥测数据。

## WP2-C BN254 金属管道和奇偶校验测试- **范围和差距：** 主机调度程序、奇偶校验工具和 `bn254_status()` 已上线，`crates/fastpq_prover/metal/kernels/bn254.metal` 现在实现 Montgomery 原语以及线程组同步的 FFT/LDE 循环。每个调度在具有每阶段屏障的单个线程组内运行整个列，因此内核并行执行分阶段清单。遥测现在已连接，并且调度程序覆盖得到认可，因此我们可以使用与 Goldilocks 内核相同的证据来控制默认的部署。
- **内核要求：** ✅ 重用分阶段的旋转/陪集清单，转换输入/输出一次，并执行每列线程组内的所有 radix-2 阶段，因此我们不需要多调度同步。蒙哥马利助手在 FFT/LDE 之间保持共享，因此仅循环几何形状发生变化。
- **主机接线：** ✅ `crates/fastpq_prover/src/metal.rs` 阶段规范肢体，零填充 LDE 缓冲区，每列选择一个线程组，并公开 `bn254_status()` 用于选通。遥测不需要额外的主机更改。
- **构建防护：** `fastpq.metallib` 附带平铺内核，因此如果着色器漂移，CI 仍然会快速失败。任何未来的优化都停留在遥测/功能门之后，而不是编译时开关。
- **奇偶校验装置：** ✅ `bn254_parity` 测试继续将 GPU FFT/LDE 输出与 CPU 装置进行比较，并且现在在 Metal 硬件上实时运行；如果出现新的内核代码路径，请记住篡改清单测试。
- **遥测和基准测试：** `fastpq_metal_bench` 现在发出：
  - `bn254_dispatch` 块总结了每个调度线程组宽度、逻辑线程计数以及 FFT/LDE 单列批次的管道限制；和
  - `bn254_metrics` 块，记录 CPU 基线的 `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` 以及运行的 GPU 后端。
  基准包装器将两个映射复制到每个包装的工件中，以便 WP2-D 仪表板摄取标记的延迟/几何，而无需对原始操作数组进行逆向工程。 `FASTPQ_METAL_THREADGROUP` 现在也适用于 BN254 FFT/LDE 调度，使旋钮可用于性能扫描。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1448】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:3155】【scripts/fastpq/wrap_benchmark.py:1037】

## 开放问题（2027 年 5 月解决）1. **Metal资源清理：** `warm_up_metal()`复用线程局部
   `OnceCell` 现在具有幂等/回归测试
   （`crates/ivm/src/vector.rs::warm_up_metal_reuses_cached_state` /
   `warm_up_metal_is_noop_on_non_metal_targets`)，因此应用程序生命周期转换
   可以安全地调用预热路径，而不会泄漏或双重初始化。
2. **基准基线：** 金属通道必须保持在 CPU 的 20% 以内
   FFT/IFFT/LDE 的基线以及 Poseidon CRC/Merkle 助手的 15% 以内；
   当 `acceleration.*_perf_delta_pct > 0.20`（或缺失）时应触发警报
   在移动奇偶校验提要中。在 20k 迹线束中观察到的 IFFT 回归
   现在由 WP2-D 中提到的队列覆盖修复来控制。
3. **StrongBox 后备：** Swift 遵循 Android 后备剧本：
   在支持运行手册中记录证明失败
   (`docs/source/sdk/swift/support_playbook.md`) 并自动切换到
   记录的 HKDF 支持的软件路径以及审计日志记录；奇偶校验向量
   通过现有的 OA 设备保持共享。
4. **遥测存储：** 加速捕获和设备池证明
   存档在 `configs/swift/` 下（例如，
   `configs/swift/xcframework_device_pool_snapshot.json`) 和出口商
   应镜像相同的布局（`artifacts/swift/telemetry/acceleration/*.json`
   或 `.prom`），因此 Buildkite 注释和门户仪表板可以摄取
   无需临时抓取即可馈送。

## 后续步骤（2026 年 2 月）

- [x] Rust：land Metal 主机集成 (`crates/fastpq_prover/src/metal.rs`) 和
      公开 Swift 的内核接口；文档交接与跟踪
      斯威夫特桥梁笔记。
- [x] Swift：公开 SDK 级加速设置（2026 年 1 月完成）。
- [x] 遥测：`scripts/acceleration/export_prometheus.py` 现在转换
      `cargo xtask acceleration-state --format json` 输出为 Prometheus
      文本文件（带有可选的 `--instance` 标签），以便 CI 运行可以附加 GPU/CPU
      启用、阈值和奇偶校验/禁用原因直接写入文本文件
      收藏家无需定制刮擦。
- [x] Swift QA：`scripts/acceleration/acceleration_matrix.py` 聚合多个
      加速状态捕获到由设备键入的 JSON 或 Markdown 表中
      标签，为烟雾线束提供确定性的“CPU vs Metal/CUDA”矩阵
      与示例应用程序一起上传。 Markdown 输出反映了
      构建风筝证据格式，以便仪表板可以摄取相同的人工制品。
- [x] 更新 status.md 现在 `irohad` 运送了队列/零填充导出器并且
      env/config 验证测试涵盖 Metal 队列覆盖，因此 WP2-D
      遥测+绑定附有实时证据。【crates/irohad/src/main.rs:2664】【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【status.md:1546】

遥测/导出帮助程序命令：

```bash
# Prometheus textfile from a single capture
cargo xtask acceleration-state --format json > artifacts/acceleration_state_macos_m4.json
python3 scripts/acceleration/export_prometheus.py \
  --input artifacts/acceleration_state_macos_m4.json \
  --output artifacts/acceleration_state_macos_m4.prom \
  --instance macos-m4

# Aggregate multiple captures into a Markdown matrix
python3 scripts/acceleration/acceleration_matrix.py \
  --state macos-m4=artifacts/acceleration_state_macos_m4.json \
  --state sim-m3=artifacts/acceleration_state_sim_m3.json \
  --format markdown \
  --output artifacts/acceleration_matrix.md
```

## WP2-D 发布基准和绑定说明- **20k 行发布捕获：** 在 macOS14 上记录了新的 Metal 与 CPU 基准测试
  （arm64，通道平衡参数，填充 32,768 行迹线，两列批次）和
  将 JSON 包签入 `fastpq_metal_bench_20k_release_macos14_arm64.json`。
  该基准导出每个操作的时间加上 Poseidon 微基准证据，以便
  WP2-D 具有与新的 Metal 队列启发法相关的 GA 质量制品。标题
  增量（完整表位于 `docs/source/benchmarks.md` 中）：

  |运营| CPU 平均值（毫秒）|金属平均值（毫秒）|加速|
  |----------|--------------|-----------------|---------|
  | FFT（32,768 个输入）| 12.741 | 12.741 10.963 | 10.963 1.16× |
  | IFFT（32,768 个输入）| 17.499 | 17.499 25.688 | 25.688 0.68× *（回归：队列扇出节流以保持确定性；需要后续调整）* |
  | LDE（262,144 个输入）| 68.389 | 68.389 65.701 | 65.701 1.04× |
  | Poseidon 哈希列（524,288 个输入）| 1,728.835 | 1,728.835 1,447.076 | 1,447.076 1.19× |

  每次捕获记录 `zero_fill` 计时（33,554,432 字节为 9.651ms）和
  `poseidon_microbench` 条目（默认通道 596.229ms 与标量 656.251ms，
  1.10× 加速），因此仪表板消费者可以区分队列压力和
  主要业务。
- **绑定/文档交叉链接：** `docs/source/benchmarks.md` 现在引用
  发布 JSON 和重现器命令，Metal 队列覆盖已验证
  通过 `iroha_config` env/manifest 测试，`irohad` 实时发布
  `fastpq_metal_queue_*` 仪表板因此仪表板可以标记 IFFT 回归，而无需
  临时日志抓取。 Swift 的 `AccelerationSettings.runtimeState` 暴露了
  JSON 捆绑包中提供的相同遥测有效负载，关闭 WP2-D
  具有可重复接受基线的绑定/文档差距。【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【crates/irohad/src/main.rs:2664】
- **IFFT 队列修复：** 逆 FFT 批次现在会跳过多队列调度
  工作负载勉强达到扇出阈值（通道平衡的 16 列）
  profile），删除上面提到的 Metal-vs-CPU 回归，同时保留
  FFT/LDE/Poseidon 多队列路径上的大列工作负载。