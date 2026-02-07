---
id: nexus-spec
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Nexus technical specification
description: Full mirror of `docs/source/nexus.md`, covering the architecture and design constraints for the Iroha 3 (Sora Nexus) ledger.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
此页面镜像 `docs/source/nexus.md`。保持两个副本对齐，直到翻译待办事项到达门户。
:::

#！ Iroha 3 – Sora Nexus Ledger：技术设计规范

本文档提出了 Iroha 3 的 Sora Nexus 账本架构，将 Iroha 2 发展为围绕数据空间 (DS) 组织的单一全球逻辑统一账本。数据空间提供强大的隐私域（“私有数据空间”）和开放参与（“公共数据空间”）。该设计保留了全球账本的可组合性，同时确保私有 DS 数据的严格隔离和机密性，并通过跨 Kura（块存储）和 WSV（世界状态视图）的纠删码引入了数据可用性扩展。

同一存储库构建 Iroha 2（自托管网络）和 Iroha 3 (SORA Nexus)。执行由
共享 Iroha 虚拟机 (IVM) 和 Kotodama 工具链，因此合约和字节码工件仍然存在
可跨自托管部署和 Nexus 全局分类账移植。

目标
- 由许多协作验证器和数据空间组成的一个全局逻辑分类账。
- 用于许可操作的私有数据空间（例如，CBDC），数据永远不会离开私有 DS。
- 开放参与的公共数据空间，类似以太坊的免许可访问。
- 跨数据空间的可组合智能合约，需要获得访问私有 DS 资产的明确权限。
- 性能隔离，因此公共活动不会降低私有 DS 内部事务的性能。
- 大规模数据可用性：纠删码 Kura 和 WSV 可有效支持无限数据，同时保持私有 DS 数据的私密性。

非目标（初始阶段）
- 定义代币经济学或验证者激励措施；调度和质押策略是可插入的。
- 引入新的 ABI 版本；根据 IVM 策略，使用显式系统调用和指针 ABI 扩展更改目标 ABI v1。

术语
- Nexus 账本：通过将数据空间（DS）块组成单个有序历史和状态承诺而形成的全局逻辑账本。
- 数据空间（DS）：一个有界的执行和存储域，具有自己的验证器、治理、隐私类别、DA 策略、配额和费用策略。存在两个类别：公共 DS 和私有 DS。
- 私有数据空间：许可的验证器和访问控制；交易数据和状态永远不会离开 DS。只有承诺/元数据是全球锚定的。
- 公共数据空间：无需许可的参与；完整的数据和状态是公开的。
- 数据空间清单（DS 清单）：声明 DS 参数（验证器/QC 密钥、隐私类别、ISI 策略、DA 参数、保留、配额、ZK 策略、费用）的 Norito 编码清单。清单哈希锚定在 Nexus 链上。除非被覆盖，否则 DS 仲裁证书使用 ML-DSA-87（Dilithium5 级）作为默认后量子签名方案。
- 空间目录：一个全球链上目录合约，用于跟踪 DS 清单、版本和治理/轮换事件，以实现可解决性和审计。
- DSID：数据空间的全局唯一标识符。用于命名所有对象和引用。
- 锚点：包含在 Nexus 链中的 DS 块/标头的加密承诺，用于将 DS 历史记录绑定到全局分类账中。
- Kura：Iroha 块存储。此处通过纠删码 blob 存储和承诺进行扩展。
- WSV：Iroha 世界状态视图。此处使用版本化、支持快照、纠删码的状态段进行扩展。
- IVM：用于智能合约执行的 Iroha 虚拟机（Kotodama 字节码 `.to`）。
 - AIR：代数中间表示。 STARK 式证明的计算代数视图，将执行描述为具有转换和边界约束的基于字段的跟踪。

数据空间模型
- 身份：`DataSpaceId (DSID)` 标识 DS 并命名所有内容。 DS 可以以两种粒度进行实例化：
  - 域 DS：`ds::domain::<domain_name>` — 执行和状态范围仅限于域。
  - 资产 DS：`ds::asset::<domain_name>::<asset_name>` — 执行和状态范围仅限于单个资产定义。
  两种形式并存；事务可以自动触及多个 DSID。
