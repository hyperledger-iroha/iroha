---
lang: zh-hans
direction: ltr
source: docs/source/ivm_isi_kotodama_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3f40329b9968530dea38745b49f7fee4d55aeb461e515e6f97b5b5986cb27e3f
source_last_modified: "2026-01-21T19:17:13.238594+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM ⇄ ISI ⇄ 数据模型 ⇄ Kotodama — 对齐审核

本文档审核 Iroha 虚拟机 (IVM) 指令集和系统调用表面如何映射到 Iroha 特殊指令 (ISI) 和 `iroha_data_model`，以及 Kotodama 如何编译到该堆栈中。它确定了当前的差距并提出了具体的改进建议，使这四层能够确定性地、符合人体工程学地组合在一起。

关于字节码目标的注意事项：Kotodama 智能合约编译为 Iroha 虚拟机 (IVM) 字节码 (`.to`)。他们并不将“risc5”/RISC-V 作为独立架构。此处引用的任何 RISC-V 类编码都是 IVM 混合指令格式的一部分，并且仍然是实现细节。

## 范围和来源
- IVM：`crates/ivm/src/{instruction.rs,ivm.rs,syscalls.rs,host.rs,mock_wsv.rs}` 和 `crates/ivm/docs/*`。
- ISI/数据模型：`crates/iroha_data_model/src/isi/*`、`crates/iroha_core/src/smartcontracts/isi/*` 和文档 `docs/source/data_model_and_isi_spec.md`。
- Kotodama：`crates/kotodama_lang/src/*`，`crates/ivm/docs/*` 中的文档。
- 核心集成：`crates/iroha_core/src/{state.rs,executor.rs,smartcontracts/ivm/cache.rs}`。

术语
- “ISI”是指通过执行器改变世界状态的内置指令类型（例如，RegisterAccount、Mint、Transfer）。
- “系统调用”是指 IVM `SCALL`，具有 8 位数字，委托主机进行账本操作。

---

## 当前映射（已实现）

### IVM 说明
- 算术、内存、控制流、加密、向量和 ZK 助手在 `instruction.rs` 中定义，并在 `ivm.rs` 中实现。这些是独立的和确定性的；加速路径（SIMD/Metal/CUDA）具有 CPU 回退。
- 系统/主机边界通过 `SCALL`（操作码 0x60）。数字列在 `syscalls.rs` 中，包括世界操作（注册/取消注册域/帐户/资产、铸造/销毁/转移、角色/权限操作、触发器）以及帮助程序（`GET_PRIVATE_INPUT`、`COMMIT_OUTPUT`、`GET_MERKLE_PATH` 等）。

### 主机层
- 特征 `IVMHost::syscall(number, &mut IVM)` 存在于 `host.rs` 中。
- DefaultHost 仅实现非账本助手（分配、堆增长、输入/输出、ZK 证明助手、功能发现）——它不执行世界状态突变。
- `mock_wsv.rs` 中存在演示 `WsvHost`，它通过寄存器 x10..x13 中的临时整数→ID 映射，使用 `AccountId`/`AssetDefinitionId` 将资产操作子集（传输/铸造/燃烧）映射到小型内存中 WSV。

### ISI 和数据模型
- 内置 ISI 类型和语义在 `iroha_core::smartcontracts::isi::*` 中实现，并在 `docs/source/data_model_and_isi_spec.md` 中记录。
- `InstructionBox` 使用具有稳定“wire ID”和 Norito 编码的注册表；本机执行调度是核心中的当前代码路径。### IVM的核心集成
- `State::execute_trigger(..)` 克隆缓存的 `IVM`，附加 `CoreHost::with_accounts_and_args`，然后调用 `load_program` + `run`。
- `CoreHost` 实现 `IVMHost`：有状态系统调用通过指针 ABI TLV 布局进行解码、映射到内置 ISI (`InstructionBox`) 并排队。一旦虚拟机返回，主机将这些 ISI 交给常规执行程序，因此权限、不变量、事件和遥测与本机执行保持相同。不接触 WSV 的帮助程序系统调用仍委托给 `DefaultHost`。
- `executor.rs` 继续原生运行内置 ISI；将验证器执行器本身迁移到 IVM 仍然是未来的工作。

