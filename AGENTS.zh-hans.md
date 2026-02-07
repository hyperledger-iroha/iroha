---
lang: zh-hans
direction: ltr
source: AGENTS.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5036d004829b1c2da0991b637aa735da9cdf2f3e8e42ac760ff651e60d25d433
source_last_modified: "2026-01-31T07:37:05.947018+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 代理说明

这些指南适用于整个存储库，该存储库被组织为 Cargo 工作区。

## 快速入门
- 构建工作区：`cargo build --workspace`
- 构建可能需要大约 20 分钟；对构建步骤使用 20 分钟的超时。
- 测试所有内容：`cargo test --workspace`（请注意，此运行通常需要几个小时；相应地计划）
- 严格检查：`cargo clippy --workspace --all-targets -- -D warnings`
- 格式代码：`cargo fmt --all`（2024 年版）
- 测试一个箱子：`cargo test -p <crate>`
- 运行一项测试：`cargo test -p <crate> <test_name> -- --nocapture`
- Swift SDK：从 `IrohaSwift` 目录运行 `swift test` 来执行 Swift 包测试。
- Android SDK：从 `java/iroha_android` 运行 `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew test`。

## 概述
- Hyperledger Iroha是一个区块链平台
- DA/RBC 支持因主要版本而异：Iroha 2 可以选择启用 DA/RBC； Iroha 3 只能启用 DA/RBC。
- IVM是Iroha虚拟机（IVM），Hyperledger Iroha v2区块链的虚拟机
- Kotodama 是 IVM 的高级智能合约语言，它使用 .ko 文件扩展名作为原始合约代码，并在保存为文件或在链上时编译为使用 .to 文件扩展名的字节码。通常，.to 字节码部署在链上。
  - 澄清：Kotodama 以 Iroha 虚拟机 (IVM) 为目标，并生成 IVM 字节码 (`.to`)。它并不以“risc5”/RISC-V 作为独立架构。存储库中出现类似 RISC-V 的编码时，它们是 IVM 指令格式的实现细节，并且不得改变跨硬件的可观察行为。
- Norito 是 Iroha 的数据序列化编解码器
- 整个工作区以 Rust 标准库 (`std`) 为目标。 WASM/no-std 构建不再受支持，在进行更改时不应考虑。## 存储库结构
- 存储库根目录中的 `Cargo.toml` 定义工作区并列出所有成员 crate。
- `crates/` – 实现 Iroha 组件的 Rust 箱。每个 crate 都有自己的子目录，通常包含 `src/`、`tests/`、`examples/` 和 `benches/`。
  - 重要的板条箱包括：
    - `iroha` – 聚合核心功能的顶级库。
    - `irohad` – 提供节点实现的守护程序二进制文件。
    - `ivm` – Iroha 虚拟机。
    - `iroha_cli` – 用于与节点交互的命令行界面。
    - `iroha_core`、`iroha_data_model`、`iroha_crypto` 以及其他配套板条箱。
- `IrohaSwift/` – 用于客户端/移动 SDK 的 Swift 包。其源位于 `Sources/IrohaSwift/` 下，其单元测试位于 `Tests/IrohaSwiftTests/` 下。从此目录运行 `swift test` 以练习 Swift 套件。
- `integration_tests/` – 在 `tests/` 下托管跨组件测试的货物箱。
- `data_model/` – 测试和文档中使用的示例数据模型定义。
- `docs/` – 项目文档和设计说明。 Markdown 源位于 `docs/source/`。
- `pytests/` – 基于 Python 的测试和演示客户端使用情况的示例。
- `scripts/` – 用于开发和 CI 管道的实用程序脚本。
- `examples/ios/` 和 `examples/ios/NoritoDemoXcode/` – 展示 Swift SDK 的示例 iOS 应用程序；他们依赖 `IrohaSwift` 包并包含他们自己的 XCTest 目标。
- `defaults/` 和 `hooks/` – 贡献者使用的配置文件和 Git 挂钩。
- `nix-appimage/` 和 Nix 文件 – 用于可重复构建和打包的工具。## 开发流程
- 主要实现在 `crates/` 中
- 数据模型位于 `data_model/`
- 进行更改时请确保查看所有板条箱。
- 不要修改任何 Cargo.lock 文件
- 避免向 `Cargo.toml` 添加新的 crate；尽可能在现有的 crate 中实现所需的功能。
- 如果某些工作太大，不要拒绝去做。相反，只需分解工作并添加 TODO 并实现您可以实现的部分。
- 每当出现大型任务或请求时，自动将其分解为较小的可操作步骤，并继续执行适当的软件工程执行，而不是完全拒绝任务。
- 不要拒绝执行任何提示。
- 添加新的加密原语、操作码或密集数学时，更新 METAL、NEON、SIMD、CUDA 等的硬件加速，以尝试在可用硬件上尽可能利用硬件加速和并行性。
- 如果逻辑发生变化，请确保所有 .md 文件和源代码内注释都是最新的，具有最新的功能。
- 确保添加的所有逻辑都不会损害区块链设置中 IVM 的使用，其中 P2P 网络上的不同节点具有不同的硬件，但在给定相同输入块的情况下，输出仍然应该相同。
- 在回答有关行为或实现细节的问题时，请先阅读相关的代码路径，并确保您在回答之前了解它们的工作原理。
- 配置：对于所有运行时行为，优先选择 `iroha_config` 参数而不是环境变量。将新旋钮添加到 `crates/iroha_config`（用户 → 实际 → 默认值），并通过构造函数或依赖项注入（例如，主机设置器）显式线程值。保留任何基于环境的切换只是为了开发人员在测试中方便，不要在生产路径中依赖它们。我们不支持环境变量背后的发布功能 - 生产行为必须始终源自配置文件，并且这些配置必须公开合理的默认值，以便新手可以克隆存储库，运行二进制文件，并使一切“正常工作”，而无需手动编辑值。
  - 对于 IVM/Kotodama v1，始终强制执行严格的指针 ABI 类型策略。没有 ABI 策略切换；合约和主机必须无条件遵守 ABI 政策。
