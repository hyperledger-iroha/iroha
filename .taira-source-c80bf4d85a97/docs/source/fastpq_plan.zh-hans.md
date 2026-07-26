---
lang: zh-hans
direction: ltr
source: docs/source/fastpq_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8324267c90cfbaf718760c4883427e85d81edcfa180dd9f64fd31a5e219749f4
source_last_modified: "2026-01-17T04:50:15.304524+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# FASTPQ 证明者工作分解

本文档捕获了交付可投入生产的 FASTPQ-ISI 验证器并将其连接到数据空间调度管道的分阶段计划。下面的每个定义都是规范性的，除非标记为 TODO。估计的健全性使用开罗风格的 DEEP-FRI 界限；如果测量的界限低于 128 位，CI 中的自动拒绝采样测试就会失败。

## 阶段 0 — 哈希占位符（已落地）
- 具有 BLAKE2b 承诺的确定性 Norito 编码。
- 占位符后端返回 `BackendUnavailable`。
- `fastpq_isi` 提供的规范参数表。

## 第 1 阶段 — 跟踪生成器原型

> **状态 (2025-11-09)：** `fastpq_prover` 现在公开规范包装
> 助手 (`pack_bytes`、`PackedBytes`) 和确定性 Poseidon2
> 超过金发姑娘的订购承诺。常量固定到
> `ark-poseidon2` 提交 `3f2b7fe`，关闭关于换出临时 BLAKE2 的后续工作
> 占位符已关闭。金色灯具 (`tests/fixtures/packing_roundtrip.json`,
> `tests/fixtures/ordering_hash.json`) 现在锚定回归套件。

### 目标
- 为 KV 更新 AIR 实现 FASTPQ 跟踪生成器。每行必须编码：
  - `key_limbs[i]`：规范密钥路径的 base-256 肢体（7 字节，小端）。
  - `value_old_limbs[i]`、`value_new_limbs[i]`：前/后值的包装相同。
  - 选择器列：`s_active`、`s_transfer`、`s_mint`、`s_burn`、`s_role_grant`、`s_role_revoke`、`s_meta_set`、`s_perm`。
  - 辅助列：`delta = value_new - value_old`、`running_asset_delta`、`metadata_hash`、`supply_counter`。
  - 资产列：使用 7 字节肢体的 `asset_id_limbs[i]`。
  - 每个级别的 SMT 列 `ℓ`：`path_bit_ℓ`、`sibling_ℓ`、`node_in_ℓ`、`node_out_ℓ`，以及非会员资格的 `neighbour_leaf`。
  - 元数据列：`dsid`、`slot`。
- **确定性排序。** 使用稳定排序按 `(key_bytes, op_rank, original_index)` 字典顺序对行进行排序。 `op_rank` 映射：`transfer=0`、`mint=1`、`burn=2`、`role_grant=3`、`role_revoke=4`、`meta_set=5`。 `original_index` 是排序前从 0 开始的索引。保留生成的 Poseidon2 排序哈希（域标签 `fastpq:v1:ordering`）。将哈希原像编码为 `[domain_len, domain_limbs…, payload_len, payload_limbs…]`，其中长度为 u64 字段元素，因此尾随零字节保持可区分。
- 查找见证：当存储列 `s_perm`（`s_role_grant` 和 `s_role_revoke` 的逻辑或）为 1 时，生成 `perm_hash = Poseidon2(role_id || permission_id || epoch_u64_le)`。角色/权限 ID 是固定宽度的 32 字节 LE 字符串； epoch 是 8 字节 LE。
- 在 AIR 之前和内部强制执行不变量：选择器互斥、按资产保存、dsid/槽常量。
- `N_trace = 2^k`（行数 `pow2_ceiling`）； `N_eval = N_trace * 2^b`，其中 `b` 是放大指数。
- 提供固定装置和性能测试：
  - 包装往返（`fastpq_prover/tests/packing.rs`、`tests/fixtures/packing_roundtrip.json`）。
  - 订购稳定性哈希 (`tests/fixtures/ordering_hash.json`)。
  - 批量夹具（`trace_transfer.json`、`trace_mint.json`、`trace_duplicate_update.json`）。### AIR 列架构
|栏目组|姓名 |描述 |
| ----------------- | ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------- |
|活动 | `s_active` | 1 表示实际行，0 表示填充。                                                                                       |
|主要| `key_limbs[i]`、`value_old_limbs[i]`、`value_new_limbs[i]` |打包的 Goldilocks 元素（little-endian、7 字节肢体）。                                                             |
|资产| `asset_id_limbs[i]` |打包的规范资产标识符（小端、7 字节肢体）。                                                      |
|选择器 | `s_transfer`、`s_mint`、`s_burn`、`s_role_grant`、`s_role_revoke`、`s_meta_set`、`s_perm` | 0/1。约束：Σ选择器（包括`s_perm`）= `s_active`； `s_perm` 镜像角色授予/撤销行。              |
|辅助| `delta`、`running_asset_delta`、`metadata_hash`、`supply_counter` |用于约束、保护和审计跟踪的状态。                                                           |
|表面贴装技术| `path_bit_ℓ`、`sibling_ℓ`、`node_in_ℓ`、`node_out_ℓ`、`neighbour_leaf` |每级 Poseidon2 输入/输出以及非会员的邻居见证。                                         |
|查找 | `perm_hash` |用于权限查找的 Poseidon2 哈希（仅当 `s_perm = 1` 时受限制）。                                            |
|元数据 | `dsid`，`slot` |跨行恒定。                                                                                                 |### 数学与约束
- **字段打包：** 字节被分成 7 字节肢体（小端）。每肢 `limb_j = Σ_{k=0}^{6} byte_{7j+k} * 256^k`；拒绝肢体≥金发姑娘模数。
- **平衡/守恒：** 让 `δ = value_new - value_old`。按 `asset_id` 对行进行分组。在每个资产组的第一行定义 `r_asset_start = 1`（否则为 0）并约束
  ```
  running_asset_delta = (1 - r_asset_start) * running_asset_delta_prev + δ.
  ```
  在每个资产组的最后一行断言
  ```
  running_asset_delta = Σ (s_mint * δ) - Σ (s_burn * δ).
  ```
  转移自动满足约束，因为它们的 δ 值在整个组中总和为零。示例：如果 `value_old = 100` 和 `value_new = 120` 在铸币行上，δ = 20，因此铸币总和贡献 +20，并且当没有发生销毁时，最终检查解析为零。
- **填充：**介绍`s_active`。将所有行约束乘以 `s_active` 并强制使用连续前缀：`s_active[i] ≥ s_active[i+1]`。填充行 (`s_active=0`) 必须保持常量值，但在其他方面不受约束。
- **排序哈希：** Poseidon2 哈希（域 `fastpq:v1:ordering`）通过行编码；存储在公共 IO 中以实现可审计性。

## 第 2 阶段 — STARK 证明者核心

### 目标
- 通过跟踪和查找评估向量构建 Poseidon2 Merkle 承诺。参数：速率=2、容量=1、整轮=8、部分轮=57、固定到 `ark-poseidon2` 提交 `3f2b7fe` (v0.3.0) 的常量。
- 低度扩展：评估域 `D = { g^i | i = 0 .. N_eval-1 }` 上的每一列，其中 `N_eval = 2^{k+b}` 除以 Goldilocks 的 2-adic 容量。设 `g = ω^{(p-1)/N_eval}` 与 `ω` 为金发姑娘的固定原根，`p` 为其模；使用基本子群（无陪集）。在转录本中记录 `g`（标签 `fastpq:v1:lde`）。
- 复合多项式：对于每个约束 `C_j`，形成 `F_j(X) = C_j(X) / Z_N(X)`，其度数边界如下。
- 查找参数（权限）：来自转录本的示例 `γ`。跟踪产品 `Z_0 = 1`、`Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}`。表产品 `T = ∏_j (table_perm_j - γ)`。边界约束：`Z_final / T = 1`。
- 数量为 `r ∈ {8, 16}` 的 DEEP-FRI：对于每一层，吸收带有标签 `fastpq:v1:fri_layer_ℓ` 的根部、样品 `β_ℓ`（标签 `fastpq:v1:beta_ℓ`），并通过 `H_{ℓ+1}(i) = Σ_{k=0}^{r-1} H_ℓ(r*i + k) * β_ℓ^k` 折叠。
- 证明对象（Norito 编码）：
  ```
  Proof {
      protocol_version: u16,
      params_version: u16,
      parameter_set: String,
      public_io: PublicIO,
      trace_root: [u8; 32],
      lookup_root: [u8; 32],
      fri_layers: Vec<[u8; 32]>,
      alphas: Vec<Field>,
      betas: Vec<Field>,
      queries: Vec<QueryOpening>,
  }
  ```
- 验证者镜像证明者；使用黄金转录本在 1k/5k/20k 行跟踪上运行回归套件。

###学位会计
|约束|划分前的度数 |选择器后的度数 |保证金 vs `deg(Z_N)` |
|------------------------|------------------------------------|------------------------|------------------------|
|转移/铸造/烧毁保存 | ≤1 | ≤1 | `deg(Z_N) - 2` |
|角色授予/撤销查找 | ≤2 | ≤2 | `deg(Z_N) - 3` |
|元数据集 | ≤1 | ≤1 | `deg(Z_N) - 2` |
| SMT 哈希（每级）| ≤3 | ≤3 | `deg(Z_N) - 4` |
|查找盛大产品|产品关系 |不适用 |边界约束|
|边界根/供应总量| 0 | 0 |准确|

填充行通过 `s_active` 处理；虚拟行将跟踪扩展到 `N_trace`，而不违反约束。## 编码和转录（全局）
- **字节打包：** base-256（7 字节肢体，小端）。在 `fastpq_prover/tests/packing.rs` 中进行测试。
- **字段编码：** 规范的 Goldilocks（little-endian 64 位分支，拒绝 ≥ p）； Poseidon2 输出/SMT 根序列化为 32 字节小端数组。
- **成绩单（菲亚特-沙米尔）：**
  1. BLAKE2b 吸收 `protocol_version`、`params_version`、`parameter_set`、`public_io` 和 Poseidon2 提交标签 (`fastpq:v1:init`)。
  2. 吸收`trace_root`、`lookup_root`（`fastpq:v1:roots`）。
  3. 导出查找挑战 `γ` (`fastpq:v1:gamma`)。
  4. 推导组合挑战 `α_j` (`fastpq:v1:alpha_j`)。
  5. 对于每个FRI层根，用`fastpq:v1:fri_layer_ℓ`吸收，导出`β_ℓ`（`fastpq:v1:beta_ℓ`）。
  6. 派生查询索引 (`fastpq:v1:query_index`)。

  标签为小写 ASCII；验证者在抽样挑战之前拒绝不匹配。金色成绩单夹具：`tests/fixtures/transcript_v1.json`。
- **版本控制：** `protocol_version = 1`、`params_version` 与 `fastpq_isi` 参数集匹配。

## 查找参数（权限）
- 提交的表按 `(role_id_bytes, permission_id_bytes, epoch_le)` 字典顺序排序，并通过 Poseidon2 Merkle 树提交（`PublicIO` 中的 `perm_root`）。
- 跟踪见证使用 `perm_hash` 和选择器 `s_perm`（角色授予/撤销的 OR）。该元组被编码为具有固定宽度（32、32、8 字节）的 `role_id_bytes || permission_id_bytes || epoch_u64_le`。
- 产品关系：
  ```
  Z_0 = 1
  for each row i: Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}
  T = ∏_j (table_perm_j - γ)
  ```
  边界断言：`Z_final / T = 1`。有关具体累加器演练，请参阅 `examples/lookup_grand_product.md`。

## 稀疏默克尔树约束
- 定义 `SMT_HEIGHT`（级别数）。对于所有 `ℓ ∈ [0, SMT_HEIGHT)`，都会显示列 `path_bit_ℓ`、`sibling_ℓ`、`node_in_ℓ`、`node_out_ℓ`、`neighbour_leaf`。
- Poseidon2 参数固定到 `ark-poseidon2` 提交 `3f2b7fe` (v0.3.0)；域标签 `fastpq:v1:poseidon_node`。所有节点都使用小端字段编码。
- 更新每个级别的规则：
  ```
  if path_bit_ℓ == 0:
      node_out_ℓ = Poseidon2(node_in_ℓ, sibling_ℓ)
  else:
      node_out_ℓ = Poseidon2(sibling_ℓ, node_in_ℓ)
  ```
- 刀片套件 `(node_in_0 = 0, node_out_0 = value_new)`；删除集 `(node_in_0 = value_old, node_out_0 = 0)`。
- 非会员证明提供`neighbour_leaf`以显示查询的区间为空。有关工作示例和 JSON 布局，请参阅 `examples/smt_update.md`。
- 边界约束：对于前行，最终散列等于 `old_root`；对于后行，最终散列等于 `new_root`。

