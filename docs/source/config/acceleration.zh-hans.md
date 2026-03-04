---
lang: zh-hans
direction: ltr
source: docs/source/config/acceleration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 03275f401b49b62d8ccb361358235e5964b1ca791a68dcada0fd763bb6a4941b
source_last_modified: "2026-01-31T19:25:45.072378+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## 加速和 Norito 启发式参考

`iroha_config` 中的 `[accel]` 块穿过
`crates/irohad/src/main.rs:1895` 转换为 `ivm::set_acceleration_config`。各位主持人
在实例化虚拟机之前应用相同的旋钮，因此操作员可以确定性地
选择允许使用哪些 GPU 后端，同时保持标量/SIMD 后备可用。
Swift、Android 和 Python 绑定通过桥接层加载相同的清单，因此
记录这些默认值可以解除 WP6-C 在硬件加速积压中的阻碍。

### `accel`（硬件加速）

下表反映了 `docs/source/references/peer.template.toml` 和
`iroha_config::parameters::user::Acceleration`定义，暴露环境
覆盖每个键的变量。

|关键|环境变量 |默认|描述 |
|-----|---------|---------|-------------|
| `enable_simd` | `ACCEL_ENABLE_SIMD` | `true` |启用 SIMD/NEON/AVX 执行。当 `false` 时，VM 强制使用标量后端进行向量运算和 Merkle 哈希，以简化确定性奇偶校验捕获。 |
| `enable_cuda` | `ACCEL_ENABLE_CUDA` | `true` |在编译时启用 CUDA 后端并且运行时通过所有黄金向量检查。 |
| `enable_metal` | `ACCEL_ENABLE_METAL` | `true` |在 macOS 版本上启用 Metal 后端。即使为 true，如果发生奇偶校验不匹配，Metal 自检仍然可以在运行时禁用后端。 |
| `max_gpus` | `ACCEL_MAX_GPUS` | `0`（自动）|限制运行时初始化的物理 GPU 数量。 `0` 表示“匹配硬件扇出”，并由 `GpuManager` 钳位。 |
| `merkle_min_leaves_gpu` | `ACCEL_MERKLE_MIN_LEAVES_GPU` | `8192` | Merkle 叶散列卸载到 GPU 之前所需的最少叶数。低于此阈值的值会在 CPU 上继续进行哈希处理，以避免 PCIe 开销 (`crates/ivm/src/byte_merkle_tree.rs:49`)。 |
| `merkle_min_leaves_metal` | `ACCEL_MERKLE_MIN_LEAVES_METAL` | `0`（继承全局）| GPU 阈值的 Metal 特定覆盖。当 `0` 时，Metal 继承 `merkle_min_leaves_gpu`。 |
| `merkle_min_leaves_cuda` | `ACCEL_MERKLE_MIN_LEAVES_CUDA` | `0`（继承全局）| GPU 阈值的 CUDA 特定覆盖。当`0`时，CUDA继承`merkle_min_leaves_gpu`。 |
| `prefer_cpu_sha2_max_leaves_aarch64` | `ACCEL_PREFER_CPU_SHA2_MAX_AARCH64` | `0`（内部 32768）|限制 ARMv8 SHA-2 指令应胜过 GPU 哈希的树大小。 `0` 保留 `32_768` 叶 (`crates/ivm/src/byte_merkle_tree.rs:59`) 的编译默认值。 |
| `prefer_cpu_sha2_max_leaves_x86` | `ACCEL_PREFER_CPU_SHA2_MAX_X86` | `0`（内部 32768）|与上面相同，但适用于使用 SHA-NI (`crates/ivm/src/byte_merkle_tree.rs:63`) 的 x86/x86_64 主机。 |

`enable_simd` 还控制 RS16 擦除编码（Torii DA 摄取 + 工具）。禁用它以
强制标量奇偶校验生成，同时保持硬件输出的确定性。

配置示例：

```toml
[accel]
enable_simd = true
enable_cuda = true
enable_metal = true
max_gpus = 2
merkle_min_leaves_gpu = 12288
merkle_min_leaves_metal = 8192
merkle_min_leaves_cuda = 16384
prefer_cpu_sha2_max_leaves_aarch64 = 65536
```

最后五个键的零值意味着“保留编译的默认值”。主办方不得
设置冲突的覆盖（例如，禁用 CUDA，同时强制仅使用 CUDA 阈值），
否则请求将被忽略，后端继续遵循全局策略。

