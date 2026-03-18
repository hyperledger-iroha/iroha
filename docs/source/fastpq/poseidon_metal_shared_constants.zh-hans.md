---
lang: zh-hans
direction: ltr
source: docs/source/fastpq/poseidon_metal_shared_constants.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4cbbc93e4212320422b8cbfcd8c563419d5ddaf5dad9e84a7878a439892ed081
source_last_modified: "2025-12-29T18:16:35.955568+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 波塞冬金属共享常数

Metal 内核、CUDA 内核、Rust 证明器和每个 SDK 固定装置必须共享
完全相同的 Poseidon2 参数以保持硬件加速
散列确定性。本文档记录了规范快照，如何
重新生成它，以及 GPU 管道如何摄取数据。

## 快照清单

这些参数以 `PoseidonSnapshot` RON 文档的形式发布。副本有
保持版本控制，因此 GPU 工具链和 SDK 不依赖于构建时
代码生成。

|路径|目的| SHA-256 |
|------|---------|---------|
| `artifacts/offline_poseidon/constants.ron` |从 `fastpq_isi::poseidon::{ROUND_CONSTANTS, MDS}` 生成的规范快照； GPU 构建的真实来源。 | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `IrohaSwift/Fixtures/offline_poseidon/constants.ron` |镜像规范快照，以便 Swift 单元测试和 XCFramework 烟雾线束加载 Metal 内核期望的相同常量。 | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `java/iroha_android/src/test/resources/offline_poseidon/constants.ron` | Android/Kotlin 设备共享相同的奇偶校验和序列化测试清单。 | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |

每个消费者在将常量连接到 GPU 之前都必须验证哈希值
管道。当清单发生更改（新的参数集或配置文件）时，SHA 和
下游镜像必须同步更新。

## 再生

该清单是通过运行 `xtask` 从 Rust 源生成的
帮手。该命令写入规范文件和 SDK 镜像：

```bash
cargo xtask offline-poseidon-fixtures --tag iroha.offline.receipt.merkle.v1
```

使用 `--constants <path>`/`--vectors <path>` 覆盖目标或
仅重新生成规范快照时的 `--no-sdk-mirror`。帮手会
当省略该标志时，将工件镜像到 Swift 和 Android 树中，
这使得 CI 的哈希值保持一致。

## 提供 Metal/CUDA 构建

- `crates/fastpq_prover/metal/kernels/poseidon2.metal` 和
  `crates/fastpq_prover/cuda/fastpq_cuda.cu` 必须从
  每当表发生变化时就会显示出来。
- 舍入和 MDS 常量被分级为连续的 `MTLBuffer`/`__constant`
  与清单布局匹配的段：`round_constants[round][state_width]`
  接下来是 3x3 MDS 矩阵。
- `fastpq_prover::poseidon_manifest()` 加载并验证快照
  运行时（Metal 预热期间），因此诊断工具可以断言
  着色器常量通过以下方式匹配发布的哈希值
  `fastpq_prover::poseidon_manifest_sha256()`。
- SDK 夹具读取器（Swift `PoseidonSnapshot`、Android `PoseidonSnapshot`）和
  Norito 离线工具依赖于相同的清单，这会阻止仅使用 GPU
  参数叉。

## 验证

1. 重新生成清单后，运行 `cargo test -p xtask` 来执行
   Poseidon 夹具生成单元测试。
2. 在本文档以及任何监控的仪表板中记录新的 SHA-256
   GPU 文物。
3. `cargo test -p fastpq_prover poseidon_manifest_consistency`解析
   `poseidon2.metal` 和 `fastpq_cuda.cu` 在构建时并断言它们
   序列化常量与清单匹配，保留 CUDA/Metal 表和
   锁步的规范快照。将清单与 GPU 构建指令放在一起可以提供 Metal/CUDA
工作流程确定性握手：内核可以自由优化其内存
布局，只要它们摄取共享常量 blob 并公开散列
用于奇偶校验的遥测。