## 健全性参数和 SLO
| N_trace |爆炸| FRI 数量 |层 |查询 | est位|打样尺寸 (≤) |内存 (≤) | P95 潜伏期 (≤) |
| -------- | ------ | ---------| ------ | -------- | -------- | ---------------- | -------- | ---------------- |
| 2^15 | 2^15 8 | 8 | 5 | 52 | 52 〜190 | 300 KB | 1.5 GB | 1.5 GB 0.40 秒 (A100) |
| 2^16 | 2^16 8 | 8 | 6 | 58 | 58 〜132 | 420 KB | 2.5 GB | 2.5 GB 0.75 秒 (A100) |
| 2^17 | 2^17 16 | 16 16 | 16 5 | 64 | 64 〜142 | 550 KB | 3.5 GB | 3.5 GB 1.20 秒 (A100) |

推导遵循附录 A。如果估计位 <128，CI 工具会生成格式错误的证明并失败。## 公共 IO 架构
|领域 |字节 |编码 |笔记|
|----------------|--------------------|----------------------------------------------------|----------------------------------------|
| `dsid` | 16 | 16小端 UUID |条目通道的数据空间 ID（默认通道的全局），使用标签 `fastpq:v1:dsid` 进行哈希处理。 |
| `slot` | 8 |小尾数 u64 |自纪元以来的纳秒。            |
| `old_root` | 32 | 32小端 Poseidon2 字段字节 |批量前SMT根。              |
| `new_root` | 32 | 32小端 Poseidon2 字段字节 | SMT批量后根。               |
| `perm_root` | 32 | 32小端 Poseidon2 字段字节 |插槽的权限表根。 |
| `tx_set_hash` | 32 | 32布莱克2b |排序的指令标识符。     |
| `parameter` |变量 | UTF-8（例如 `fastpq-lane-balanced`）|参数集名称。                 |
| `protocol_version`，`params_version` |各 2 个 |小尾数 u16 |版本值。                      |
| `ordering_hash` | 32 | 32 Poseidon2（小端）|排序行的稳定哈希。         |

删除由零值肢体编码；缺席的键使用零叶+邻居见证。

`FastpqTransitionBatch.public_inputs` 是 `dsid`、`slot` 和根承诺的规范载体；
批次元数据保留用于条目哈希/转录本计数簿记。

## 哈希编码
- 排序哈希：Poseidon2（标签 `fastpq:v1:ordering`）。
- 批量工件哈希：BLAKE2b over `PublicIO || proof.commitments`（标签 `fastpq:v1:artifact`）。

## 完成 (DoD) 的阶段定义
- **第一阶段国防部**
  - 包装往返测试和固定装置合并。
  - AIR 规范 (`docs/source/fastpq_air.md`) 包括 `s_active`、资产/SMT 列、选择器定义（包括 `s_perm`）和符号约束。
  - 排序哈希记录在 PublicIO 中并通过固定装置进行验证。
  - 使用会员和非会员向量实现 SMT/查找见证生成。
  - 保存测试涵盖转移、薄荷、燃烧和混合批次。
- **第 2 阶段国防部**
  - 实施转录规范；黄金成绩单 (`tests/fixtures/transcript_v1.json`) 和域名标签已验证。
  - Poseidon2 参数提交 `3f2b7fe` 固定在证明者和验证者中，并进行跨架构的字节序测试。
  - 健全 CI 防护激活；记录校样大小/RAM/延迟 SLO。
- **第 3 阶段国防部**
  - 使用幂等密钥记录的调度程序 API（`SubmitProofRequest`、`ProofResult`）。
  - 通过重试/退避以可寻址方式存储内容的证明工件。
  - 导出队列深度、队列等待时间、证明执行延迟、重试计数、后端故障计数和 GPU/CPU 利用率的遥测，以及每个指标的仪表板和警报阈值。## 第 5 阶段 — GPU 加速和优化
