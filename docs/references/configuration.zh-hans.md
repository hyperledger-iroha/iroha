---
lang: zh-hans
direction: ltr
source: docs/references/configuration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cff283a14bf65f185f81539f8fbcd78ddcc6447c5e9045e1b46493051febaf6a
source_last_modified: "2025-12-29T18:16:35.913045+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 加速

`[accel]` 部分控制 IVM 和助手的可选硬件加速。全部
加速路径具有确定性的 CPU 回退；如果后端失败了
运行时自检会自动禁用，并在 CPU 上继续执行。

- `enable_cuda`（默认值：true） - 编译并可用时使用 CUDA。
- `enable_metal`（默认值：true） - 在 macOS 上使用 Metal（如果可用）。
- `max_gpus`（默认值：0）——要初始化的最大 GPU 数量； `0` 表示自动/无上限。
- `merkle_min_leaves_gpu`（默认值：8192） - 卸载 Merkle 的最小叶数
  叶散列到 GPU。仅对于速度异常快的 GPU 而言较低。
- 高级（可选；通常继承合理的默认值）：
  - `merkle_min_leaves_metal`（默认：继承`merkle_min_leaves_gpu`）。
  - `merkle_min_leaves_cuda`（默认：继承`merkle_min_leaves_gpu`）。
  - `prefer_cpu_sha2_max_leaves_aarch64`（默认值：32768） – 优先选择 CPU SHA-2，直到 ARMv8 上具有 SHA2 的这么多叶子。
  - `prefer_cpu_sha2_max_leaves_x86`（默认值：32768） – 在 x86/x86_64 上优先使用 CPU SHA-NI 最多这么多叶子。

注释
- 确定性第一：加速度永远不会改变可观察的输出；后端
  在 init 上运行黄金测试，并在检测到不匹配时回退到标量/SIMD。
- 通过 `iroha_config` 配置；避免生产中的环境变量。