### Kotodama → IVM
- 存在前端部分（词法分析器/解析器/最小语义/IR/regalloc）。
- Codegen (`kotodama::compiler`) 发出 IVM 操作的子集并使用 `SCALL` 进行资产操作：
  - `MintAsset`→设置x10 =帐户，x11 =资产，x12 =＆NoritoBytes（数字）； `SCALL SYSCALL_MINT_ASSET`。
  - `BurnAsset`/`TransferAsset` 类似（作为 NoritoBytes（数字）指针传递的数量）。
- 演示 `koto_*_demo.rs` 展示了使用 `WsvHost` 以及映射到 ID 的整数索引以进行快速测试。

---

## 差距和不匹配

1) 核心主机覆盖率和奇偶校验
- 状态：`CoreHost` 现在存在于核心中，并将许多分类帐系统调用转换为通过标准路径执行的 ISI。覆盖范围仍然不完整（例如，某些角色/权限/触发系统调用是存根），并且需要奇偶校验测试来保证排队的 ISI 产生与本机执行相同的状态/事件。

2) 系统调用表面与 ISI/数据模型命名和覆盖范围
- NFT：系统调用现在公开与 `iroha_data_model::nft` 对齐的规范 `SYSCALL_NFT_*` 名称。
- 角色/权限/触发器：存在系统调用列表，但没有将每个调用与核心中的具体 ISI 联系起来的参考实现或映射表。
- 参数/语义：某些系统调用不指定参数编码（类型 ID 与指针）或气体语义； ISI 语义定义明确。

3) 用于跨虚拟机/主机边界传递类型化数据的 ABI
- 指针 ABI TLV 现在以 `CoreHost` (`decode_tlv_typed`) 进行解码，为 ID、元数据和 JSON 负载提供确定性路径。仍然需要确保每个系统调用记录预期的指针类型，并确保 Kotodama 发出正确的 TLV（包括策略拒绝类型时的错误处理）。

4) 气体和误差映射一致性
- IVM 操作码按操作收取 Gas 费用； CoreHost 现在使用本机 Gas 计划（包括批量传输和供应商 ISI 桥）为 ISI 系统调用返回额外的 Gas，并且 ZK 验证系统调用重用机密 Gas 计划。 DefaultHost 仍然保持最低的测试覆盖成本。
- 错误表面不同：IVM 返回 `VMError::{OutOfGas,PermissionDenied,...}`； ISI 返回 `InstructionExecutionError` 类别（`Find`、`Repetition`、`InvariantViolation`、`Math`、`Type`、`Mintability`、`InvalidParameter`）。5) 加速路径上的确定性
- IVM 矢量/CUDA/Metal 有 CPU 回退，但某些操作仍然保留（`SETVL`、PARBEGIN/PAREND），并且还不是确定性核心的一部分。
- IVM 和节点之间的 Merkle 树不同（`ivm::merkle_tree` 与 `iroha_crypto::MerkleTree`） - 统一项已出现在 `roadmap.md` 中。

6) Kotodama 语言表面与预期账本语义
- 编译器发出一个小子集；大多数语言功能（状态/结构、触发器、权限、类型化参数/返回）尚未连接到主机/ISI 模型。
- 没有能力/效果类型来确保系统调用对于权威机构来说是合法的。

---

## 建议（具体步骤）

### A. 在核心中实现生产 IVM 主机
- 添加实现 `ivm::host::IVMHost` 的 `iroha_core::smartcontracts::ivm::host` 模块。
- 对于 `ivm::syscalls` 中的每个系统调用：
  - 通过规范的 ABI 解码参数（参见 B.），构造相应的内置 ISI 或直接调用相同的核心逻辑，针对 `StateTransaction` 执行它，并将错误确定性地映射回 IVM 返回代码。
  - 使用核心中定义的每个系统调用表确定性地充入气体（如果将来需要，则通过 `SYSCALL_GET_PARAMETER` 暴露给 IVM）。最初，为每次调用从主机返回固定的额外气体。