- 目标内核：LDE (NTT)、Poseidon2 哈希、Merkle 树构建、FRI 折叠。
- 确定性：禁用快速数学，确保跨 CPU、CUDA、Metal 的位相同输出。 CI 必须跨设备比较证明根。
- 基准套件比较参考硬件（例如 Nvidia A100、AMD MI210）上的 CPU 与 GPU。
- 金属后端（Apple Silicon）：
  - 构建脚本通过 `xcrun metal`/`xcrun metallib` 将内核套件（`metal/kernels/ntt_stage.metal`、`metal/kernels/poseidon2.metal`）编译为 `fastpq.metallib`；确保 macOS 开发者工具包含 Metal 工具链（`xcode-select --install`，如果需要，则为 `xcodebuild -downloadComponent MetalToolchain`）。【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:189】
  - 用于 CI 预热或确定性打包的手动重建（镜像 `build.rs`）：
    ```bash
    export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
    xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
    export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
    ```
    成功的构建会发出 `FASTPQ_METAL_LIB=<path>`，因此运行时可以确定性地加载 Metallib。【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:42】
  - LDE 内核现在假定主机上的评估缓冲区已初始化为零。保留现有的 `vec![0; ..]` 分配路径或在重用时显式将缓冲区清零。【crates/fastpq_prover/src/metal.rs:233】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:141】
  - 陪集乘法被融合到最后的FFT阶段以避免额外的通过；对 LDE 分段的任何更改都必须保留该不变性。【crates/fastpq_prover/metal/kernels/ntt_stage.metal:193】
  - 共享内存 FFT/LDE 内核现在停止在图块深度处，并将剩余的蝴蝶和任何逆缩放交给专用的 `fastpq_fft_post_tiling` 通道。 Rust 主机通过两个内核对相同的列批次进行线程处理，并且仅在 `log_len` 超出切片限制时才启动切片后分派，因此队列深度遥测、内核统计数据和回退行为保持确定性，而 GPU 完全处理宽阶段工作设备上。【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:654】
  - 要试验发射形状，请设置 `FASTPQ_METAL_THREADGROUP=<width>`；调度路径将值限制为设备限制并记录覆盖，因此分析运行可以扫描线程组大小而无需重新编译。【crates/fastpq_prover/src/metal.rs:321】- 直接调整 FFT 图块：主机现在派生线程组通道（短迹线为 16 个，`log_len ≥ 6` 为 32 个，`log_len ≥ 10` 为 64 个，`log_len ≥ 14` 为 128 个，`log_len ≥ 18` 为 256 个）和图块深度（小迹线为 5 个阶段，小迹线为 4 个阶段） `log_len ≥ 12`，一旦域达到 `log_len ≥ 18/20/22`，共享内存通道现在会从请求的域加上设备的执行宽度/最大线程运行 12/14/16 个阶段，然后将控制权交给后切片内核）。使用 `FASTPQ_METAL_FFT_LANES`（8 和 256 之间的 2 的幂）和 `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) 覆盖以固定特定的发射形状；两个值都流过 `FftArgs`，被固定到支持的窗口，并被记录以进行分析扫描。【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:120】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:244】
- FFT/IFFT 和 LDE 列批处理现在源自已解析的线程组宽度：主机的目标是每个命令缓冲区大约 4096 个逻辑线程，通过循环缓冲区图块暂存一次最多融合 64 列，并且当评估域跨越 216/218/220/222 阈值时，仅通过 64→32→16→8→4→2→1 列逐渐下降。这使得每次调度 20k 行捕获≥64 列，同时确保长陪集仍然确定地完成。自适应调度程序仍然使列宽加倍，直到调度接近 ≈2ms 目标，并且现在只要采样调度超过该目标 ≥30%，就会自动将批次减半，因此导致每列成本增加的通道/图块转换会回落，无需手动覆盖。 Poseidon 排列共享相同的自适应调度程序，并且 `fastpq_metal_bench` 中的 `metal_heuristics.batch_columns.poseidon` 块现在记录已解析的状态计数、上限、最后持续时间和覆盖标志，因此队列深度遥测可以直接与 Poseidon 调整相关联。使用 `FASTPQ_METAL_FFT_COLUMNS` (1–64) 覆盖以固定确定性 FFT 批量大小，并在需要 LDE 调度程序遵守固定列数时使用 `FASTPQ_METAL_LDE_COLUMNS` (1–64)； Metal bench 在每次捕获中都会显示已解析的 `kernel_profiles.*.columns` 条目，因此调整实验保持可重复性。【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/metal.rs:1402】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1284】- 多队列调度现在在独立 Mac 上是自动的：主机检查 `is_low_power`、`is_headless` 和设备位置，以决定是否启动两个 Metal 命令队列，仅在工作负载至少承载 16 列（由已解析的扇出扩展）时扇出，并对列批次进行循环，以便长跟踪使两个 GPU 通道保持繁忙，而不牺牲确定性。命令缓冲区信号量现在强制执行“每个队列两个飞行”楼层，队列遥测记录全局信号量和每个队列条目的聚合测量窗口 (`window_ms`) 以及标准化繁忙比率 (`busy_ratio`)，因此发布工件可以证明两个队列在同一时间跨度内保持 ≥50% 的繁忙状态。使用 `FASTPQ_METAL_QUEUE_FANOUT`（1–4 通道）和 `FASTPQ_METAL_COLUMN_THRESHOLD`（扇出前的最小总列数）覆盖默认值； Metal 奇偶校验测试强制覆盖，以便多 GPU Mac 保持覆盖，并且解决的策略与队列深度遥测和新的 `metal_dispatch_queue.queues[*]` 一起记录区块.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:871】
- 金属检测现在直接探测 `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices`（在无头 shell 上预热 CoreGraphics），然后回退到 `system_profiler`，并且 `FASTPQ_DEBUG_METAL_ENUM` 在设置时打印枚举设备，因此无头 CI 运行可以解释为什么 `FASTPQ_GPU=gpu` 仍然降级到 CPU路径。当覆盖设置为 `gpu` 但未检测到加速器时，`fastpq_metal_bench` 现在会立即出现错误，指针指向调试旋钮，而不是在 CPU 上默默地继续。这缩小了 WP2-E 中调用的“静默 CPU 回退”类别，并为操作员提供了一个旋钮来捕获包装基准内的枚举日志。【crates/fastpq_prover/src/backend.rs:665】【crates/fastpq_prover/src/backend.rs:705】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】
  - Poseidon GPU 计时现在拒绝将 CPU 回退视为“GPU”数据。 `hash_columns_gpu` 报告加速器是否实际运行，`measure_poseidon_gpu` 在管道回退时丢弃样本（并记录警告），如果 GPU 哈希不可用，则 Poseidon microbench 子级会退出并显示错误。因此，每当 Metal 执行回退时，`gpu_recorded=false` 队列摘要仍会记录失败的调度窗口，并且仪表板摘要会立即标记回退。现在，当 `metal_dispatch_queue.poseidon.dispatch_count == 0` 时，包装器 (`scripts/fastpq/wrap_benchmark.py`) 会失败，因此如果没有真正的 GPU Poseidon 调度，则无法对 Stage7 捆绑包进行签名证据。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1123】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2200】【scripts/fastpq/wrap_benchmark.py:912】- 波塞冬哈希现在反映了该分期合同。 `PoseidonColumnBatch` 生成扁平化的有效负载缓冲区以及偏移/长度描述符，主机每批重新设置这些描述符的基础并运行 `COLUMN_STAGING_PIPE_DEPTH` 双缓冲区，因此有效负载 + 描述符上传与 GPU 工作重叠，并且两个 Metal/CUDA 内核都直接消耗描述符，因此每次调度在发出列摘要之前都会吸收设备上的所有填充速率块。 `hash_columns_from_coefficients` 现在通过 GPU 工作线程流式传输这些批次，默认情况下在离散 GPU 上保持 64 个以上的列处于运行状态（可通过 `FASTPQ_POSEIDON_PIPE_COLUMNS` / `FASTPQ_POSEIDON_PIPE_DEPTH` 进行调整）。 Metal 工作台记录了 `metal_dispatch_queue.poseidon_pipeline` 下解析的管道设置 + 批次计数，`kernel_profiles.poseidon.bytes` 包括描述符流量，因此 Stage7 捕获证明了新的 ABI端到端。【crates/fastpq_prover/src/trace.rs:604】【crates/fastpq_prover/src/trace.rs:809】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:196 3】【板条箱/fastpq_prover/src/bin/fastpq_metal_bench.rs:2675】【板条箱/fastpq_prover/src/metal.rs:2290】【板条箱/fastpq_prover/cuda/fastpq_cuda.cu:351】
- Stage7-P2 融合 Poseidon 哈希现在登陆两个 GPU 后端。流工作器将连续的 `PoseidonColumnBatch::column_window()` 切片输入 `hash_columns_gpu_fused`，后者通过管道将它们传送到 `poseidon_hash_columns_fused`，因此每次调度都会使用规范的 `(⌈columns / 2⌉)` 父映射写入 `leaf_digests || parent_digests`。 `ColumnDigests` 保留两个切片，`merkle_root_with_first_level` 立即使用父层，因此 CPU 永远不会重新计算深度 1 节点，并且 Stage7 遥测可以断言只要融合内核，GPU 捕获报告零“回退”父层成功。【crates/fastpq_prover/src/trace.rs:1070】【crates/fastpq_prover/src/gpu.rs:365】【crates/fastpq_prover/src/metal.rs:2422】【crates/fastpq_prover/cuda/fastpq_cuda.cu:631】
- `fastpq_metal_bench` 现在发出一个 `device_profile` 块，其中包含 Metal 设备名称、注册表 ID、`low_power`/`headless` 标志、位置（内置、插槽、外部）、离散指示器、`hw.model` 和派生的 Apple SoC 标签（例如，“M3”）麦克斯”）。 Stage7 仪表板使用此字段来通过 M4/M3 与离散 GPU 进行捕获，而无需解析主机名，并且 JSON 发送到队列/启发式证据旁边，因此每个发布工件都证明是哪个舰队类产生了运行。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2536】- FFT 主机/设备重叠现在使用双缓冲暂存窗口：当批次 *n* 在 `fastpq_fft_post_tiling` 内完成时，主机将批次 *n+1* 压平到第二个暂存缓冲区中，并且仅在必须回收缓冲区时才暂停。后端记录扁平化的批次数量以及扁平化与等待 GPU 完成所花费的时间，并且 `fastpq_metal_bench` 显示聚合的 `column_staging.{batches,flatten_ms,wait_ms,wait_ratio}` 块，因此发布工件可以证明重叠而不是无声的主机停顿。 JSON 报告现在还在 `column_staging.phases.{fft,lde,poseidon}` 下细分了每个阶段的总数，让 Stage7 捕获证明 FFT/LDE/Poseidon 分段是主机绑定的还是等待 GPU 完成。 Poseidon 排列重复使用相同的池暂存缓冲区，因此 `--operation poseidon_hash_columns` 捕获现在会发出 Poseidon 特定的 `column_staging` 增量以及队列深度证据，而无需定制仪器。新的 `column_staging.samples.{fft,lde,poseidon}` 数组记录每批次的 `batch/flatten_ms/wait_ms/wait_ratio` 元组，因此可以轻松证明 `COLUMN_STAGING_PIPE_DEPTH` 重叠保持不变（或发现主机何时开始等待 GPU完成）。【板条箱/fastpq_prover/src/metal.rs:319】【板条箱/fastpq_prover/src/metal.rs:330】【板条箱/fastpq_prover/src/metal.rs:1813】【板条箱/fas tpq_prover/src/metal.rs:2488】【板条箱/fastpq_prover/src/bin/fastpq_metal_bench.rs:1189】【板条箱/fastpq_prover/src/bin/fastpq_metal_bench.rs:1216】- Poseidon2 加速现在作为高占用金属内核运行：每个线程组将轮常量和 MDS 行复制到线程组内存中，展开完整/部分轮，并在每个通道上行走多个状态，因此每次调度至少启动 4096 个逻辑线程。通过 `FASTPQ_METAL_POSEIDON_LANES`（32 和 256 之间的 2 的幂，限制到器件限制）和 `FASTPQ_METAL_POSEIDON_BATCH`（每通道 1-32 个状态）覆盖启动形状，以重现分析实验，而无需重建 `fastpq.metallib`； Rust 主机在分派之前通过 `PoseidonArgs` 线程解决调整。现在，主机每次启动都会对 `MTLDevice::{is_low_power,is_headless,location}` 进行一次快照，并自动将离散 GPU 偏向于 VRAM 分层启动（≥48GiB 部件上为 `256×24`，32GiB 上为 `256×20`，否则为 `256×16`），而低功耗 SoC 则坚持使用 `256×8` （128/64 通道硬件的后备继续使用每通道 8/6 个状态），因此操作员无需触及环境变量即可获得 >16 状态的管道深度。 `fastpq_metal_bench` 在 `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` 下重新执行自身，以捕获专用的 `poseidon_microbench` 块，将标量通道与多状态内核进行比较，以便发布工件可以引用具体的加速。同样捕获表面 `poseidon_pipeline` 遥测（`chunk_columns`、`pipe_depth`、`batches`、`fallbacks`），因此 Stage7 证据证明了每个 GPU 上的重叠窗口跟踪.【板条箱/fastpq_prover/metal/kernels/poseidon2.metal:1】【板条箱/fastpq_prover/src/metal_config.rs:78】【板条箱/fastpq_prover/src/metal.rs:1971】【cra tes/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【板条箱/fastpq_prover/src/trace.rs:299】【板条箱/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】
  - LDE 切片分段现在反映了 FFT 启发式：一旦 `log₂(len) ≥ 18`，大量跟踪仅执行共享内存传递中的 12 个阶段，在 log220 处下降到 10 个阶段，并在 log222 处钳位到 8 个阶段，以便宽蝴蝶移动到后切片内核中。每当您需要确定性深度时，请使用 `FASTPQ_METAL_LDE_TILE_STAGES` (1–32) 覆盖；主机仅在启发式提前停止时启动后平铺调度，因此队列深度和内核遥测保持确定性。【crates/fastpq_prover/src/metal.rs:827】
  - 内核微优化：共享内存 FFT/LDE 块现在重用每通道旋转和陪集步幅，而不是为每个蝴蝶重新评估 `pow_mod*`。每个通道预先计算 `w_seed`、`w_stride` 和（需要时）每个块的陪集步长，然后流过偏移量，大幅削减 `apply_stage_tile`/`apply_stage_global` 内部的标量乘法，并将最新的 20k 行 LDE 平均值降低至约 1.55 秒启发式（仍然高于 950 毫秒的目标，但比仅批处理调整进一步提高了约 50 毫秒）。【crates/fastpq_prover/metal/kernels/ntt_stage.metal:164】【fastpq_metal_bench_run11.json:1】- 内核套件现在有一个专门的参考资料（`docs/source/fastpq_metal_kernels.md`），它记录了每个入口点、`fastpq.metallib`中强制执行的线程组/图块限制，以及手动编译metallib的复制步骤。【docs/source/fastpq_metal_kernels.md:1】
  - 基准报告现在发出一个 `post_tile_dispatches` 对象，该对象记录在专用后平铺内核中运行了多少个 FFT/IFFT/LDE 批次（每种调度计数加上阶段/log2 边界）。 `scripts/fastpq/wrap_benchmark.py` 将该块复制到 `benchmarks.post_tile_dispatches`/`benchmarks.post_tile_summary` 中，并且清单门拒绝忽略证据的 GPU 捕获，因此每个 20k 行工件都证明多通道内核运行设备上。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:255】【xtask/src/fastpq.rs:280】
  - 设置 `FASTPQ_METAL_TRACE=1` 以发出每次调度调试日志（管道标签、线程组宽度、启动组、经过时间）以进行仪器/金属跟踪关联。【crates/fastpq_prover/src/metal.rs:346】
- 调度队列现已检测：`FASTPQ_METAL_MAX_IN_FLIGHT` 限制并发 Metal 命令缓冲区（自动默认值通过 `system_profiler` 检测到的 GPU 核心计数派生，当 macOS 拒绝报告设备时，至少夹紧到队列扇出层，并使用主机并行回退）。该工作台支持队列深度采样，因此导出的 JSON 带有 `metal_dispatch_queue` 对象，其中包含 `limit`、`dispatch_count`、`max_in_flight`、`busy_ms` 和 `overlap_ms` 字段作为发布证据，添加了嵌套每当仅 Poseidon 捕获 (`--operation poseidon_hash_columns`) 运行时，`metal_dispatch_queue.poseidon` 块就会发出 `metal_heuristics` 块，描述已解析的命令缓冲区限制以及 FFT/LDE 批处理列（包括是否覆盖强制值），以便审阅者可以在遥测的同时审核调度决策。 Poseidon 内核还提供从内核样本中提取的专用 `poseidon_profiles` 块，以便跨工件跟踪字节/线程、占用和调度几何结构。如果主运行无法收集队列深度或 LDE 零填充统计信息（例如，当 GPU 调度默默地回退到 CPU 时），该工具会自动触发单个探针调度来收集丢失的遥测数据，并且现在在 GPU 拒绝报告它们时合成主机零填充计时，因此已发布的证据始终包括 `zero_fill`区块.【crates/fastpq_prover/src/metal.rs:2056】【crates/fastpq_prover/src/metal.rs:247】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1524】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2078】
  - 在没有 Metal 工具链的情况下交叉编译时设置 `FASTPQ_SKIP_GPU_BUILD=1`；警告记录了跳过，规划器继续在CPU路径上运行。【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】- 运行时检测使用 `system_profiler` 来确认 Metal 支持；如果框架、设备或 Metallib 丢失，构建脚本将清除 `FASTPQ_METAL_LIB` 并且规划器保留在确定性 CPU 上路径.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】【crates/fastpq_prover/src/metal.rs:43】
  - 操作员清单（金属主机）：
    1.确认工具链存在，并且`FASTPQ_METAL_LIB`指向已编译的`.metallib`（`echo $FASTPQ_METAL_LIB`在`cargo build --features fastpq-gpu`之后应该非空）。【crates/fastpq_prover/build.rs:188】
    2. 在启用 GPU 通道的情况下运行奇偶校验测试：`FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release`。这会锻炼 Metal 内核，并在检测失败时自动回退。【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
    3. 捕获仪表板的基准示例：找到已编译的 Metal 库
       (`fd -g 'fastpq.metallib' target/release/build | head -n1`)，通过导出
       `FASTPQ_METAL_LIB`，然后运行
      `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`。
       规范的 `fastpq-lane-balanced` 集现在将每次捕获填充到 32,768 行，因此
       JSON 反映了请求的 20k 行和驱动 GPU 的填充域
       内核。将 JSON/日志上传到您的证据存储；每晚 macOS 工作流程镜像
      这次运行并将文物存档以供参考。报告记录
     `fft_tuning.{threadgroup_lanes,tile_stage_limit}` 与每个操作的 `speedup` 一起，
     LDE 部分添加了 `zero_fill.{bytes,ms,queue_delta}`，因此发布的工件证明了确定性，
     主机零填充开销，以及增量 GPU 队列使用情况（限制、调度计数、
     高峰飞行时间、繁忙/重叠时间），以及新的 `kernel_profiles` 块捕获每个内核
     占用率、估计带宽和持续时间范围，以便仪表板可以标记 GPU
       无需重新处理原始样本即可进行回归。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
       预计 Metal LDE 路径保持在 950ms 以下（Apple M 系列硬件上的 `<1 s` 目标）；
4. 从真实的 ExecWitness 捕获行使用遥测数据，以便仪表板可以绘制传输小工具的图表
   收养。从 Torii 获取证人
  (`iroha_cli audit witness --binary --out exec.witness`) 并解码它
  `iroha_cli audit witness --decode exec.witness`（可选添加
  `--fastpq-parameter fastpq-lane-balanced` 断言预期参数集； FASTPQ批次
  默认发出；仅当您需要调整输出时才通过 `--no-fastpq-batches`）。
   现在，每个批次条目都会发出一个 `row_usage` 对象（`total_rows`、`transfer_rows`、
   `non_transfer_rows`、每个选择器计数和 `transfer_ratio`）。存档该 JSON 片段
   重新处理原始转录本。【crates/iroha_cli/src/audit.rs:209】将新捕获的内容与
   之前的基线为 `scripts/fastpq/check_row_usage.py`，因此如果传输比率或
   总行回归：

   ```bash
   python3 scripts/fastpq/check_row_usage.py \
     --baseline artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json \
     --candidate fastpq_row_usage_2025-05-12.json \
     --max-transfer-ratio-increase 0.005 \
     --max-total-rows-increase 0
   ```用于冒烟测试的 JSON blob 示例位于 `scripts/fastpq/examples/` 中。您可以在本地运行 `make check-fastpq-row-usage`
   （包装 `ci/check_fastpq_row_usage.sh`），CI 通过 `.github/workflows/fastpq-row-usage.yml` 运行相同的脚本来比较提交的
   `artifacts/fastpq_benchmarks/fastpq_row_usage_*.json` 快照，因此证据包无论何时都会快速失败
   转移行慢慢回升。如果您想要机器可读的差异，请传递 `--summary-out <path>`（CI 作业上传 `fastpq_row_usage_summary.json`）。
   当 ExecWitness 不方便时，可以使用 `fastpq_row_bench` 合成回归样本
   (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`)，它发出完全相同的 `row_usage`
   可配置选择器计数的对象（例如 65536 行压力测试）：

   ```bash
   cargo run -p fastpq_prover --bin fastpq_row_bench -- \
     --transfer-rows 65536 \
     --mint-rows 256 \
     --burn-rows 128 \
     --pretty \
     --output artifacts/fastpq_benchmarks/fastpq_row_usage_65k.json
   ```Stage7-3 推出捆绑包还必须通过 `scripts/fastpq/validate_row_usage_snapshot.py`，这
   强制每个 `row_usage` 条目包含选择器计数并且
   `transfer_ratio = transfer_rows / total_rows`； `ci/check_fastpq_rollout.sh` 呼叫助手
   自动，因此缺少这些不变量的包会在强制执行 GPU 通道之前失败。【scripts/fastpq/validate_row_usage_snapshot.py:1】【ci/check_fastpq_rollout.sh:1】
       工作台清单门通过 `--max-operation-ms lde=950` 强制执行此操作，因此请刷新
       每当您的证据超过该界限时就进行捕获。
      当您还需要仪器证据时，请传递 `--trace-dir <dir>`，以便线束
      通过 `xcrun xctrace record`（默认“金属系统跟踪”模板）重新启动自身并
      将带时间戳的 `.trace` 文件与 JSON 一起存储；您仍然可以覆盖位置/
      使用 `--trace-output <path>` 以及可选的 `--trace-template` 手动模板 /
      `--trace-seconds`。生成的 JSON 通告 `metal_trace_{template,seconds,output}`，因此
      人工制品包总是识别捕获的痕迹。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】
      将每个捕获包裹起来
      `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output`
       （如果需要固定签名身份，请添加 `--gpg-key <fingerprint>`），因此捆绑失败
       每当 GPU LDE 平均值突破 950 毫秒目标、Poseidon 超过 1 秒或
       Poseidon 遥测块丢失，嵌入 `row_usage_snapshot`
      在 JSON 旁边，在 `benchmarks.poseidon_microbench` 下显示 Poseidon 微基准摘要，
      并且仍然携带运行手册和 Grafana 仪表板的元数据
    （`dashboards/grafana/fastpq_acceleration.json`）。 JSON 现在发出 `speedup.ratio` /
     每次操作 `speedup.delta_ms` 因此发布证据可以证明 GPU 与
     CPU 增益无需重新处理原始样本，并且包装器会复制
     零填充统计信息（加上 `queue_delta`）到 `zero_fill_hotspots`（字节、延迟、派生
     GB/s)，在 `metadata.metal_trace` 下记录仪器元数据，线程可选
     当提供 `--row-usage <decoded witness>` 时，`metadata.row_usage_snapshot` 块，并将
     每个内核计数器进入 `benchmarks.kernel_summary` 因此填充瓶颈，金属队列
     利用率、内核占用率和带宽回归一目了然，无需
     深入研究原始报告。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:521】【scripts/fastpq/wrap_benchmark.py:1】【artifacts/fastpq_benchmarks/fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】
     由于行使用快照现在与打包的工件一起传输，因此部署票证只需
     引用该包而不是附加第二个 JSON 片段，并且 CI 可以区分嵌入的
    验证 Stage7 提交时直接计数。要单独存档微基准数据，
    运行 `python3 scripts/fastpq/export_poseidon_microbench.py --bundle artifacts/fastpq_benchmarks/<metal>.json`
    并将生成的文件存储在 `benchmarks/poseidon/` 下。保持聚合清单最新
    `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`
    因此仪表板/CI 可以比较完整的历史记录，而无需手动遍历每个文件。4. 通过curl `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`（Prometheus端点）或查找`telemetry::fastpq.execution_mode`日志来验证遥测；意外的 `resolved="cpu"` 条目表明尽管 GPU 意图，主机还是回退了。【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    5. 使用 `FASTPQ_GPU=cpu`（或配置旋钮）在维护期间强制 CPU 执行，并确认回退日志仍然出现；这使 SRE Runbook 与确定性路径保持一致。【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