- 清单生命周期：DS 创建、更新（密钥轮换、策略更改）和停用均记录在空间目录中。每个插槽 DS 工件都会引用最新的清单哈希。
- 类别：公共 DS（开放参与、公共 DA）和私人 DS（许可、机密 DA）。通过清单标志可以实现混合策略。
- 每个 DS 的策略：ISI 权限、DA 参数 `(k,m)`、加密、保留、配额（每个块的最小/最大 tx 份额）、ZK/乐观证明策略、费用。
- 治理：DS 成员资格和验证者轮换由清单的治理部分定义（链上提案、多重签名或由 Nexus 交易和证明锚定的外部治理）。

能力清单和 UAID
- 通用帐户：每个参与者都会收到一个跨越所有数据空间的确定性 UAID（`crates/iroha_data_model/src/nexus/manifest.rs` 中的 `UniversalAccountId`）。功能清单 (`AssetPermissionManifest`) 将 UAID 绑定到特定数据空间、激活/到期时期以及范围为 `dataspace`、`program_id`、`method` 的允许/拒绝 `ManifestEntry` 规则的有序列表`asset`，以及可选的 AMX 角色。拒绝规则总是胜利；评估者发出带有审核原因的 `ManifestVerdict::Denied` 或带有匹配津贴元数据的 `Allowed` 拨款。
- 限额：每个允许条目携带确定性 `AllowanceWindow` 存储桶（`PerSlot`、`PerMinute`、`PerDay`）以及可选的 `max_amount`。主机和 SDK 使用相同的 Norito 有效负载，因此跨硬件和 SDK 实现的实施保持相同。
- 审核遥测：只要清单更改状态，空间目录就会广播 `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`)。新的 `SpaceDirectoryEventFilter` 表面允许 Torii/数据事件订阅者监控 UAID 清单更新、撤销和拒绝胜利决策，而无需自定义管道。

有关端到端操作员证据、SDK 迁移说明和清单发布清单，请将此部分与通用帐户指南 (`docs/source/universal_accounts_guide.md`) 进行镜像。每当 UAID 策略或工具发生变化时，请保持两个文档保持一致。

高层架构
1）全局组合层（Nexus链）
- 维护 1 秒 Nexus 块的单一规范排序，以完成跨越一个或多个数据空间 (DS) 的原子事务。每个提交的事务都会更新统一的全局世界状态（每个 DS 根的向量）。
- 包含最少的元数据加上聚合证明/QC，以确保可组合性、最终性和欺诈检测（涉及的 DSID、之前/之后的每个 DS 状态根、DA 承诺、每个 DS 有效性证明以及使用 ML-DSA-87 的 DS 仲裁证书）。不包含私人数据。
- 共识：规模为 22 人（3f+1，f=7）的单一全球流水线 BFT 委员会，通过划时代的 VRF/权益机制从多达 20 万潜在验证者池中选出。 Nexus 委员会对交易进行排序并在 1 秒内完成区块。

2）数据空间层（公共/私有）
- 执行全局事务的每 DS 片段，更新 DS 本地 WSV，并生成汇总到 1 秒 Nexus 块中的每块有效性工件（聚合的每 DS 证明和 DA 承诺）。
- 私有 DS 对授权验证者之间的静态数据和动态数据进行加密；只有承诺和 PQ 有效性证明离开 DS。
- 公共 DS 导出完整数据体（通过 DA）和 PQ 有效性证明。