### 检查运行时状态运行 `cargo xtask acceleration-state [--format table|json]` 对应用的快照
配置以及 Metal/CUDA 运行时健康位。该命令拉取
当前 `ivm::acceleration_config`、奇偶校验状态和粘性错误字符串（如果
后端被禁用），因此操作可以将结果直接输入奇偶校验
仪表板或事件回顾。

```
$ cargo xtask acceleration-state
Acceleration Configuration
--------------------------
enable_simd: yes
enable_metal: yes
enable_cuda: no
max_gpus: 1
merkle_min_leaves_gpu: 8192
merkle_min_leaves_metal: 8192
merkle_min_leaves_cuda: auto
prefer_cpu_sha2_max_leaves_aarch64: auto
prefer_cpu_sha2_max_leaves_x86: auto

Backend Status
--------------
Backend Supported  Configured  Available  ParityOK  Last error
SIMD    yes        yes         yes        yes       -
Metal   yes        yes         yes        yes       -
CUDA    no         no          no         no        policy disabled (no CUDA libraries present)
```

当快照需要通过自动化摄取时，使用 `--format json`（JSON
包含表中所示的相同字段）。

`acceleration_runtime_errors()` 现在指出了 SIMD 回退到标量的原因：
`disabled by config`、`forced scalar override`、`simd unsupported on hardware` 或
`simd unavailable at runtime` 当检测成功但执行仍然运行时
没有向量。清除覆盖或重新启用策略会删除该消息
在支持 SIMD 的主机上。

### 奇偶校验

在仅 CPU 和加速之间翻转 `AccelerationConfig` 以证明确定性结果。
`poseidon_instructions_match_across_acceleration_configs` 回归运行
Poseidon2/6 操作码两次 — 首先将 `enable_cuda`/`enable_metal` 设置为 `false`，然后
两者都启用，并在存在 GPU 时断言相同的输出以及 CUDA 奇偶校验。【crates/ivm/tests/crypto.rs:100】
捕获 `acceleration_runtime_status()` 并记录后端是否运行
已在实验室日志中配置/可用。

```rust
let baseline = ivm::acceleration_config();
ivm::set_acceleration_config(AccelerationConfig {
    enable_cuda: false,
    enable_metal: false,
    ..baseline
});
// run CPU-only parity workload
ivm::set_acceleration_config(AccelerationConfig {
    enable_cuda: true,
    enable_metal: true,
    ..baseline
});

When isolating SIMD/NEON differences, set `enable_simd = false` to force scalar
execution. The `disabling_simd_forces_scalar_and_preserves_outputs` regression
forces the scalar backend and asserts vector ops stay bit-identical to the
SIMD-enabled baseline on the same host while surfacing the `simd` status/error
fields via `acceleration_runtime_status`/`acceleration_runtime_errors`.【crates/ivm/tests/acceleration_simd.rs:9】
```

### GPU 默认值和启发式

默认情况下，`MerkleTree` GPU 卸载在 `8192` 离开时启动，CPU SHA-2
每个架构的首选项阈值保持在 `32_768` 叶。当 CUDA 和
Metal可用或已被健康检查禁用，VM自动下降
回到 SIMD/标量哈希，上面的数字不会影响确定性。

`max_gpus` 限制输入 `GpuManager` 的池大小。将 `max_gpus = 1` 设置为开启
多 GPU 主机使遥测变得简单，同时仍允许加速。运营商可以
使用此开关可为 FASTPQ 或 CUDA Poseidon 作业保留剩余设备。

### 下一步加速目标和预算

最新的FastPQ金属走线（`fastpq_metal_bench_20k_latest.json`，32K行×16
列，5 迭代）显示 Poseidon 列哈希主导 ZK 工作负载：

- `poseidon_hash_columns`：CPU 平均值 **3.64s** 与 GPU 平均值 **3.55s** (1.03×)。
- `lde`：CPU 平均值 **1.75s** 与 GPU 平均值 **1.57s** (1.12×)。

IVM/Crypto 将在下一次加速扫描中针对这两个内核。基准预算：

- 保持标量/SIMD 奇偶校验等于或低于 CPU 均值以上，并捕获
  `acceleration_runtime_status()` 伴随每次运行，因此 Metal/CUDA 可用性是
  记录预算数字。
- 一旦调谐金属，`poseidon_hash_columns` 的目标加速≥1.3×，`lde` 的目标加速≥1.2×
  和 CUDA 内核落地，无需更改输出或遥测标签。

将 JSON 跟踪和 `cargo xtask acceleration-state --format json` 快照附加到
未来的实验室运行，以便 CI/回归可以断言预算和后端运行状况，同时
比较仅 CPU 与加速运行。