- 遥测和后备：
  - 执行模式日志（`telemetry::fastpq.execution_mode`）和计数器（`fastpq_execution_mode_total{device_class="…", backend="metal"|…}`）公开了请求模式与已解决模式，因此仪表板中可以看到静默回退。【crates/fastpq_prover/src/backend.rs:174】【crates/iroha_telemetry/src/metrics.rs:5397】
  - `FASTPQ Acceleration Overview` Grafana 板 (`dashboards/grafana/fastpq_acceleration.json`) 可视化 Metal 采用率并链接回基准工件，而配对警报规则 (`dashboards/alerts/fastpq_acceleration_rules.yml`) 在持续降级时推出。
  - `FASTPQ_GPU={auto,cpu,gpu}` 覆盖仍然受支持；未知值会引发警告，但仍会传播到遥测进行审核。【crates/fastpq_prover/src/backend.rs:308】【crates/fastpq_prover/src/backend.rs:349】
  - CUDA 和 Metal 必须通过 GPU 奇偶校验测试 (`cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu`)；当metallib不存在或检测失败时，CI会优雅地跳过。【crates/fastpq_prover/src/gpu.rs:49】【crates/fastpq_prover/src/backend.rs:346】
  - 金属就绪证据（每次推出时存档以下工件，以便路线图审核可以证明确定性、遥测覆盖范围和后备行为）：|步骤|目标|命令/证据|
    | ---- | ---- | ------------------ |
    |构建 Metallib |确保 `xcrun metal`/`xcrun metallib` 可用，并为此提交发出确定性 `.metallib` | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"`； `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`； `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"`；导出 `FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】
    |验证环境变量 |通过检查构建脚本记录的环境变量来确认 Metal 保持启用状态 | `echo $FASTPQ_METAL_LIB`（必须返回绝对路径；空表示后端被禁用）。【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
    | GPU 奇偶校验套件 |在发货前证明内核执行（或发出确定性降级日志）| `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` 并存储显示 `backend="metal"` 或后备警告的结果日志片段。【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】
    |基准样本|捕获记录 `speedup.*` 和 FFT 调整的 JSON/日志对，以便仪表板可以获取加速器证据 | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`；将 JSON、带时间戳的 `.trace` 和标准输出与发行说明一起存档，以便 Grafana 板选择 Metal 运行（报告记录请求的 20k 行加上填充的 32,768 行域，以便审阅者可以确认 `<1 s` LDE目标）。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
    |包装并签署报告 |如果 GPU LDE 平均时间超过 950 毫秒、Poseidon 超过 1 秒或 Poseidon 遥测块丢失，则发布失败，并生成签名的工件包 | `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`；发送打包的 JSON 和生成的 `.json.asc` 签名，以便审核员可以验证亚秒级指标，而无需重新运行工作负载。【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
    |签署的替补清单 |在 Metal/CUDA 捆绑包中强制执行 `<1 s` LDE 证据并捕获签名摘要以供发布批准 | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`；将清单+签名附加到发布票证上，以便下游自动化可以验证亚秒级证明指标。【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
| CUDA 捆绑包 |使 SM80 CUDA 捕获与 Metal 证据保持同步，以便清单涵盖两个 GPU 类别。 | Xeon+RTX 主机上的 `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` → `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_cuda_bench.json artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --label device_class=xeon-rtx-sm80 --sign-output`；将包装路径附加到 `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt`，将 `.json`/`.asc` 对保留在金属包旁边，并在审核员需要参考时引用种子 `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json`布局.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】
|遥测检查 |验证 Prometheus/OTEL 表面反映 `device_class="<matrix>", backend="metal"`（或记录降级）| `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` 并复制启动时发出的 `telemetry::fastpq.execution_mode` 日志。【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】|强制后退演练|记录 SRE 剧本的确定性 CPU 路径 |使用 `FASTPQ_GPU=cpu` 或 `zk.fastpq.execution_mode = "cpu"` 运行一个简短的工作负载并捕获降级日志，以便操作员可以排练回滚过程。【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
    |跟踪捕获（可选）|进行分析时，捕获调度跟踪，以便稍后可以检查内核通道/图块覆盖 |使用 `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` 重新运行一项奇偶校验测试，并将生成的跟踪日志附加到您的发布工件中。【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】

    使用发布票证归档证据，并在 `docs/source/fastpq_migration_guide.md` 中镜像相同的清单，以便分期/产品部署遵循相同的剧本。【docs/source/fastpq_migration_guide.md:1】

### 发布清单执行

将以下门添加到每个 FASTPQ 发布票证中。发布将被阻止，直到所有项目都已完成
完整并作为签名的文物附在附件中。

1. **亚秒级证明指标** — 规范的 Metal 基准测试捕获
   (`fastpq_metal_bench_*.json`) 必须证明 20000 行工作负载（32768 填充行）完成于
   <1秒。具体来说，`benchmarks.operations` 条目，其中 `operation = "lde"` 和匹配的
   `report.operations` 示例必须显示 `gpu_mean_ms ≤ 950`。超出上限要求的运行
   在签署清单之前进行调查并重新捕获。
2. **签署基准清单** — 记录新的 Metal + CUDA 捆绑包后，运行
   `cargo xtask fastpq-bench-manifest … --signing-key <path>` 发出
   `artifacts/fastpq_bench_manifest.json` 和分离的签名
   （`artifacts/fastpq_bench_manifest.sig`）。将两个文件以及公钥指纹附加到
   发布票据，以便审阅者可以独立验证摘要和签名。【xtask/src/fastpq.rs:1】
3. **证据附件** — 存储原始基准 JSON、标准输出日志（或仪器跟踪，当
   捕获），以及带有发布票证的清单/签名对。清单仅
   当票证链接到这些文物并且值班审核员确认了这些文物时，该票证被视为绿色
   `fastpq_bench_manifest.json`中记录的摘要与上传的文件匹配。【artifacts/fastpq_benchmarks/README.md:1】

## 第 6 阶段 — 强化和文档记录
- 占位符后端已停用；生产管道默认情况下没有功能切换。
- 可重复的构建（固定工具链、容器镜像）。
- 用于跟踪、SMT、查找结构的模糊器。
- 证明者级别的烟雾测试涵盖治理选票拨款和汇款转账，以在 IVM 全面推出之前保持 Stage6 固定装置的稳定。【crates/fastpq_prover/tests/realistic_flows.rs:1】
- 包含警报阈值、补救程序、容量规划指南的运行手册。
- CI 中的跨架构证明重放（x86_64、ARM64）。

### 工作台清单和释放门

发布证据现在包括涵盖金属和
CUDA 基准测试包。运行：

```bash
cargo xtask fastpq-bench-manifest \
  --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json \
  --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json \
  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \
  --signing-key secrets/fastpq_bench.ed25519 \
  --out artifacts/fastpq_bench_manifest.json
```该命令验证打包的捆绑包，强制执行延迟/加速阈值，
发出 BLAKE3 + SHA-256 摘要，并（可选）使用
Ed25519 密钥，以便发布工具可以验证出处。参见
`xtask/src/fastpq.rs`/`xtask/src/main.rs` 用于实施和
`artifacts/fastpq_benchmarks/README.md` 用于操作指导。

> **注意：** 省略 `benchmarks.poseidon_microbench` 的金属捆绑现在会导致
> 明显的一代失败。重新运行 `scripts/fastpq/wrap_benchmark.py`
>（如果您需要独立的，则为 `scripts/fastpq/export_poseidon_microbench.py`
> 摘要）每当波塞冬证据丢失时，就会发布清单
> 始终捕获标量与默认值的比较。【xtask/src/fastpq.rs:409】

`--matrix` 标志（默认为 `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json`
如果存在）加载捕获的跨设备中位数
`scripts/fastpq/capture_matrix.sh`。清单对 20000 行楼层进行编码，
每个设备类别的每次操作延迟/加速限制，因此定制
`--require-rows`/`--max-operation-ms`/`--min-operation-speedup` 覆盖无
除非您正在调试特定的回归，否则需要更长的时间。

通过将包装的基准路径附加到矩阵来刷新矩阵
`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt` 列出并运行
`scripts/fastpq/capture_matrix.sh`。该脚本快照每个设备的中位数，
发出整合的 `matrix_manifest.json`，并打印相对路径
`cargo xtask fastpq-bench-manifest` 将消耗。 AppleM4、Xeon+RTX 和
Neoverse+MI300 捕获列表 (`devices/apple-m4-metal.txt`,
`devices/xeon-rtx-sm80.txt`、`devices/neoverse-mi300.txt`) 加上它们的包装
基准捆绑包
（`fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json`，
`fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json`，
`fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json`) 现已检查
中，因此每个版本在清单之前都强制执行相同的跨设备中位数
是签名。【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-metal.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/x eon-rtx-sm80.txt:1【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【artifacts/fastpq_benchmarks/fastp q_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1【artifacts/fastpq_benchmarks/fastpq_cuda_bench_2025-11-12T09050 1Z_ubuntu24_x86_64.json:1】【工件/fastpq_benchmarks/fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json:1】

---

## 批评总结和公开行动

## 第 7 阶段 — 舰队采用和推广证据