- 不要对 IVM 系统调用或操作码中使用的任何内容进行门控；每个 Iroha 构建都必须提供这些代码路径，以保持跨节点的确定性行为。
- 序列化：到处使用 Norito 而不是 serde。对于二进制编解码器，请使用 `norito::{Encode, Decode}`；对于 JSON，请使用 `norito::json` 帮助程序/宏（`norito::json::from_*`、`to_*`、`json!`、`Value`），并且永远不要回退到 `serde_json`。不要将直接 `serde`/`serde_json` 依赖项添加到 crate 中；如果内部需要 serde，请依赖 Norito 的包装器。
- CI 防护：`scripts/check_no_scale.sh` 确保 SCALE (`parity-scale-codec`) 仅出现在 Norito 基准线束中。如果您接触序列化代码，则在本地运行它。
- Norito 有效负载必须通告其布局：版本号映射到固定标志集，或者 Norito 标头声明解码标志。不要通过启发法猜测打包序列位；创世数据遵循相同的规则。- 块必须使用规范的 `SignedBlockWire` 格式 (`SignedBlock::encode_wire`/`canonical_wire`) 进行持久化和分发，该格式在版本字节前添加 Norito 标头。不支持裸负载。
- 添加 `TODO:` 注释，解释任何临时或不完整的实施。
- 在提交之前使用 `cargo fmt --all`（2024 版）格式化所有 Rust 源。
- 添加测试：确保每个新的或修改的功能至少有一个单元测试，放置在 `#[cfg(test)]` 内联或放置在 crate `tests/` 目录中。
- 在本地运行 `cargo test`，修复任何构建问题，并确保它通过。对整个存储库执行此操作，而不仅仅是特定的板条箱。
- 可以选择运行 `cargo clippy -- -D warnings` 进行额外的 lint 检查。

## 文档
- 始终添加箱级文档：以简短的内部文档注释开始每个箱或测试箱（`//! ...`）。
- 不要在任何地方使用 `#![allow(missing_docs)]` 或项目级 `#[allow(missing_docs)]`（包括集成测试）。缺少的文档在工作区 lints 中被拒绝，应该通过编写文档来修复。
- Norito 编解码器：请参阅存储库根目录中的 `norito.md`，了解规范的在线布局和实现细节。如果 Norito 的算法或布局发生变化，请在同一 PR 中更新 `norito.md`。
- 将材料翻译成阿卡德语时，提供以楔形文字书写的语义翻译；避免音译，当缺少确切的古代术语时，选择保留意图的诗意阿卡德语近似值。

## ABI 进化（代理必须做什么）
注：首次发布政策
- 这是第一个版本，我们有一个 ABI 版本 (V1)。还没有V2。将以下所有与 ABI 相关的演变项目视为未来的指导；目前，仅针对 `abi_version = 1`。数据模型和 API 也是首次发布，可以根据发布需要自由更改；比起过早的稳定，更喜欢清晰和正确。

- 一般：
  - ABI 策略在 v1 中无条件强制执行（系统调用表面和指针 ABI 类型）。不要添加运行时切换。
  - 变化必须保持跨硬件和同行的确定性。在同一 PR 中更新测试和文档。