3) 原子跨数据空间事务 (AMX)
- 模型：每一笔用户交易可能会涉及多个 DS（例如，域 DS 和一个或多个资产 DS）。它在单个 Nexus 块中原子提交或中止；没有部分影响。
- 1 秒内准备提交：对于每个候选交易，触及的 DS 针对同一快照（时隙起始 DS 根）并行执行，并生成每个 DS PQ 有效性证明 (FASTPQ-ISI) 和 DA 承诺。仅当所有必需的 DS 证明都经过验证并且 DA 证书到达（≤300 毫秒目标）时，Nexus 委员会才会提交交易；否则，交易将重新安排到下一个时隙。
- 一致性：声明读写集；冲突检测发生在针对槽开始根的提交时。每个 DS 的无锁乐观执行避免了全局停顿；原子性由 Nexus 提交规则强制执行（DS 中的全有或全无）。
- 隐私：私人 DS 仅导出与前/后 DS 根相关的证明/承诺。没有原始私人数据离开 DS。4) 具有纠删码的数据可用性 (DA)
- Kura 将块体和 WSV 快照存储为纠删码 blob。公共 blob 被广泛分片；私有 blob 仅存储在私有 DS 验证器中，并带有加密块。
- DA 承诺记录在 DS 工件和 Nexus 块中，从而在不泄露私人内容的情况下实现采样和恢复保证。

块和提交结构
- 数据空间证明神器（每 1s 插槽，每 DS）
  - 字段：dsid、slot、pre_state_root、post_state_root、ds_tx_set_hash、kura_da_commitment、wsv_da_commitment、manifest_hash、ds_qc (ML-DSA-87)、ds_validity_proof (FASTPQ-ISI)。
  - 没有数据体的私有 DS 导出工件；公共 DS 允许通过 DA 检索尸体。

- Nexus 块（1s 节奏）
  - 字段：block_number、parent_hash、slot_time、tx_list（涉及 DSID 的原子跨 DS 交易）、ds_artifacts[]、nexus_qc。
  - 功能：完成所有需要 DS 工件验证的原子事务；一步更新 DS 根的全局世界状态向量。

共识与调度
- Nexus 链共识：单一全局、流水线 BFT（Sumeragi 级），具有 22 节点委员会（3f+1，f=7），目标是 1s 区块和 1s 最终性。委员会成员是通过 VRF/股权从约 20 万名候选人中划时代选出的；轮换可以维持权力下放和抗审查性。
- 数据空间共识：每个 DS 在其验证器中运行自己的 BFT，以生成每个时隙的工件（证明、DA 承诺、DS QC）。通道中继委员会的大小使用数据空间 `fault_tolerance` 设置为 `3f+1`，并使用与 `(dataspace_id, lane_id)` 绑定的 VRF 纪元种子从数据空间验证器池中按纪元确定性地进行采样。私人 DS 已获得许可；公共 DS 允许开放活动，但须遵守反女巫政策。全球联系委员会保持不变。
- 事务调度：用户提交声明触及的 DSID 和读写集的原子事务。 DS在时隙内并行执行；如果所有 DS 工件都经过验证并且 DA 证书都是及时的（≤300 毫秒），那么 Nexus 委员会会将交易包含在 1s 区块中。
- 性能隔离：每个 DS 都有独立的内存池和执行。每个 DS 配额限制了每个块可以提交的涉及给定 DS 的事务数量，以避免队头阻塞并保护私有 DS 延迟。

数据模型和命名空间
- DS 合格 ID：所有实体（域、帐户、资产、角色）均符合 `dsid` 资格。示例：`ds::<domain>::account`、`ds::<domain>::asset#precision`。
- 全局引用：全局引用是一个元组 `(dsid, object_id, version_hint)`，可以放置在链上的 Nexus 层或 AMX 描述符中以供跨 DS 使用。
- Norito 序列化：所有跨 DS 消息（AMX 描述符、证明）均使用 Norito 编解码器。生产路径中没有使用 serde。

智能合约和 IVM 扩展
- 执行上下文：将 `dsid` 添加到 IVM 执行上下文。 Kotodama 合约始终在特定数据空间内执行。
- 原子交叉 DS 基元：
  - `amx_begin()` / `amx_commit()` 在 IVM 主机中划分原子多 DS 事务。
  - `amx_touch(dsid, key)` 声明针对插槽快照根的冲突检测的读/写意图。
  - `verify_space_proof(dsid, proof, statement)` → 布尔
  - `use_asset_handle(handle, op, amount)` → 结果（仅当策略允许且句柄有效时才允许操作）