- 将 `authority: &AccountId` 和 `&mut StateTransaction` 线程到主机中，以便权限检查和事件与本机 ISI 相同。
- 更新 `State::execute_trigger(ExecutableRef::Ivm)` 以在 `vm.run()` 之前附加此主机，并返回与 ISI 相同的 `ExecutionStep` 语义（事件已在核心中发出；应验证一致的行为）。

### B. 为键入的值定义确定性 VM/主机 ABI
- 在 VM 端使用 Norito 来获取结构化参数：
  - 将指针（以 x10..x13 等形式）传递到包含 Norito 编码值（例如 `AccountId`、`AssetDefinitionId`、`Numeric`、`Metadata` 等类型）的 VM 内存区域。
  - 主机通过 `IVM` 内存助手读取字节并使用 Norito 进行解码（`iroha_data_model` 已派生 `Encode/Decode`）。
- 在 Kotodama codegen 中添加最少的帮助程序，以将文字 ID 序列化到代码/常量池中或在内存中准备调用帧。
- 金额为 `Numeric` 并作为 NoritoBytes 指针传递；其他复杂类型也通过指针传递。
- 将其记录在 `crates/ivm/docs/calling_convention.md` 中并添加示例。### C. 使系统调用命名和覆盖范围与 ISI/数据模型保持一致
- 为了清晰起见，重命名 NFT 相关的系统调用：规范名称现在遵循 `SYSCALL_NFT_*` 模式（`SYSCALL_NFT_MINT_ASSET`、`SYSCALL_NFT_SET_METADATA` 等）。
- 发布从每个系统调用到核心 ISI 语义的映射表（文档 + 代码注释），包括：
  - 参数（寄存器与指针）、预期先决条件、事件和错误映射。
  - 煤气费。
- 确保每个内置 ISI 都有一个可从 Kotodama 调用的系统调用（域、帐户、资产、角色/权限、触发器、参数）。如果 ISI 必须保持特权，请将其记录下来并通过主机中的权限检查来强制执行。

### D. 统一错误和气体
- 在主机中添加转换层：将 `InstructionExecutionError::{Find,Repetition,InvariantViolation,Math,Type,Mintability,InvalidParameter}` 映射到特定的 `VMError` 代码或扩展结果约定（例如，设置 `x10=0/1` 并使用明确定义的 `VMError::HostRejected { code }`）。
- 在系统调用核心中引入气体表；将其镜像到 IVM 文档中；确保成本在输入规模上是可预测的并且与平台无关。