Stage7 将证明者从“记录和基准测试”（Stage6）转变为
“默认为生产车队做好准备”。重点是遥测摄取，
跨设备捕获奇偶校验和操作员证据捆绑，以便 GPU 加速
可以被确定性地强制执行。- **第 7-1 阶段 — 车队遥测摄取和 SLO。** 生产仪表板
  (`dashboards/grafana/fastpq_acceleration.json`) 必须接线带电
  Prometheus/OTel 提供 Alertmanager 覆盖队列深度停顿的信息，
  零填充回归和静默 CPU 回退。警报包保持在
  `dashboards/alerts/fastpq_acceleration_rules.yml` 并提供相同的证据
  Stage6所需的包。【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】
  仪表板现在公开 `device_class`、`chip_family`、
  和 `gpu_kind`，让运营商通过精确的矩阵来推动 Metal 的采用
  标签（例如，`apple-m4-max`），按 Apple 芯片系列，或按分立与独立芯片。
  集成 GPU 类，无需编辑查询。
  使用 `irohad --features fastpq-gpu` 构建的 macOS 节点现在发出信号
  `fastpq_execution_mode_total{device_class,chip_family,gpu_kind,...}`，
  `fastpq_metal_queue_ratio{device_class,chip_family,gpu_kind,queue,metric}`
  （繁忙/重叠比率），以及
  `fastpq_metal_queue_depth{device_class,chip_family,gpu_kind,metric}`
  （limit、max_in_flight、dispatch_count、window_seconds）因此仪表板和
  Alertmanager 规则可以直接读取金属信号量占空比/余量
  Prometheus，无需等待基准测试包。主机现在导出
  `fastpq_zero_fill_duration_ms{device_class,chip_family,gpu_kind}` 和
  `fastpq_zero_fill_bandwidth_gbps{device_class,chip_family,gpu_kind}` 每当
  LDE 帮助程序将 GPU 评估缓冲区清零，Alertmanager 获得了
  `FastpqQueueHeadroomLow`（10m 净空 0.40ms超过15m）规则，因此队列净空和
  立即进行零填充回归页面操作，而不是等待
  下一个包装基准。新的 `FastpqCpuFallbackBurst` 页面级警报跟踪
  超过 5% 的工作负载落在 CPU 后端的 GPU 请求，
  迫使操作员捕获证据并找出 GPU 瞬时故障的根本原因
  在重试推出之前。【crates/irohad/src/main.rs:2345】【crates/iroha_telemetry/src/metrics.rs:4436】【dashboards/alerts/fastpq_acceleration_rules.yml:1】【dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1】
  SLO 设置现在还通过以下方式强制执行 ≥50% 的金属占空比目标：
  `FastpqQueueDutyCycleDrop` 规则，求平均值
  `fastpq_metal_queue_ratio{metric="busy"}` 在 15 分钟的滚动窗口中，并且
  每当 GPU 工作仍在调度但队列无法保留时发出警告
  需要占用。这使得实时遥测合同与
  强制执行 GPU 通道之前的基准测试证据。【dashboards/alerts/fastpq_acceleration_rules.yml:1】【dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1】
- **Stage7-2 — 跨设备捕获矩阵。** 新
  `scripts/fastpq/capture_matrix.sh` 构建
  来自每个设备的 `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json`
  `artifacts/fastpq_benchmarks/matrix/devices/` 下的捕获列表。苹果M4,
  Xeon+RTX 和 Neoverse+MI300 中值现在与它们一起存在于仓库中
  打包的捆绑包，因此 `cargo xtask fastpq-bench-manifest` 加载清单
  自动执行 20000 行下限，并应用于每个设备
  在发布捆绑包之前没有定制 CLI 标志的延迟/加速限制已批准。【scripts/fastpq/capture_matrix.sh:1】【artifacts/fastpq_benchmarks/matrix/matrix_manifest.json:1】【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-met al.txt:1【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【xtask/src/fastpq.rs:1】
现在，汇总的不稳定原因与矩阵一起出现：通过
`--reason-summary-out` 到 `scripts/fastpq/geometry_matrix.py` 发出
由主机标签和源键入的失败/警告原因的 JSON 直方图
摘要，因此 Stage7-2 审阅者可以在以下位置查看 CPU 回退或缺失的遥测数据：
无需扫描整个 Markdown 表即可一目了然。现在同一个助手
接受 `--host-label chip_family:Chip` （对多个密钥重复），因此
Markdown/JSON 输出包括精选的主机标签列，而不是隐藏
原始摘要中的元数据，使得过滤操作系统版本或
编译Stage7-2证据包时的Metal驱动版本。【scripts/fastpq/geometry_matrix.py:1】
几何扫描还将 ISO8601 `started_at` / `completed_at` 字段标记到
摘要、CSV 和 Markdown 输出，因此捕获包可以证明窗口
Stage7-2矩阵合并多个实验运行时的每个主机。【scripts/fastpq/launch_geometry_sweep.py:1】
`scripts/fastpq/stage7_bundle.py` 现在将几何矩阵与
`row_usage/*.json` 快照到单个 Stage7 捆绑包中 (`stage7_bundle.json`
+ `stage7_geometry.md`)，通过验证传输比率
`validate_row_usage_snapshot.py` 和持久主机/env/reason/source 摘要
因此，推出门票可以附加一个确定性的人工制品，而不是杂耍
每个主机表。【scripts/fastpq/stage7_bundle.py:1】【scripts/fastpq/validate_row_usage_snapshot.py:1】
- **第 7-3 阶段 — 运营商采用证据和回滚演习。** 新
  `docs/source/fastpq_rollout_playbook.md` 描述了工件包
  （`fastpq_bench_manifest.json`，包装金属/CUDA 捕获，Grafana 导出，
  Alertmanager 快照、回滚日志）必须随每个推出票证一起提供
  加上分阶段（试点→斜坡→默认）时间线和强制后备演习。
  `ci/check_fastpq_rollout.sh` 验证这些捆绑包，以便 CI 强制执行 Stage7
  发布前的大门向前推进。发布管道现在可以拉相同的
  通过捆绑到 `artifacts/releases/<version>/fastpq_rollouts/…`
  `scripts/run_release_pipeline.py --fastpq-rollout-bundle <path>`，确保
  签署的清单和推出证据保留在一起。参考包已存在
  在 `artifacts/fastpq_rollouts/20250215T101500Z/fleet-alpha/canary/` 下保留
  GitHub 工作流程 (`.github/workflows/fastpq-rollout.yml`) 绿色而真实
  审核提交的提交。

### Stage7 FFT 队列扇出`crates/fastpq_prover/src/metal.rs` 现在实例化一个 `QueuePolicy`
每当主机报告一个命令时，就会自动生成多个 Metal 命令队列
独立GPU。集成 GPU 保持单队列路径
(`MIN_QUEUE_FANOUT = 1`)，而离散设备默认有两个队列，并且仅
当工作负载至少覆盖 16 列时展开。两种启发式方法都可以调整
通过新的 `FASTPQ_METAL_QUEUE_FANOUT` 和 `FASTPQ_METAL_COLUMN_THRESHOLD`
环境变量，调度程序循环 FFT/LDE 批处理
在同一队列上发出配对后平铺调度之前的活动队列
保留订购保证。【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:772】【crates/fastpq_prover/src/metal.rs:900】
节点操作员不再需要手动导出这些环境变量：
`iroha_config` 配置文件公开 `fastpq.metal_queue_fanout` 和
`fastpq.metal_queue_column_threshold` 和 `irohad` 通过以下方式应用它们
Metal 后端初始化之前的 `fastpq_prover::set_metal_queue_policy`
舰队配置文件无需定制启动包装即可保持可复制性。【crates/irohad/src/main.rs:1879】【crates/fastpq_prover/src/lib.rs:60】
现在，每当工作负载刚刚达到时，逆 FFT 批次就会坚持到单个队列
达到扇出阈值（例如，16 列通道平衡捕获），
恢复 WP2-D ≥1.0× 奇偶校验，同时保留大列 FFT/LDE/Poseidon
在多队列路径上调度。【crates/fastpq_prover/src/metal.rs:2018】

辅助测试执行队列策略限制和解析器验证，以便 CI 可以
证明 Stage7 启发式，无需在每个构建器上安装 GPU 硬件，
并且特定于 GPU 的测试强制扇出覆盖以保持重放覆盖范围
与新的默认值同步。【crates/fastpq_prover/src/metal.rs:2163】【crates/fastpq_prover/src/metal.rs:2236】

### Stage7-1 设备标签和警报合同

`scripts/fastpq/wrap_benchmark.py` 现在在 macOS 捕获上探测 `system_profiler`
在每个包装的基准测试中托管和记录硬件标签，以便舰队遥测
捕获矩阵可以根据设备进行旋转，无需定制电子表格。一个
20000 行金属捕获现在包含以下条目：

```json
"labels": {
  "device_class": "apple-m4-pro",
  "chip_family": "m4",
  "chip_bin": "pro",
  "gpu_kind": "integrated",
  "gpu_vendor": "apple",
  "gpu_bus": "builtin",
  "gpu_model": "Apple M4 Pro"
}
```

这些标签与 `benchmarks.zero_fill_hotspots` 一起被摄取，
`benchmarks.metal_dispatch_queue`所以Grafana快照，捕获矩阵
(`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) 和警报管理器
所有证据都同意产生这些指标的硬件类别。的
当实验室主机缺少 `--label` 标志时，仍然允许手动覆盖
`system_profiler`，但自动探测的标识符现在涵盖 AppleM1–M4 和
开箱即用的独立 PCIe GPU。【scripts/fastpq/wrap_benchmark.py:1】

Linux 捕获得到相同的处理：`wrap_benchmark.py` 现在检查
`/proc/cpuinfo`、`nvidia-smi`/`rocm-smi` 和 `lspci`，以便 CUDA 和 OpenCL 运行
派生 `cpu_model`、`gpu_model` 和规范的 `device_class` (`xeon-rtx-sm80`
对于 Stage7 CUDA 主机，对于 MI300A 实验室为 `neoverse-mi300`）。运营商可以
仍然覆盖自动检测的值，但 Stage7 证据包不再
需要手动编辑以使用正确的设备标记 Xeon/Neoverse 捕获
元数据。在运行时，每个主机设置 `fastpq.device_class`、`fastpq.chip_family` 和
`fastpq.gpu_kind`（或相应的 `FASTPQ_*` 环境变量）到
与捕获包中出现的矩阵标签相同，因此 Prometheus 导出
`fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}` 和
FASTPQ 加速仪表板可以按三个轴中的任何一个进行过滤。的
Alertmanager 规则聚合在同一标签集上，让操作员可以绘制图表
每个硬件配置文件的采用、降级和回退，而不是单个
车队比例。【crates/iroha_config/src/parameters/user.rs:1224】【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】

遥测 SLO/警报合约现在将捕获的指标与 Stage7 联系起来
盖茨。下表总结了信号和执行点：

|信号|来源 |目标/触发|执法|
| ------ | ------ | ---------------- | ----------- |
| GPU采用率| Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", device_class="…", chip_family="…", gpu_kind="…", backend="metal"}` | ≥95% 的每（device_class、chip_family、gpu_kind）分辨率必须落在 `resolved="gpu", backend="metal"` 上；当任何三元组在 15m 内下降到 50% 以下时的页面 | `FastpqMetalDowngrade` 警报（页面）【dashboards/alerts/fastpq_acceleration_rules.yml:1】 |
|后端间隙| Prometheus `fastpq_execution_mode_total{backend="none", device_class="…", chip_family="…", gpu_kind="…"}` |每个三元组必须保持为 0；任何持续 (>10m) 爆发后发出警告 | `FastpqBackendNoneBurst` 警报（警告）【dashboards/alerts/fastpq_acceleration_rules.yml:21】 |
| CPU回退率| Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", backend="cpu", device_class="…", chip_family="…", gpu_kind="…"}` |对于任何三元组，≤5% 的 GPU 请求的证明可能会登陆 CPU 后端；当三元组超过 5% ≥10m 时的页面 | `FastpqCpuFallbackBurst` 警报（页面）【dashboards/alerts/fastpq_acceleration_rules.yml:32】 |
|金属队列占空比| Prometheus `fastpq_metal_queue_ratio{metric="busy", device_class="…", chip_family="…", gpu_kind="…"}` |每当 GPU 作业排队时，滚动 15m 平均值必须保持 ≥50%；当 GPU 请求持续存在时利用率低于目标时发出警告 | `FastpqQueueDutyCycleDrop` 警报（警告）【dashboards/alerts/fastpq_acceleration_rules.yml:98】 |
|队列深度和零填充预算|包装基准 `metal_dispatch_queue` 和 `zero_fill_hotspots` 块 |对于规范的 20000 行跟踪，`max_in_flight` 必须至少比 `limit` 低一个槽，并且 LDE 零填充平均值必须保持 ≤0.4ms (≈80GB/s)；任何回归都会阻碍推出捆绑包 |通过 `scripts/fastpq/wrap_benchmark.py` 输出进行审查并附加到 Stage7 证据包 (`docs/source/fastpq_rollout_playbook.md`)。 |
|运行时队列净空| Prometheus `fastpq_metal_queue_depth{metric="limit|max_in_flight", device_class="…", chip_family="…", gpu_kind="…"}` |每个三元组为 `limit - max_in_flight ≥ 1`； 10m后无净空警告 | `FastpqQueueHeadroomLow` 警报（警告）【dashboards/alerts/fastpq_acceleration_rules.yml:41】 |
|运行时零填充延迟 | Prometheus `fastpq_zero_fill_duration_ms{device_class="…", chip_family="…", gpu_kind="…"}` |最新的填零样本必须保持≤0.40ms（Stage7限制）| `FastpqZeroFillRegression` 警报（页面）【dashboards/alerts/fastpq_acceleration_rules.yml:58】 |包装器直接强制执行零填充行。通行证
`--require-zero-fill-max-ms 0.40` 至 `scripts/fastpq/wrap_benchmark.py` 及其
当工作台 JSON 缺乏零填充遥测或最热时将失败
零填充样本超出了 Stage7 预算，从而阻止了推出捆绑包
在没有强制证据的情况下发货。【scripts/fastpq/wrap_benchmark.py:1008】