- 资产处理和费用：
  - 资产操作由DS的ISI/角色策略授权；费用以 DS 的 Gas 代币支付。稍后可以添加可选的功能令牌和更丰富的策略（多审批者、速率限制、地理围栏），而无需更改原子模型。
- 确定性：所有新的系统调用都是纯粹且确定性的给定输入和声明的 AMX 读/写集。没有隐藏的时间或环境影响。

后量子有效性证明（广义 ISI）
- FASTPQ‑ISI（PQ，无可信设置）：一种基于哈希的内核化参数，将传输设计推广到所有 ISI 系列，同时针对 GPU 级硬件上的 20k 规模批次进行亚秒级验证。
  - 运营概况：
    - 生产节点通过 `fastpq_prover::Prover::canonical` 构建证明器，现在始终初始化生产后端；确定性模拟已被删除。【crates/fastpq_prover/src/proof.rs:126】
    - `zk.fastpq.execution_mode`（配置）和 `irohad --fastpq-execution-mode` 允许操作员确定性地固定 CPU/GPU 执行，同时观察者挂钩记录队列的请求/解析/后端三元组审核。【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:2192】【crates/iroha_telemetry/src/metrics.rs:8887】
- 算术化：
  - KV-Update AIR：将 WSV 视为通过 Poseidon2-SMT 提交的类型化键值映射。每个 ISI 都扩展到密钥（帐户、资产、角色、域、元数据、供应）上的一小组读取-检查-写入行。
  - 操作码门控约束：带有选择器列的单个 AIR 表强制执行每个 ISI 规则（保护、单调计数器、权限、范围检查、有界元数据更新）。
  - 查找参数：权限/角色、资产精度和策略参数的透明哈希提交表避免了严重的按位约束。
- 国家承诺和更新：
  - 聚合 SMT 证明：所有触摸键（前/后）均通过使用具有重复数据删除同级的压缩前沿来针对 `old_root`/`new_root` 进行验证。
  - 不变量：全局不变量（例如，每项资产的总供应量）通过效应行和跟踪计数器之间的多重集相等性来强制执行。
- 证明系统：
  - FRI 式多项式承诺 (DEEP-FRI)，具有高数量 (8/16) 和放大 8-16； Poseidon2 哈希；带有 SHA-2/3 的 Fiat-Shamir 成绩单。
  - 可选递归：DS 本地递归聚合，可根据需要将微批次压缩为每个槽一个证明。
- 涵盖的范围和示例：
  - 资产：转移、铸造、销毁、注册/注销资产定义、设置精度（有界）、设置元数据。
  - 帐户/域：创建/删除、设置密钥/阈值、添加/删除签名者（仅限状态；签名检查由 DS 验证器证明，不在 AIR 内部证明）。
  - 角色/权限（ISI）：授予/撤销角色和权限；通过查找表和单调策略检查来强制执行。
  - 合同/AMX：AMX 开始/提交标记，能力铸造/撤销（如果启用）；被证明是状态转换和政策计数器。
- Out-of-AIR 检查以保持延迟：
  - 签名和重加密（例如 ML-DSA 用户签名）由 DS 验证器验证并在 DS QC 中进行证明；有效性证明仅涵盖状态一致性和政策合规性。这可以保持证明的 PQ 和速度。
- 性能目标（示例性 32 核 CPU + 单个现代 GPU）：
  - 具有小按键触摸的 20k 混合 ISI（≤8 个按键/ISI）：〜0.4–0.9 秒验证，〜150–450 KB 验证，〜5–15 毫秒验证。
  - 更重的 ISI（更多键/丰富约束）：微批量（例如 10×2k）+ 递归以保持每个时隙 <1 s。