### E. 决定论和共享原语
- 完成 Merkle 树统一（参见路线图），并删除/别名 `ivm::merkle_tree` 为 `iroha_crypto`，具有相同的叶子和证明。
- 保留 `SETVL`/PARBEGIN/PAREND` 保留，直到端到端确定性检查和确定性调度程序策略就位；文档表明 IVM 今天忽略了这些提示。
- 确保加速路径产生逐字节相同的输出；如果不可行，请通过测试来保护功能，以确保 CPU 回退等效性。

### F.Kotodama 编译器接线
- 将代码生成扩展到 ID 和复杂参数的规范 ABI (B.)；停止使用整数→ID 演示地图。
- 使用清晰的名称添加直接映射到资产（域/帐户/角色/权限/触发器）之外的 ISI 系统调用的内置函数。
- 添加编译时功能检查和可选的 `permission(...)` 注释；当静态证明不可能时，回退到运行时主机错误。
- 在 `crates/ivm/tests/kotodama.rs` 中添加单元测试，使用解码 Norito 参数并改变临时 WSV 的测试主机端到端编译和运行小型合约。

### G. 文档和开发人员人体工程学
- 使用系统调用映射表和 ABI 注释更新 `docs/source/data_model_and_isi_spec.md`。
- 在 `crates/ivm/docs/` 中添加新文档“IVM 主机集成指南”，描述如何在真实的 `StateTransaction` 上实现 `IVMHost`。
- 在 `README.md` 和 crate 文档中澄清 Kotodama 的目标是 IVM `.to` 字节码，并且系统调用是进入世界状态的桥梁。

---

## 建议映射表（初稿）

代表性子集 - 在主机实现期间最终确定和扩展。- SYSCALL_REGISTER_DOMAIN(id: ptr DomainId) → ISI 注册
- SYSCALL_REGISTER_ACCOUNT(id: ptr AccountId) → ISI 注册
- SYSCALL_REGISTER_ASSET(id: ptr AssetDefinitionId, mintable: u8) → ISI Register
- SYSCALL_MINT_ASSET（账户：ptr AccountId，资产：ptr AssetDefinitionId，金额：ptr NoritoBytes（数字））→ ISI Mint
- SYSCALL_BURN_ASSET（账户：ptr AccountId，资产：ptr AssetDefinitionId，金额：ptr NoritoBytes（数字））→ ISI Burn
- SYSCALL_TRANSFER_ASSET（从：ptr AccountId，到：ptr AccountId，资产：ptr AssetDefinitionId，金额：ptr NoritoBytes（数字））→ ISI Transfer
- SYSCALL_TRANSFER_V1_BATCH_BEGIN() / SYSCALL_TRANSFER_V1_BATCH_END() → ISI TransferAssetBatch（打开/关闭范围；通过 `transfer_asset` 降低各个条目）
- SYSCALL_TRANSFER_V1_BATCH_APPLY(&NoritoBytes) → 当合约已经在链外序列化条目时提交预编码批次
- SYSCALL_NFT_MINT_ASSET（id：ptr NftId，所有者：ptr AccountId）→ ISI 注册
- SYSCALL_NFT_TRANSFER_ASSET（从：ptr AccountId，到：ptr AccountId，id：ptr NftId）→ ISI Transfer
- SYSCALL_NFT_SET_METADATA(id: ptr NftId, 内容: ptr 元数据) → ISI SetKeyValue
- SYSCALL_NFT_BURN_ASSET(id: ptr NftId) → ISI 取消注册
- SYSCALL_CREATE_ROLE(id: ptr RoleId, 角色: ptr Role) → ISI 注册
- SYSCALL_GRANT_ROLE（帐户：ptr AccountId，角色：ptr RoleId）→ ISI Grant
- SYSCALL_REVOKE_ROLE（帐户：ptr AccountId，角色：ptr RoleId）→ ISI 撤销
- SYSCALL_SET_PARAMETER（参数：ptr 参数）→ ISI SetParameter

注释
- “ptr T”表示寄存器中指向 T 的 Norito 编码字节的指针，存储在 VM 内存中；主机将其解码为相应的`iroha_data_model`类型。
- 返回约定：成功设置`x10=1`；失败会设置 `x10=0`，并可能因致命错误而引发 `VMError::HostRejected`。

---

## 风险和推出计划
- 首先为一个狭窄的集合（资产+帐户）连接主机并添加有针对性的测试。
- 在主机语义成熟的情况下，保持本机 ISI 执行作为权威路径；在测试中在“影子模式”下运行两条路径，以断言相同的最终效果和事件。
- 验证奇偶校验后，为生产中的 IVM 触发器启用 IVM 主机；稍后也考虑通过 IVM 路由常规事务。

---

## 杰出作品
- 最终确定传递 Norito 编码指针 (`crates/ivm/src/kotodama_std.rs`) 的 Kotodama 帮助程序，并通过编译器 CLI 显示它们。
- 发布系统调用气体表（包括辅助系统调用）并保持 CoreHost 执行/测试与其保持一致。
- ✅ 添加了涵盖指针参数 ABI 的往返 Norito 固定装置；请参阅 `crates/iroha_data_model/tests/norito_pointer_abi_roundtrip.rs` 了解 CI 中保存的清单和 NFT 指针覆盖范围。