### Norito 启发式（编译时默认值）Norito 的布局和压缩启发式方法存在于 `crates/norito/src/core/heuristics.rs` 中
并被编译到每个二进制文件中。它们在运行时不可配置，但会暴露
这些输入可帮助 SDK 和运营团队预测 GPU 运行后 Norito 的行为方式
压缩内核已启用。
工作区现在构建 Norito，并且默认启用 `gpu-compression` 功能，
所以 GPU zstd 后端被编译进去；运行时可用性仍然取决于硬件，
帮助程序库 (`libgpuzstd_*`/`gpuzstd_cuda.dll`) 和 `allow_gpu_compression`
配置标志。使用 `cargo build -p gpuzstd_metal --release` 构建 Metal 助手并
将 `libgpuzstd_metal.dylib` 放在加载程序路径上。当前 Metal 助手运行 GPU
匹配查找/序列生成并使用板条箱内确定性 zstd 框架
主机上的编码器（Huffman/FSE+框架组件）；解码使用板条箱内的帧
对不支持的帧使用 CPU zstd 回退的解码器，直到连接 GPU 块解码。

|领域 |默认 |目的|
|--------|---------|---------|
| `min_compress_bytes_cpu` | `256` 字节 |在此之下，有效负载完全跳过 zstd 以避免开销。 |
| `min_compress_bytes_gpu` | `1_048_576` 字节 (1MiB) |当 `norito::core::hw::has_gpu_compression()` 为 true 时，等于或高于此限制的有效负载会切换到 GPU zstd。 |
| `zstd_level_small` / `zstd_level_large` | `1` / `3` |分别针对 <32KiB 和 ≥32KiB 有效负载的 CPU 压缩级别。 |
| `zstd_level_gpu` | `1` |保守的 GPU 级别可在填充命令队列时保持延迟一致。 |
| `large_threshold` | `32_768` 字节 | “小”和“大”CPU zstd 级别之间的大小边界。 |
| `aos_ncb_small_n` | `64` 行 |在此行数下方，自适应编码器探测 AoS 和 NCB 布局以选择最小的有效负载。 |
| `combo_no_delta_small_n_if_empty` | `2` 行 |当 1-2 行包含空单元格时，防止启用 u32/id 增量编码。 |
| `combo_id_delta_min_rows` / `combo_u32_delta_min_rows` | `2` |仅当至少有两行时，增量才会生效。 |
| `combo_enable_id_delta` / `combo_enable_u32_delta_names` / `combo_enable_u32_delta_bytes` | `true` |默认情况下，对于行为良好的输入启用所有增量变换。 |
| `combo_enable_name_dict` | `true` |当命中率证明内存开销合理时，允许使用每列字典。 |
| `combo_dict_ratio_max` | `0.40` |当超过 40% 的行不同时禁用字典。 |
| `combo_dict_avg_len_min` | `8.0` |在构建字典之前要求平均字符串长度≥8（短别名保持内联）。 |
| `combo_dict_max_entries` | `1024` |字典条目的硬上限以保证有限的内存使用。 |

这些启发式方法使启用 GPU 的主机与仅使用 CPU 的主机保持一致：选择器
从不做出改变线路格式的决定，并且阈值是固定的
每个版本。当分析发现更好的收支平衡点时，Norito 会更新
规范 `Heuristics::canonical` 实现和 `docs/source/benchmarks.md` plus
`status.md` 将更改与版本证据一起记录。GPU zstd 帮助程序强制执行相同的 `min_compress_bytes_gpu` 截止，即使
直接调用（例如通过 `norito::core::gpu_zstd::encode_all`），所以很小
无论 GPU 可用性如何，有效负载始终保留在 CPU 路径上。

### 故障排除和奇偶校验检查表

- 使用 `cargo xtask acceleration-state --format json` 快照运行时状态并保留
  输出以及任何失败的日志；该报告显示已配置/可用的后端
  加上奇偶校验/最后一个错误字符串。
- 在本地重新运行加速奇偶校验回归以排除漂移：
  `cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  （仅运行 CPU，然后加速）。记录运行的 `acceleration_runtime_status()`。
- 如果后端自检失败，请以仅 CPU 模式保持节点在线（`enable_metal =
  false`, `enable_cuda = false`) 并使用捕获的奇偶校验输出打开事件
  而不是强制启动后端。结果必须在不同模式下保持确定性。
- **CUDA 奇偶校验烟雾（实验室 NV 硬件）：** 运行
  `ACCEL_ENABLE_CUDA=1 cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  在 sm_8x 硬件上，捕获 `cargo xtask acceleration-state --format json`，并附加
  基准测试工件的状态快照（包括 GPU 型号/驱动程序）。