- DS 清单配置：
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`、`zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false`（签名由 DS QC 验证）
  - `attestation.qc_signature = "ml_dsa_87"`（默认值；必须明确声明替代方案）
- 后备方案：
  - 复杂/自定义 ISI 可以使用通用 STARK (`zk.policy = "stark_fri_general"`)，通过 QC 证明+对无效证明进行削减，实现延迟证明和 1 秒最终确定。
  - 非 PQ 选项（例如，带有 KZG 的 Plonk）需要可信设置，默认版本不再支持。

AIR 底漆（适用于 Nexus）
- 执行跟踪：具有宽度（寄存器列）和长度（步长）的矩阵。每一行都是 ISI 处理的一个逻辑步骤；列保存前/后值、选择器和标志。
- 限制：
  - 转换约束：强制执行行到行关系（例如，post_balance = pre_balance − `sel_transfer = 1` 时借方行的金额）。
  - 边界约束：将公共 I/O（old_root/new_root、计数器）绑定到第一行/最后一行。
  - 查找/排列：确保针对提交表（权限、资产参数）的成员资格和多重集平等，而无需使用大量位电路。
- 承诺与验证：
  - 证明者通过基于散列的编码提交跟踪，并构造有效的 iff 约束成立的低次多项式。
  - 验证者通过 FRI（基于哈希的后量子）和一些 Merkle 开口检查低度；成本是步长的对数。
- 示例（转账）：寄存器包括 pre_balance、amount、post_balance、nonce 和选择器。约束强制执行非负/范围、守恒和随机数单调性，而聚合的 SMT 多重证明将前/后叶子链接到旧/新根。ABI 和系统调用演变 (ABI v1)
- 要添加的系统调用（说明性名称）：
  - `SYS_AMX_BEGIN`、`SYS_AMX_TOUCH`、`SYS_AMX_COMMIT`、`SYS_VERIFY_SPACE_PROOF`、`SYS_USE_ASSET_HANDLE`。
- 要添加的指针 ABI 类型：
  - `PointerType::DataSpaceId`、`PointerType::AmxDescriptor`、`PointerType::AssetHandle`、`PointerType::ProofBlob`。
- 所需更新：
  - 添加到 `ivm::syscalls::abi_syscall_list()`（保留订购），按策略进行控制。
  - 将未知号码映射到主机中的 `VMError::UnknownSyscall`。
  - 更新测试：系统调用列表黄金、ABI 哈希、指针类型 ID 黄金和策略测试。
  - 文档：`crates/ivm/docs/syscalls.md`、`status.md`、`roadmap.md`。

隐私模型
- 私有数据包含：私有 DS 的交易主体、状态差异和 WSV 快照永远不会离开私有验证器子集。
- 公开曝光：仅导出标头、DA 承诺和 PQ 有效性证明。
- 可选的 ZK 证明：私有 DS 可以生成 ZK 证明（例如，余额充足、满足策略），从而能够在不泄露内部状态的情况下实现跨 DS 操作。
- 访问控制：授权由 DS 内的 ISI/角色策略强制执行。能力令牌是可选的，可以在需要时稍后引入。

性能隔离和 QoS
- 每个 DS 具有独立的共识、内存池和存储。
- Nexus 每个 DS 的调度配额，以限制锚点包含时间并避免队头阻塞。
- 每个 DS 的合同资源预算（计算/内存/IO），由 IVM 主机强制执行。公共 DS 争用不能消耗私人 DS 预算。
- 异步跨 DS 调用避免了私有 DS 执行中的长时间同步等待。

数据可用性和存储设计
1) 纠删码
- 使用系统化的 Reed-Solomon（例如 GF(2^16)）对 Kura 块和 WSV 快照进行 blob 级擦除编码：参数 `(k, m)` 和 `n = k + m` 分片。
- 默认参数（建议，公共 DS）：`k=32, m=16` (n=48)，能够通过约 1.5 倍的扩展从最多 16 个分片丢失中恢复。对于私有 DS：许可集中的 `k=16, m=8` (n=24)。两者均可根据 DS Manifest 进行配置。
- 公共 Blob：分布在许多 DA 节点/验证器上的分片，具有基于采样的可用性检查。标头中的 DA 承诺允许轻客户端进行验证。
- 私有 Blob：仅在私有 DS 验证器（或指定托管人）内加密和分发的分片。全球链仅承载 DA 承诺（没有分片位置或密钥）。

2) 承诺和抽样
- 对于每个 blob：计算分片上的 Merkle 根并将其包含在 `*_da_commitment` 中。通过避免椭圆曲线承诺来保持 PQ。
- DA 证明者：VRF 抽样的区域证明者（例如，每个区域 64 名）颁发 ML-DSA-87 证书，证明分片抽样成功。目标 DA 证明延迟 ≤300 毫秒。 Nexus 委员会验证证书而不是拉取分片。

3) 库拉整合
- 区块将交易主体存储为带有 Merkle 承诺的纠删码 blob。
- 标头带有 blob 承诺；公共 DS 的主体可通过 DA 网络检索，私有 DS 的主体可通过专用通道检索。

4) WSV 集成
- WSV 快照：定期将 DS 状态检查点到分块、纠删码快照中，并在标头中记录承诺。在快照之间，维护更改日志。公共快照被广泛分片；私有快照保留在私有验证器中。
- 携带证明的访问：合约可以提供（或请求）由快照承诺锚定的状态证明（Merkle/Verkle）。私人 DS 可以提供零知识证明而不是原始证明。

5）保留和修剪
- 公共 DS 无需修剪：通过 DA（水平缩放）保留所有 Kura 体和 WSV 快照。私人 DS 可以定义内部保留，但导出的承诺仍然不可变。 Nexus 层保留所有 Nexus 块和 DS 工件承诺。

网络和节点角色
- 全球验证者：参与nexus共识，验证Nexus区块和DS工件，对公共DS执行DA检查。
- 数据空间验证器：运行 DS 共识、执行合约、管理本地 Kura/WSV、为其 DS 处理 DA。
- DA 节点（可选）：存储/公开公共 blob，方便采样。对于私有 DS，DA 节点与验证者或受信任的托管人位于同一地点。

系统级改进和注意事项
- 排序/内存池解耦：采用 DAG 内存池（例如，Narwhal 风格）在连接层提供流水线 BFT，以在不更改逻辑模型的情况下降低延迟并提高吞吐量。
- DS 配额和公平性：每个 DS 每块配额和权重上限，以避免队头阻塞并确保私有 DS 的可预测延迟。
- DS 证明 (PQ)：默认 DS 仲裁证书使用 ML-DSA-87（Dilithium5 级）。这是后量子的，比 EC 签名更大，但在每个时隙一个 QC 上是可以接受的。 DS 可以明确选择 ML-DSA-65/44（较小）或 EC 签名（如果在 DS 清单中声明）；强烈鼓励公众 DS 保留 ML-DSA-87。
- DA 证明者：对于公共 DS，使用颁发 DA 证书的 VRF 抽样区域证明者。 Nexus 委员会验证证书而不是原始分片采样；私有 DS 将 DA 证明保留在内部。
- 递归和纪元证明：可以选择将 DS 中的多个微批次聚合为每个时隙/纪元的一个递归证明，以保持证明大小并在高负载下验证时间稳定。
- 通道扩展（如果需要）：如果单个全局委员会成为瓶颈，则引入具有确定性合并的 K 个并行排序通道。这在水平扩展的同时保留了单一的全局顺序。
- 确定性加速：为散列/FFT 提供 SIMD/CUDA 功能门控内核，并具有位精确的 CPU 回退，以保持跨硬件确定性。
- 通道激活阈值（建议）：如果 (a) p95 最终确定性超过 1.2 秒持续超过 3 分钟，或 (b) 每个区块占用率超过 85% 持续超过 5 分钟，或 (c) 传入交易速率在持续水平下需要 >1.2 倍的区块容量，则启用 2-4 个通道。通道通过 DSID 哈希确定性地存储交易并合并到连接块中。

费用和经济（初始默认）
- Gas 单位：带有计量计算/IO 的 per‑DS Gas 代币；费用通过 DS 的原生天然气资产支付。跨 DS 的转换是一个应用程序问题。
- 纳入优先：跨 DS 的循环赛，每个 DS 配额以保持公平性和 1 秒 SLO；在 DS 内，费用竞价可以打破平局。
- 未来：可以在不改变原子性或 PQ 证明设计的情况下探索可选的全球费用市场或 MEV 最小化政策。

跨数据空间工作流程（示例）
1) 用户提交涉及公共 DSP 和私有 DS S 的 AMX 交易：将资产 X 从 S 转移到账户位于 P 的受益人 B。
2) 在槽内，P 和 S 各自针对槽快照执行其片段。 S验证授权和可用性，更新其内部状态，并产生PQ有效性证明和DA承诺（不泄露私人数据）。 P准备相应的状态更新（例如根据策略在P中铸造/销毁/锁定）及其证明。
3）Nexus委员会验证DS证明和DA证书；如果两者都在槽内验证，则事务在 1s Nexus 块中以原子方式提交，更新全局世界状态向量中的两个 DS 根。
4) 如果任何证明或 DA 证书丢失/无效，交易将中止（无影响），并且客户端可以重新提交下一个时段。在任何步骤中都没有私人数据离开 S。

- 安全考虑
- 确定性执行：IVM 系统调用保持确定性；跨 DS 结果由 AMX 提交和最终性驱动，而不是挂钟或网络计时。
- 访问控制：私有 DS 中的 ISI 权限限制谁可以提交交易以及允许执行哪些操作。能力令牌对跨 DS 使用的细粒度权限进行编码。
- 保密性：私有 DS 数据的端到端加密、仅在授权成员之间存储的纠删码分片、用于外部证明的可选 ZK 证明。
- DoS 抵抗：内存池/共识/存储层的隔离可防止公共拥塞影响私有 DS 进程。Iroha 组件的更改
- iroha_data_model：引入 `DataSpaceId`、DS 限定标识符、AMX 描述符（读/写集）、证明/DA 承诺类型。仅 Norito 序列化。
- ivm：为 AMX 添加系统调用和指针 ABI 类型（`amx_begin`、`amx_commit`、`amx_touch`）和 DA 证明；根据 v1 策略更新 ABI 测试/文档。
- iroha_core：实现 Nexus 调度程序、空间目录、AMX 路由/验证、DS 工件验证以及 DA 采样和配额的策略实施。
- 空间目录和清单加载器：通过 DS 清单解析线程 FMS 端点元数据（和其他公共服务描述符），以便节点在加入数据空间时自动发现本地服务端点。
- kura：具有纠删码、承诺、尊重私人/公共政策的检索 API 的 Blob 存储。
- WSV：快照、分块、承诺；证明 API；与 AMX 冲突检测和验证集成。
- irohad：节点角色、DA 网络、私有 DS 成员资格/身份验证、通过 `iroha_config` 进行配置（生产路径中没有环境切换）。

配置和确定性
- 所有运行时行为均通过 `iroha_config` 配置并通过构造函数/主机进行线程化。没有生产环境切换。
- 硬件加速（SIMD/NEON/METAL/CUDA）是可选的并且具有功能门控；确定性回退必须在硬件上产生相同的结果。
 - 后量子默认：默认情况下，所有 DS 必须使用 PQ 有效性证明 (STARK/FRI) 和 ML-DSA-87 进行 DS QC。替代方案需要明确的 DS 清单声明和策略批准。

迁移路径 (Iroha 2 → Iroha 3)
1）在数据模型中引入数据空间限定的ID和关系块/全局状态组合；添加功能标志以在转换期间保留 Iroha 2 旧模式。
2) 在功能标志后面实现 Kura/WSV 纠删码后端，在早期阶段将当前后端保留为默认值。
3) 添加 IVM 系统调用和 AMX（原子多 DS）操作的指针类型；扩展测试和文档；保留 ABI v1。
4) 提供具有单个公共 DS 和 1s 区块的最小 Nexus 链；然后仅添加第一个私人 DS 试点导出证明/承诺。
5) 通过 DS 本地 FASTPQ-ISI 证明和 DA 证明者扩展到完整的原子跨 DS 交易 (AMX)；跨 DS 启用 ML-DSA-87 QC。

测试策略
- 数据模型类型、Norito 往返、AMX 系统调用行为和证明编码/解码的单元测试。
- IVM 测试新的系统调用和 ABI 黄金。
- 原子跨 DS 事务（正/负）、DA 证明者延迟目标（≤300 毫秒）以及负载下的性能隔离的集成测试。
- DS QC 验证 (ML-DSA-87)、冲突检测/中止语义和机密碎片泄漏预防的安全测试。

### NX-18 遥测和运行手册资产

- **Grafana 板：** `dashboards/grafana/nexus_lanes.json` 现在导出 NX-18 请求的“Nexus Lane Finality & Oracles”仪表板。面板涵盖 `iroha_slot_duration_ms` 上的 `histogram_quantile()`、`iroha_da_quorum_ratio`、DA 可用性警告 (`sumeragi_da_gate_block_total{reason="missing_local_data"}`)、oracle 价格/陈旧性/TWAP/理发仪表以及实时 `iroha_settlement_buffer_xor` 缓冲面板，以便操作员可以证明 1s 插槽、DA 和没有定制查询的财务 SLO。
- **运行手册：** `docs/source/runbooks/nexus_lane_finality.md` 记录了仪表板附带的待命工作流程（阈值、事件步骤、证据捕获、混沌演习），实现了 NX-18 中的“发布操作员仪表板/运行​​手册”要点。
- **遥测助手：**重用现有的 `scripts/telemetry/compare_dashboards.py` 来比较导出的仪表板（防止分段/产品漂移），在 `ci/check_nexus_lane_smoke.sh` 内运行 `scripts/telemetry/nx18_acceptance.py --json-out artifacts/nx18/nx18_acceptance.json <metrics.prom>` 以门控 DA/quorum/oracle/buffer/slot SLO，并在路由跟踪或混沌排练期间调用 `scripts/telemetry/check_nexus_audit_outcome.py`因此，每台 NX‑18 钻头都会存档匹配的 `nexus.audit.outcome` 有效负载。

开放性问题（需要澄清）
1) 交易签名：决策——最终用户可以自由选择其目标 DS 宣传的任何签名算法（Ed25519、secp256k1、ML-DSA 等）。主机必须在清单中强制执行多重签名/曲线功能标志，提供确定性回退，并记录混合算法时的延迟影响。杰出：最终确定跨 Torii/SDK 的能力协商流程并更新准入测试。
2）Gas经济性：每个DS可以以本地代币计价Gas，而全球结算费用以SORA XOR支付。突出：定义标准转换路径（公共通道 DEX 与其他流动性来源）、账本会计挂钩以及补贴或零价格交易的 DS 保障措施。
3) DA 证明者：每个区域和阈值的目标数量（例如，64 个采样，64 个 ML-DSA-87 签名中的 43 个），以满足 ≤300 毫秒的要求，同时保持耐久性。我们从第一天起就必须包括哪些区域？
4）默认DA参数：我们建议公共DS `k=32, m=16`和私有DS `k=16, m=8`。您是否想要某些 DS 类别具有更高的冗余配置文件（例如 `k=30, m=20`）？
5）DS粒度：域名和资产都可以是DS。我们是否应该支持具有可选策略继承的分层 DS（域 DS 作为资产 DS 的父级），还是在 v1 中保持平坦？
6）重ISI：对于无法产生亚秒级证明的复杂ISI，我们应该（a）拒绝它们，（b）跨块分割成更小的原子步骤，还是（c）允许使用显式标志延迟包含？
7）跨DS冲突：客户端声明的读/写集是否足够，或者主机应该为了安全而自动推断和扩展它（以更多冲突为代价）？

附录：遵守存储库政策
- Norito 用于通过 Norito 帮助程序进行所有有线格式和 JSON 序列化。
- 仅 ABI v1； ABI 策略没有运行时切换。系统调用和指针类型的添加遵循记录的演化过程和黄金测试。
- 跨硬件保留确定性；加速是可选的并且是门控的。
- 生产路径中没有 serde；生产中没有基于环境的配置。