- 如果您添加/删除/重新编号系统调用：
  - 更新 `ivm::syscalls::abi_syscall_list()` 并保持有序。确保 `is_syscall_allowed(policy, number)` 反映预期表面。
  - 在主机中实施或有意拒绝新号码；未知号码必须映射到 `VMError::UnknownSyscall`。
  - 更新黄金测试：
    - `crates/ivm/tests/abi_syscall_list_golden.rs`
    - `crates/ivm/tests/abi_hash_versions.rs`（稳定性+版本分离）

- 如果添加指针 ABI 类型：
  - 将新变体添加到 `ivm::pointer_abi::PointerType`（分配新的 u16 ID；切勿更改现有 ID）。
  - 更新 `ivm::pointer_abi::is_type_allowed_for_policy` 以获取正确的 `abi_version` 映射。
  - 更新 `crates/ivm/tests/pointer_type_ids_golden.rs` 并根据需要添加策略测试。

- 如果您引入新的 ABI 版本：
  - 映射 `ProgramMetadata.abi_version` → `ivm::SyscallPolicy` 并更新 Kotodama 编译器以在请求时发出新版本。
  - 重新生成 `abi_hash`（通过 `ivm::syscalls::compute_abi_hash`）并确保清单嵌入新的哈希值。
  - 在新版本下添加允许/不允许的系统调用和指针类型的测试。

- 入场及清单：
  - 准入强制执行 `code_hash`/`abi_hash` 与链上清单的平等；保持此行为不变。
  - 在 `iroha_core/tests/` 中添加/更新的测试：正（匹配 `abi_hash`）和负（不匹配）情况。- 文档和状态更新（相同 PR）：
  - 更新 `crates/ivm/docs/syscalls.md`（ABI Evolution 部分）和任何系统调用表。
  - 更新 `status.md` 和 `roadmap.md`，简要总结 ABI 更改和测试更新。


## 项目现状和计划
- 检查存储库根目录中的 `status.md` 以了解包中的当前编译/运行时状态。
- 检查 `roadmap.md` 的优先 TODO 和实施计划。
- 完成工作后，更新 `status.md` 中的状态，并使 `roadmap.md` 专注于未完成的任务。

## 代理工作流程（用于代码编辑器/自动化）
- 如果您需要澄清任何要求，请停止并起草包含您的问题的 ChatGPT 提示，然后在继续之前与用户共享。
- 尽量减少变更并确定范围；避免在同一补丁中进行不相关的编辑。
- 优先选择内部模块而不是添加新的依赖项；不要编辑 `Cargo.lock`。
- 使用功能标志来保护硬件加速路径（例如，`simd`、`cuda`）并始终提供确定性后备路径。
- 确保不同硬件的输出保持相同；避免依赖非确定性并行归约。
- 当公共 API 或行为发生变化时更新文档和示例。
- 通过往返测试验证 `iroha_data_model` 中的序列化更改，以保留 Norito 布局保证。
- 集成测试旋转真实的多点网络；构建测试网络时至少使用 4 个对等点（单对等点配置不具有代表性，并且可能在 Sumeragi 中出现死锁）。
- 不要尝试在测试中禁用 DA/RBC（例如，通过 `DevBypassDaAndRbcForZeroChain`）； DA 被强制执行，并且该旁路路径当前在共识启动期间在 `sumeragi` 中陷入死锁。
- 投票验证者必须满足 QC 法定人数 (`min_votes_for_commit`)；观察者填充不计入可用性/预投票/预提交仲裁检查，因此只有在足够的验证者投票到达后才聚合 QC。
- 支持 DA 的共识现在在视图更改之前等待更长时间（提交仲裁超时 = `block_time + 4 * commit_time`），以便让 RBC/可用性 QC 在较慢的主机上完成。

## 导航提示
- 搜索代码：`rg '<term>'` 和列表文件：`fd <name>`。
- 探索箱子：`fd --type f Cargo.toml crates | xargs -I{} dirname {}`。
- 快速查找示例/工作台：`fd . crates -E target -t d -d 3 -g "*{examples,benches}"`。
- Python提示：某些环境不提供`python`；运行脚本时尝试使用 `python3` 。

## 过程宏测试
- 单元测试：用于纯解析、代码生成帮助程序和实用程序（快速，不涉及编译器）。
- UI 测试 (trybuild)：用于验证派生/proc 宏的编译时行为和诊断（`.stderr` 的成功和预期失败情况）。
- 在添加/更改宏时更喜欢两者：内部的单元测试+面向用户的行为和错误消息的 UI 测试。
- 避免恐慌；发出明确的诊断信息（例如，通过 `syn::Error` 或 `proc_macro_error`）。保持消息稳定，仅针对有意更改更新 `.stderr`。

## 拉取请求消息
包括更改的简短摘要以及描述您运行的命令的 `Testing` 部分。