#### Stage7-1 警报处理清单

上面列出的每个警报都会提供特定的待命演习，以便操作员收集
发布包所需的相同工件：

1. **`FastpqQueueHeadroomLow`（警告）。** 运行即时 Prometheus 查询
   对于 `fastpq_metal_queue_depth{metric=~"limit|max_in_flight",device_class="<matrix>"}` 和
   从 `fastpq-acceleration` 捕获 Grafana“队列净空”面板
   板。将查询结果记录在
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_headroom.prom`
   与警报 ID 一起，以便发布包证明警告是
   在队列饥饿之前确认。【dashboards/grafana/fastpq_acceleration.json:1】
2. **`FastpqZeroFillRegression`（页）。** 检查
   `fastpq_zero_fill_duration_ms{device_class="<matrix>"}` 并且，如果度量为
   有噪音，在最新的工作台 JSON 上重新运行 `scripts/fastpq/wrap_benchmark.py`
   刷新 `zero_fill_hotspots` 块。附上 promQL 输出，
   截图，并将bench文件刷新到rollout目录；这创造了
   `ci/check_fastpq_rollout.sh` 在发布期间期望的相同证据
   验证。【scripts/fastpq/wrap_benchmark.py:1】【ci/check_fastpq_rollout.sh:1】
3. **`FastpqCpuFallbackBurst`（页）。** 确认
   `fastpq_execution_mode_total{requested="gpu",backend="cpu"}` 超过 5%
   地板，然后采样 `irohad` 日志以获取相应的降级消息
   （`telemetry::fastpq.execution_mode resolved="cpu"`）。存储 promQL 转储
   加上 `metrics_cpu_fallback.prom`/`rollback_drill.log` 中的日志摘录，因此
   捆绑包展示了影响和运营商的认可。
4. **证据打包。** 任何警报清除后，重新运行中的 Stage7-3 步骤
   部署手册（Grafana 导出、警报快照、回滚演练）以及
   在重新附加之前通过 `ci/check_fastpq_rollout.sh` 重新验证捆绑包
   前往发行票。【docs/source/fastpq_rollout_playbook.md:114】

喜欢自动化的操作员可以运行
`scripts/fastpq/capture_alert_evidence.sh --device-class <label> --out <bundle-dir>`
查询 Prometheus API 的队列余量、零填充和 CPU 回退
上面列出的指标；助手写入捕获的 JSON（前缀为
原始 promQL）转换为 `metrics_headroom.prom`、`metrics_zero_fill.prom`，以及
`metrics_cpu_fallback.prom` 在所选的推出目录下，因此这些文件
可以附加到捆绑包中，无需手动调用curl。`ci/check_fastpq_rollout.sh` 现在强制执行队列余量和零填充
直接预算。它解析由引用的每个 `metal` 基准
`fastpq_bench_manifest.json`，检查
`benchmarks.metal_dispatch_queue.{limit,max_in_flight}` 和
`benchmarks.zero_fill_hotspots[]`，并且当净空下降时捆绑失败
低于一个插槽或当任何 LDE 热点报告 `mean_ms > 0.40` 时。这保持了
CI 中的 Stage7 遥测防护，与在
Grafana快照及发布证据。【ci/check_fastpq_rollout.sh#L1】
作为同一验证过程的一部分，脚本现在坚持每个包装的
基准测试带有自动检测的硬件标签（`metadata.labels.device_class`
和 `metadata.labels.gpu_kind`）。缺少这些标签的捆绑包会立即失败，
保证发布工件、Stage7-2 矩阵清单和运行时
仪表板均引用完全相同的设备类名称。

Grafana“最新基准”面板和相关的推出捆绑包现在引用
`device_class`、零填充预算、队列深度快照等随叫随到的工程师
可以将生产遥测与标志期间使用的确切捕获类别相关联
关闭。未来的矩阵条目继承相同的标签，这意味着 Stage7-2 设备
列表和 Prometheus 仪表板共享 AppleM4 的单一命名方案，
M3 Max 和即将推出的 MI300/RTX 捕获。

### Stage7-1 舰队遥测操作手册

默认情况下启用 GPU 通道之前请遵循此清单，以便车队遥测
和 Alertmanager 规则反映了发布准备期间捕获的相同证据：

1. **标签捕获和运行时主机。** `python3 scripts/fastpq/wrap_benchmark.py`
   已发出 `metadata.labels.device_class`、`chip_family` 和 `gpu_kind`
   对于每个包装好的 JSON。使这些标签与
   `fastpq.{device_class,chip_family,gpu_kind}`（或
   `FASTPQ_{DEVICE_CLASS,CHIP_FAMILY,GPU_KIND}` 环境变量）在 `iroha_config` 内
   所以运行时指标发布
   `fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}`
   以及具有相同标识符的 `fastpq_metal_queue_*` 仪表
   在 `artifacts/fastpq_benchmarks/matrix/devices/*.txt` 中。上演新剧时
   类，通过重新生成矩阵清单
   `scripts/fastpq/capture_matrix.sh --devices artifacts/fastpq_benchmarks/matrix/devices`
   因此 CI 和仪表板可以理解附加标签。
2. **验证队列计量和采用指标。** 运行 `irohad --features fastpq-gpu`
   在 Metal 主机上并抓取遥测端点以确认实时队列
   仪表正在出口：

   ```bash
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_metal_queue_(ratio|depth)'
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_execution_mode_total'
   ```第一个命令证明信号量采样器正在发出 `busy`，
   `overlap`、`limit` 和 `max_in_flight` 系列和第二个显示是否
   每个设备类别都解析为 `backend="metal"` 或回退到
   `backend="cpu"`。之前通过 Prometheus/OTel 连接抓取目标
   导入仪表板，以便 Grafana 可以立即绘制车队视图。
3. **安装仪表板+警报包。** 导入
   `dashboards/grafana/fastpq_acceleration.json` 转换为 Grafana（保留
   内置设备类、芯片系列和 GPU 类型模板变量）并加载
   `dashboards/alerts/fastpq_acceleration_rules.yml`一起进入Alertmanager
   及其单元测试夹具。规则包附带 `promtool` 线束；跑
   `promtool test rules dashboards/alerts/tests/fastpq_acceleration_rules.test.yml`
   每当规则发生变化以证明 `FastpqMetalDowngrade` 且
   `FastpqBackendNoneBurst` 仍然在记录的阈值下触发。
4. **通过证据包进行释放。** 保留
   `docs/source/fastpq_rollout_playbook.md` 生成部署时很方便
   提交，以便每个捆绑包都带有打包的基准，Grafana 导出，
   警报包、队列遥测证明和回滚日志。 CI 已经强制执行
   合约：`make check-fastpq-rollout`（或调用
   `ci/check_fastpq_rollout.sh --bundle <path>`) 验证捆绑包，重新运行
   警报测试，并在队列净空或零填充时拒绝注销
   预算回归。
5. **将警报与修复联系起来。** 当 Alertmanager 寻呼时，使用 Grafana
   板和步骤 2 中的原始 Prometheus 计数器，以确认是否
   降级源于队列饥饿、CPU 回退或 backend=none 突发。
运行手册位于
本文档加上 `docs/source/fastpq_rollout_playbook.md`；更新
释放带有相关 `fastpq_execution_mode_total` 的票证，
`fastpq_metal_queue_ratio` 和 `fastpq_metal_queue_depth` 一起摘录
包含指向 Grafana 面板和警报快照的链接，以便审阅者可以查看
到底是哪个 SLO 触发的。

### WP2-E — 逐步金属分析快照

`scripts/fastpq/src/bin/metal_profile.rs` 总结了包裹的 Metal 捕获
因此可以随着时间的推移跟踪低于 900 毫秒的目标（运行
`cargo run --manifest-path scripts/fastpq/Cargo.toml --bin metal_profile -- <capture.json>`）。
新的 Markdown 助手
`scripts/fastpq/metal_capture_summary.py fastpq_metal_bench_20k_latest.json --label "20k snapshot (pre-override)"`
生成下面的阶段表（它打印 Markdown 以及文本
摘要，以便 WP2-E 票据可以逐字嵌入证据）。跟踪两次捕获
现在：

> **新的 WP2-E 仪器：** `fastpq_metal_bench --gpu-probe ...` 现在发出
> 检测快照（请求/解析执行模式，`FASTPQ_GPU`
> 覆盖、检测到的后端和枚举的 Metal 设备/注册表 ID）
> 在任何内核运行之前。每当强制 GPU 仍在运行时捕获此日志
> 回退到 CPU 路径，以便证据包记录主机看到的内容
> `MTLCopyAllDevices` 返回零以及哪些覆盖在
> 基准测试。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:603】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2616】> **舞台捕捉助手：** `cargo xtask fastpq-stage-profile --trace --out-dir artifacts/fastpq_stage_profiles/<label>`
> 现在分别为 FFT、LDE 和 Poseidon 驱动 `fastpq_metal_bench`，
> 将原始 JSON 输出存储在每个阶段目录下，并发出单个
> `stage_profile_summary.json` 捆绑包，记录 CPU/GPU 计时、队列深度
> 遥测、列分段统计、内核配置文件和相关跟踪
> 文物。通过 `--stage fft --stage lde --stage poseidon` 来定位子集，
> `--trace-template "Metal System Trace"` 选择特定的 xctrace 模板，
> 和 `--trace-dir` 将 `.trace` 捆绑包路由到共享位置。附上
> 摘要 JSON 加上每个 WP2-E 问题生成的跟踪文件，以便审阅者
> 可以比较队列占用率 (`metal_dispatch_queue.*`)、重叠率和
> 无需手动探索多个运行即可捕获发射几何形状
> `fastpq_metal_bench` 调用。【xtask/src/fastpq.rs:721】【xtask/src/main.rs:3187】

> **队列/分期证据助手 (2026-05-09):** `scripts/fastpq/profile_queue.py` 现在
> 摄取一个或多个 `fastpq_metal_bench` JSON 捕获并发出 Markdown 表和
> 机器可读的摘要 (`--markdown-out/--json-out`)，因此队列深度、重叠率和
> 主机端分段遥测可以与每个 WP2-E 制品一起使用。跑步
> `python3 scripts/fastpq/profile_queue.py fastpq_metal_bench_poseidon.json fastpq_metal_bench_20k_new.json --json-out artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.json` 生成了下表并标记了已存档的 Metal 捕获仍然报告
> `dispatch_count = 0` 和 `column_staging.batches = 0` — WP2-E.1 保持打开状态，直到金属
> 仪器在启用遥测的情况下重建。生成的 JSON/Markdown 工件已上线
> 在 `artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.{json,md}` 下进行审核。
> 助手现在（2026-05-19）还显示了 Poseidon 管道遥测（`pipe_depth`，
> `batches`、`chunk_columns` 和 `fallbacks`) 在 Markdown 表和 JSON 摘要中，
> 因此 WP2-E.4/6 审阅者可以证明 GPU 是否保持在流水线路径上以及是否存在任何问题
> 在未打开原始捕获的情况下发生回退。【scripts/fastpq/profile_queue.py:1】> **阶段概况摘要（2026-05-30）：** `scripts/fastpq/stage_profile_report.py` 消耗
> 由 `cargo xtask fastpq-stage-profile` 发出的 `stage_profile_summary.json` 捆绑包以及
> 呈现 Markdown 和 JSON 摘要，以便 WP2-E 审阅者可以将证据复制到票证中
> 无需手动转录时间。调用
> `python3 scripts/fastpq/stage_profile_report.py artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.json --label "m3-lab" --markdown-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.md --json-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.jsonl`
> 生成列出 GPU/CPU 平均值、加速增量、跟踪覆盖范围的确定性表
> 每个阶段的遥测间隙。 JSON 输出镜像表并记录每个阶段的问题标签
>（`trace missing`、`queue telemetry missing` 等），因此治理自动化可以区分主机
> WP2-E.1 至 WP2-E.6 中引用的运行。
> **主机/设备重叠保护 (2026-06-04):** `scripts/fastpq/profile_queue.py` 现在注释
> FFT/LDE/Poseidon 等待比率以及每级展平/等待毫秒总数并发出
> 每当 `--max-wait-ratio <threshold>` 检测到重叠不良时就会出现问题。使用
> `python3 scripts/fastpq/profile_queue.py --max-wait-ratio 0.20 fastpq_metal_bench_20k_latest.json --markdown-out artifacts/fastpq_benchmarks/<stamp>/queue.md`
> 捕获具有明确等待率的 Markdown 表和 JSON 包，以便 WP2-E.5 票证
> 可以显示双缓冲窗口是否保持 GPU 供电。纯文本控制台输出也
> 列出每相比率，使随叫随到的调查变得更加容易。
> **遥测保护 + 运行状态 (2026-06-09):** `fastpq_metal_bench` 现在发出 `run_status` 块
>（后端标签、调度计数、原因）和新的 `--require-telemetry` 标志运行失败
> 每当 GPU 计时或队列/分段遥测丢失时。 `profile_queue.py` 渲染运行
> 作为专用栏的状态并在问题列表中显示非 `ok` 状态，以及
> `launch_geometry_sweep.py` 将相同的状态线程化到警告/分类中，因此矩阵不能
> 较长的承认捕获会默默地回退到 CPU 或跳过队列检测。
> **Poseidon/LDE 自动调整 (2026-06-12)：** `metal_config::poseidon_batch_multiplier()` 现已扩展
> 使用 Metal 工作集提示和 `lde_tile_stage_target()` 提高了独立 GPU 上的图块深度。
> 应用的乘数和拼贴限制包含在 `metal_heuristics` 块中
> `fastpq_metal_bench` 输出并由 `scripts/fastpq/metal_capture_summary.py` 渲染，因此 WP2-E
> 包记录每次捕获中使用的确切管道旋钮，而无需挖掘原始 JSON。【crates/fastpq_prover/src/metal_config.rs:1】【crates/fastpq_prover/src/metal.rs:2833】【scripts/fastpq/metal_capture_summary.py:1】

|标签|调度|忙碌 |重叠|最大深度| FFT 展平 | FFT 等待 | FFT 等待% | LDE 扁平化 | LDE 等待 | LDE 等待% |波塞冬压扁|海神等等|波塞冬等待% |管道深度|管材批次|管道后备|
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fastpq_metal_bench_poseidon | fastpq_metal_bench_poseidon | 0 | 0.0% | 0.0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |
| fastpq_metal_bench_20k_new | fastpq_metal_bench_20k_new | 0 | 0.0% | 0.0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |

#### 20k 快照（覆盖前）

`fastpq_metal_bench_20k_latest.json`|舞台|专栏 |输入长度 | GPU 平均值（毫秒）| CPU 平均值（毫秒）| GPU 份额 |加速| Δ CPU（毫秒）|
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
|快速傅里叶变换 | 16 | 16 32768 | 130.986 毫秒（115.761–167.755）| 112.616 毫秒（95.335–132.929）| 2.4% | 0.860× | −18.370 |
|快速傅里叶变换 | 16 | 16 32768 | 129.296 毫秒（111.127–142.955）| 158.144 毫秒（126.847–237.887）| 2.4% | 1.223× | +28.848 |
| LDE | 16 | 16 262144 | 262144 1570.656 毫秒（1544.397–1584.502）| 1752.523 毫秒（1548.807–2191.930）| 29.2% | 1.116× | +181.867 |
|波塞冬 | 16 | 16 524288 | 524288 3548.329 毫秒（3519.881–3576.041）| 3642.706 毫秒（3539.055–3758.279）| 66.0% | 1.027× | +94.377 |

主要观察结果：1. GPU 总计为 5.379 秒，比 900 毫秒目标**4.48 秒。波塞冬
   哈希仍然在运行时占据主导地位（约 66%），LDE 内核位居第二
   位置（≈29%），因此WP2-E需要同时攻击Poseidon管道深度和
   CPU 回退消失之前的 LDE 内存驻留/平铺计划。
2. 即使 IFFT 在标量上 >1.22×，FFT 仍然是回归 (0.86×)
   路径。我们需要一次发射几何扫描
   （`FASTPQ_METAL_{FFT,LDE}_COLUMNS` + `FASTPQ_METAL_QUEUE_FANOUT`）了解
   是否可以在不伤害已经更好的情况下挽救 FFT 占用率
   IFFT 时序。 `scripts/fastpq/launch_geometry_sweep.py` 助手现在驱动
   这些实验端到端：传递以逗号分隔的覆盖（例如，
   `--fft-columns 16,32 --queue-fanout 1,2` 和
   `--poseidon-lanes auto,256`），它将调用
   `fastpq_metal_bench` 对于每个组合，将 JSON 有效负载存储在
   `artifacts/fastpq_geometry/<timestamp>/`，并保留 `summary.json` 捆绑包
   描述每次运行的队列比率、FFT/LDE 启动选择、GPU 与 CPU 时序、
   以及主机元数据（主机名/标签、平台三元组、检测到的设备
   类、GPU 供应商/型号），因此跨设备比较具有确定性
   出处。助手现在还在旁边写入 `reason_summary.json`
   默认情况下summary，使用与几何矩阵相同的分类器进行roll
   CPU 回退和遥测丢失。使用 `--host-label staging-m3` 进行标记
   来自共享实验室的捕获。
   配套的 `scripts/fastpq/geometry_matrix.py` 工具现在可以摄取一个或
   更多摘要捆绑包 (`--summary hostA/summary.json --summary hostB/summary.json`)
   并发出 Markdown/JSON 表，将每个启动形状标记为*稳定*
   （FFT/LDE/Poseidon GPU 计时捕获）或*不稳定*（超时、CPU 回退、
   非金属后端，或缺少遥测）与主机列一起。的
   表现在包括已解析的 `execution_mode`/`gpu_backend` 以及
   `Reason` 列，因此 CPU 回退和缺少 GPU 时序在
   即使存在计时块，Stage7 矩阵也是如此；摘要行很重要
   稳定运行与总运行。通过 `--operation fft|lde|poseidon_hash_columns`
   当扫描需要隔离单个阶段时（例如，分析
   Poseidon 单独）并保留 `--extra-args` 空闲用于特定于工作台的标志。
   助手接受任何
   命令前缀（默认为 `cargo run … fastpq_metal_bench`）加上可选
   `--halt-on-error` / `--timeout-seconds` 提供保护，以便性能工程师可以
   在不同的机器上重现扫描，同时收集可比的，
   Stage7 的多设备证据包。
3、`metal_dispatch_queue`报`dispatch_count = 0`，所以队列占用
   即使 GPU 内核运行，遥测也丢失。 Metal 运行时现在使用
   获取/释放队列/列分段切换的栅栏，以便工作线程
   观察仪器标志，几何矩阵报告会调用
   当 FFT/LDE/Poseidon GPU 时序缺失时，启动形状不稳定。保留
   将 Markdown/JSON 矩阵附加到 WP2-E 票证，以便审阅者可以看到
   一旦队列遥测可用，哪些组合仍然失败。`run_status` 防护和 `--require-telemetry` 标志现在无法捕获
   每当 GPU 计时缺失或队列/暂存遥测缺失时，
   dispatch_count=0 次运行不会再被忽视地滑入 WP2-E 捆绑包中。
   `fastpq_metal_bench` 现在公开 `--require-gpu`，并且
   `launch_geometry_sweep.py` 默认启用它（选择退出
   `--allow-cpu-fallback`)，因此 CPU 回退和金属检测故障中止
   立即，而不是用非GPU遥测污染Stage7矩阵。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs】【scripts/fastpq/launch_geometry_sweep.py】
4. 之前的零填充指标也因为同样的原因消失了；击剑修复
   保持主机仪器处于活动状态，因此下一次捕获应包括
   `zero_fill` 块没有合成计时。

#### 20k 快照，`FASTPQ_GPU=gpu`

`fastpq_metal_bench_20k_refresh.json`

|舞台|专栏 |输入长度| GPU 平均值（毫秒）| CPU 平均值（毫秒）| GPU 份额 |加速| Δ CPU（毫秒）|
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
|快速傅里叶变换 | 16 | 16 32768 | 79.951 毫秒（65.645–93.193）| 83.289 毫秒（59.956–107.585）| 0.3% | 1.042× | +3.338 |
|快速傅里叶变换 | 16 | 16 32768 | 78.605 毫秒 (69.986–83.726) | 93.898 毫秒（80.656–119.625）| 0.3% | 1.195×| +15.293 |
| LDE | 16 | 16 262144 | 262144 657.673 毫秒（619.219–712.367）| 669.537 毫秒（619.716–723.285）| 2.1% | 1.018× | +11.864 |
|波塞冬 | 16 | 16 524288 | 524288 30004.898 毫秒（27284.117–32945.253）| 29087.532 毫秒（24969.810–33020.517）| 97.4% | 0.969× | −917.366 |

观察结果：

1. 即使使用 `FASTPQ_GPU=gpu`，此捕获仍然反映了 CPU 回退：
   每次迭代约 30 秒，`metal_dispatch_queue` 停留在零。当
   设置了覆盖，但主机无法发现 Metal 设备，CLI 现在退出
   在运行任何内核并打印请求/解析模式以及
   后端标签，以便工程师可以判断是否是检测、权利或
   Metallib 查找导致降级。运行`fastpq_metal_bench --gpu-probe
   --rows …` with `FASTPQ_DEBUG_METAL_ENUM=1` 捕获枚举日志并
   在重新运行分析器之前修复底层检测问题。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2636】
2. 零填充遥测现在记录真实样本（32MiB 上 18.66ms），证明
   防护修复有效，但队列增量在 GPU 调度之前仍然不存在
   成功。
3.由于后端不断降级，Stage7遥测门仍然存在
   阻塞：队列净空证据和海神重叠需要真正的 GPU
   跑。

这些捕获现在锚定了 WP2-E 积压工作。下一步行动：收集分析器
火焰图和队列日志（一旦后端在 GPU 上执行），目标
重新审视 FFT 之前的 Poseidon/LDE 瓶颈，并解锁后端回退
所以 Stage7 遥测拥有真实的 GPU 数据。

### 优势
- 增量分段、跟踪优先设计、透明的 STARK 堆栈。### 高优先级行动项目
1. 实施包装/订购装置并更新 AIR 规范。
2. 最终确定 Poseidon2 提交 `3f2b7fe` 并发布示例 SMT/查找向量。
3. 将工作示例（`lookup_grand_product.md`、`smt_update.md`）与夹具一起维护。
4. 添加附录 A，记录健全性推导和 CI 拒绝方法。

### 已解决的设计决策
- P1 中禁用 ZK（仅限正确性）；在未来阶段重新审视。
- 权限表根源自治理状态；批次将表视为只读并通过查找证明成员身份。
- 缺少关键证明使用零叶加邻居见证和规范编码。
- 删除语义 = 规范键空间内的叶值设置为零。

使用本文档作为规范参考；将其与源代码、固定装置和附录一起更新以避免漂移。

## 附录 A — 稳健性推导

This appendix explains how the “Soundness & SLOs” table is produced and how CI enforces the ≥128-bit floor mentioned earlier.

### 符号
- `N_trace = 2^k` — 排序并填充为 2 的幂后的迹线长度。
- `b` — 膨胀系数 (`N_eval = N_trace × b`)。
- `r` — FRI 数量（规范集为 8 或 16）。
- `ℓ` — FRI 减少数量（`layers` 列）。
- `q` — 验证者查询每个证明（`queries` 列）。
- `ρ` — 列规划器报告的有效码率：`ρ = max_i(degree_i / domain_i)` 超过第一轮 FRI 的约束。

Goldilocks 基域具有 `|F| = 2^64 - 2^32 + 1`，因此 Fiat-Shamir 碰撞受 `q / 2^64` 限制。 Grinding增加了一个正交的`2^{-g}`因子，`g = 23`用于`fastpq-lane-balanced`，`g = 21`用于延迟配置文件。【crates/fastpq_isi/src/params.rs:65】

### 分析界限

对于恒定速率 DEEP-FRI，统计故障概率满足

```
p_fri ≤ Σ_{j=0}^{ℓ-1} ρ^{q} = ℓ · ρ^{q}
```

因为每层都会以相同的因子 `r` 减少多项式次数和域宽度，从而保持 `ρ` 恒定。表的 `est bits` 列报告 `⌊-log₂ p_fri⌋`； Fiat-Shamir 和磨削可提供额外的安全裕度。

### 规划器输出和工作计算

对代表性批次运行 Stage1 列规划器会产生：

| Parameter set | `N_trace` | `b` | `N_eval` | `ρ`（规划师）|有效度(`ρ × N_eval`)| `ℓ` | `q` | `-log₂(ℓ · ρ^{q})` |
| ------------- | --------- | ---| -------- | ------------- | -------------------------------- | ---| ---| ------------------ |
|平衡20k批次| `2^15` | 8 | 262144 | 262144 0.077026 | 0.077026 20192 | 20192 5 | 52 | 52 190 位 |
|吞吐量 65k 批次 | `2^16` | 8 | 524288 | 524288 0.200208 | 104967 | 104967 6 | 58 | 58 132 位 |
|延迟 131k 批次 | `2^17` | 16 | 16 2097152 | 2097152 0.209492 | 439337 | 439337 5 | 64 | 64 142 位 |示例（平衡 20k 批次）：
1. `N_trace = 2^15`，所以`N_eval = 2^15 × 8 = 2^18`。
2. Planner 仪器报告 `ρ = 0.077026`，因此 `p_fri = 5 × ρ^{52} ≈ 6.4 × 10^{-58}`。
3. `-log₂ p_fri = 190 bits`，与表条目匹配。
4. Fiat-Shamir 碰撞最多添加 `2^{-58.3}`，研磨 (`g = 23`) 减去另一个 `2^{-23}`，使总稳健性保持在 160 位以上。

### CI 拒绝采样工具

每次 CI 运行都会执行蒙特卡罗工具，以确保经验测量值保持在分析界限的 ±0.6 位以内：
1. 绘制规范参数集并合成具有匹配行数的 `TransitionBatch`。
2. 构建跟踪，翻转随机选择的约束（例如，干扰查找总乘积或 SMT 同级），并尝试生成证明。
3. 重新运行验证器，重新采样 Fiat-Shamir 挑战（包括研磨），并记录篡改证明是否被拒绝。
4. 对每个参数集 16384 个种子重复此操作，并将观察到的拒绝率的 99% Clopper-Pearson 下限转换为比特。

如果测量的下限低于 128 位，则作业会立即失败，因此在合并之前会捕获规划器、折叠循环或转录连线中的回归。

## 附录 B — 域根推导

Stage0 将跟踪和评估生成器固定到 Poseidon 派生常量，因此所有实现共享相同的子组。

### 程序
1. **种子选择。** 将 UTF-8 标签 `fastpq:v1:domain_roots` 吸收到 FASTPQ 中其他地方使用的 Poseidon2 海绵中（状态宽度 = 3，速率 = 2，四个完整轮 + 57 个部分轮）。输入重用来自 `pack_bytes` 的 `[len, limbs…]` 编码，生成基本生成器 `g_base = 7`。【crates/fastpq_prover/src/packing.rs:44】【scripts/fastpq/src/bin/poseidon_gen.rs:1】
2. **迹线发生器。** 计算 `trace_root = g_base^{(p-1)/2^{trace_log_size}} mod p` 并在半功率不为 1 时验证 `trace_root^{2^{trace_log_size}} = 1`。
3. **LDE 生成器。** 对 `lde_log_size` 重复相同的求幂运算，得到 `lde_root`。
4. **陪集选择。** Stage0 使用基本子组 (`omega_coset = 1`)。未来的陪集可以吸收额外的标签，例如 `fastpq:v1:domain_roots:coset`。
5. **排列大小。** 显式保留 `permutation_size`，以便调度程序永远不会从 2 的隐式幂推断填充规则。

### 复制和验证
- 工具：`cargo run --manifest-path scripts/fastpq/Cargo.toml --bin poseidon_gen -- domain-roots` 发出 Rust 片段或 Markdown 表（参见 `--format table`、`--seed`、`--filter`）。【scripts/fastpq/src/bin/poseidon_gen.rs:1】
- 测试：`canonical_sets_meet_security_target` 使规范参数集与已发布的常量保持一致（非零根、放大/元数配对、排列大小），因此 `cargo test -p fastpq_isi` 立即捕获漂移。【crates/fastpq_isi/src/params.rs:138】
- 事实来源：每当引入新参数包时，都会更新 Stage0 表和 `fastpq_isi/src/params.rs`。

## 附录 C — 承诺渠道详细信息### Streaming Poseidon 承诺流程
Stage2 定义了证明者和验证者共享的确定性跟踪承诺：
1. **标准化转换。** `trace::build_trace` 对每个批次进行排序，将其填充到 `N_trace = 2^{⌈log₂ rows⌉}`，并按以下顺序发出列向量。【crates/fastpq_prover/src/trace.rs:123】
2. **哈希列。** `trace::column_hashes` 通过标记为 `fastpq:v1:trace:column:<name>` 的专用 Poseidon2 海绵对列进行流式传输。当 `fastpq-prover-preview` 功能处于活动状态时，相同的遍历会回收后端所需的 IFFT/LDE 系数，因此不会分配额外的矩阵副本。【crates/fastpq_prover/src/trace.rs:474】
3. **提升到 Merkle 树中。** `trace::merkle_root` 将列摘要与标记为 `fastpq:v1:trace:node` 的 Poseidon 节点折叠，每当级别具有奇数扇出时复制最后一个叶子以避免特殊情况。【crates/fastpq_prover/src/trace.rs:656】
4. **完成摘要。** `digest::trace_commitment` 使用相同的 `[len, limbs…]` 编码为域标记 (`fastpq:v1:trace_commitment`)、参数名称、填充维度、列摘要和 Merkle 根添加前缀，然后使用 SHA3-256 对有效负载进行哈希处理，然后将其嵌入`Proof::trace_commitment`.【crates/fastpq_prover/src/digest.rs:25】

验证者在对 Fiat-Shamir 挑战进行采样之前重新计算相同的摘要，因此不匹配会在任何空缺之前中止证明。

### Poseidon 后备控制- 证明者现在公开了专用的 Poseidon 管道覆盖（`zk.fastpq.poseidon_mode`、env `FASTPQ_POSEIDON_MODE`、CLI `--fastpq-poseidon-mode`），因此操作员可以在无法达到 Stage7 <900ms 目标的设备上将 GPU FFT/LDE 与 CPU Poseidon 哈希混合。支持的值镜像执行模式旋钮（`auto`、`cpu`、`gpu`），未指定时默认为全局模式。运行时通过通道配置 (`FastpqPoseidonMode`) 将此值线程化，并将其传播到证明者 (`Prover::canonical_with_modes`)，因此覆盖在配置中具有确定性和可审核性dumps.【crates/iroha_config/src/parameters/user.rs:1488】【crates/fastpq_prover/src/proof.rs:138】【crates/iroha_core/src/fastpq/lane.rs:123】
- 遥测通过新的 `fastpq_poseidon_pipeline_total{requested,resolved,path,device_class,chip_family,gpu_kind}` 计数器（和 OTLP twin `fastpq.poseidon_pipeline_resolutions_total`）导出解析的管道模式。因此，`sorafs`/操作员仪表板可以确认部署何时运行 GPU 融合/流水线哈希与强制 CPU 回退 (`path="cpu_forced"`) 或运行时降级 (`path="cpu_fallback"`)。 CLI 探针会自动安装在 `irohad` 中，因此发布包和实时遥测共享相同的证据流。【crates/iroha_telemetry/src/metrics.rs:4780】【crates/irohad/src/main.rs:2504】
- 混合模式证据也通过现有的采用门被标记到每个记分板上：证明者为每个批次发出已解析的模式+路径标签，并且每当证明落地时，`fastpq_poseidon_pipeline_total` 计数器与执行模式计数器一起递增。这通过使停电可见并在优化继续的同时为确定性降级提供干净的开关来满足 WP2-E.6。【crates/fastpq_prover/src/trace.rs:1684】【docs/source/sorafs_orchestrator_rollout.md:139】
- `scripts/fastpq/wrap_benchmark.py --poseidon-metrics metrics_poseidon.prom` 现在解析 Prometheus 刮擦（金属或 CUDA），并将 `poseidon_metrics` 摘要嵌入每个打包的包中。帮助程序按 `metadata.labels.device_class` 过滤计数器行，捕获匹配的 `fastpq_execution_mode_total` 样本，并在 `fastpq_poseidon_pipeline_total` 条目丢失时使包装失败，因此 WP2-E.6 捆绑包始终提供可重现的 CUDA/Metal 证据，而不是临时证据笔记.【scripts/fastpq/wrap_benchmark.py:1】【scripts/fastpq/tests/test_wrap_benchmark.py:1】

#### 确定性混合模式策略 (WP2-E.6)1. **检测 GPU 不足。** 标记 Stage7 捕获或实时 Grafana 快照显示 Poseidon 延迟使总证明时间 >900ms 而 FFT/LDE 保持低于目标的任何设备类。操作员注释捕获矩阵（`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`），并在 `fastpq_poseidon_pipeline_total{device_class="<label>",path="gpu"}` 停滞时分页待命，而 `fastpq_execution_mode_total{backend="metal"}` 仍记录 GPU FFT/LDE 调度。【scripts/fastpq/wrap_benchmark.py:1】【dashboards/grafana/fastpq_acceleration.json:1】
2. **仅针对受影响的主机翻转到 CPU Poseidon。** 在主机本地配置中与队列标签一起设置 `zk.fastpq.poseidon_mode = "cpu"`（或 `FASTPQ_POSEIDON_MODE=cpu`），保留 `zk.fastpq.execution_mode = "gpu"`，以便 FFT/LDE 继续使用加速器。在部署票证中记录配置差异，并将每个主机的覆盖添加到捆绑包中，作为 `poseidon_fallback.patch`，以便审阅者可以确定地重播更改。
3. **证明降级。** 重启节点后立即刮掉 Poseidon 计数器：
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"'
   ```
   转储必须显示 `path="cpu_forced"` 与 GPU 执行计数器同步增长。将抓取的内容存储为 `metrics_poseidon.prom` ，位于现有 `metrics_cpu_fallback.prom` 快照旁边，并捕获 `poseidon_fallback.log` 中匹配的 `telemetry::fastpq.poseidon` 日志行。
4. **监控并退出。** 在优化工作继续进行的同时，保持对 `fastpq_poseidon_pipeline_total{path="cpu_forced"}` 的警报。一旦补丁使测试主机上的每个证明运行时间恢复到 900 毫秒以下，请将配置回滚到 `auto`，重新运行抓取（再次显示 `path="gpu"`），并将之前/之后的指标附加到捆绑包以关闭混合模式演练。

**遥测合同。**

|信号| PromQL / 来源 |目的|
|--------------------|-----------------|---------|
|波塞冬模式计数器 | `fastpq_poseidon_pipeline_total{device_class="<label>",path=~"cpu_.*"}` |确认 CPU 哈希是有意的且范围仅限于标记的设备类。 |
|执行模式计数器 | `fastpq_execution_mode_total{device_class="<label>",backend="metal"}` |证明即使 Poseidon 降级，FFT/LDE 仍然可以在 GPU 上运行。 |
|记录证据| `poseidon_fallback.log` 中捕获的 `telemetry::fastpq.poseidon` 条目 |提供主机通过 Reason `cpu_forced` 解析为 CPU 散列的每个证明。 |

推出捆绑包现在必须包括 `metrics_poseidon.prom`、配置差异以及混合模式处于活动状态时的日志摘录，以便治理可以与 FFT/LDE 遥测一起审核确定性后备策略。 `ci/check_fastpq_rollout.sh` 已强制执行队列/零填充限制；一旦混合模式进入自动化发布阶段，后续的门将会对 Poseidon 计数器进行健全性检查。

Stage7 捕获工具已经可以处理 CUDA：用 `--poseidon-metrics` 包装每个 `fastpq_cuda_bench` 包（指向已删除的 `metrics_poseidon.prom`），并且输出现在带有与 Metal 上使用的相同的管道计数器/分辨率摘要，因此治理可以在无需定制工具的情况下验证 CUDA 回退。【scripts/fastpq/wrap_benchmark.py:1】### 列顺序
哈希管道按以下确定顺序消耗列：
1. 选择器标志：`s_active`、`s_transfer`、`s_mint`、`s_burn`、`s_role_grant`、`s_role_revoke`、`s_meta_set`、 `s_perm`。
2. 压缩肢体列（每个零填充至迹线长度）：`key_limb_{i}`、`value_old_limb_{i}`、`value_new_limb_{i}`、`asset_id_limb_{i}`。
3. 辅助标量：`delta`、`running_asset_delta`、`metadata_hash`、`supply_counter`、`perm_hash`、`neighbour_leaf`、`dsid`、 `slot`。
4. 每个级别 `ℓ ∈ [0, SMT_HEIGHT)` 的稀疏 Merkle 见证人：`path_bit_ℓ`、`sibling_ℓ`、`node_in_ℓ`、`node_out_ℓ`。

`trace::column_hashes` 完全按照这个顺序遍历列，因此占位符后端和 Stage2 STARK 实现在各个版本中保持跟踪稳定。【crates/fastpq_prover/src/trace.rs:474】

### 转录域标签
Stage2 修复了下面的 Fiat-Shamir 目录，以保持挑战生成的确定性：

|标签 |目的|
| --- | -------- |
| `fastpq:v1:init` | Absorb 协议版本、参数集和 `PublicIO`。 |
| `fastpq:v1:roots` |提交追踪并查找 Merkle 根。 |
| `fastpq:v1:gamma` |示例查找盛大产品挑战。 |
| `fastpq:v1:alpha:<i>` |示例组合多项式挑战 (`i = 0, 1`)。 |
| `fastpq:v1:lookup:product` |吸收评估的查找宏积。 |
| `fastpq:v1:beta:<round>` |尝试 FRI 每轮的折叠挑战。 |
| `fastpq:v1:fri_layer:<round>` |提交每个 FRI 层的 Merkle 根。 |
| `fastpq:v1:fri:final` |在打开查询之前记录最终的 FRI 层。 |
| `fastpq:v1:query_index:0` |确定性地导出验证者查询索引。 |