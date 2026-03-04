---
lang: zh-hans
direction: ltr
source: docs/source/agents.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7f35a28d00188a3e1f3db76b56e6b29c708dbb75afa3dd009d416b7cd4314754
source_last_modified: "2025-12-29T18:16:35.916241+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 自动化代理执行指南

本页总结了任何自动化代理的操作护栏
在 Hyperledger Iroha 工作空间内工作。它反映了规范
`AGENTS.md` 指南和路线图参考，以便构建、文档和
遥测变化看起来都是一样的，无论它们是由人类还是由人类产生的
自动贡献者。

每个任务都期望获得确定性代码以及匹配的文档、测试、
和操作证据。将以下部分作为准备之前的参考
触摸 `roadmap.md` 项目或回答行为问题。

## 快速启动命令

|行动|命令|
|--------|---------|
|构建工作区 | `cargo build --workspace` |
|运行完整的测试套件 | `cargo test --workspace` *（通常需要几个小时）* |
|运行 Clippy 并显示默认拒绝警告 | `cargo clippy --workspace --all-targets -- -D warnings` |
|格式化 Rust 代码 | `cargo fmt --all` *（2024 年版）* |
|测试单个板条箱 | `cargo test -p <crate>` |
|运行一项测试 | `cargo test -p <crate> <test_name> -- --nocapture` |
| Swift SDK 测试 |从 `IrohaSwift/` 开始，运行 `swift test` |

## 工作流程基础知识

- 在回答问题或更改逻辑之前阅读相关代码路径。
- 将大型路线图项目分解为易于处理的提交；永远不要直接拒绝工作。
- 留在现有的工作区成员资格内，重复使用内部板条箱，并执行以下操作：
  **不**改变 `Cargo.lock` 除非明确指示。
- 仅在硬件要求的情况下使用功能标志和功能切换
  加速器；在每个平台上保持确定性后备可用。
- 更新文档和 Markdown 参考以及任何功能更改
  所以文档总是描述当前的行为。
- 为每个新的或修改的功能添加至少一个单元测试。更喜欢内联
  `#[cfg(test)]` 模块或板条箱的 `tests/` 文件夹，具体取决于范围。
- 完成工作后，更新 `status.md` 并附上简短的总结和参考
  相关文件；让 `roadmap.md` 专注于仍需要工作的项目。

## 实施护栏

### 序列化和数据模型
- 到处使用 Norito 编解码器（通过 `norito::{Encode, Decode}` 进行二进制编码，
  JSON 通过 `norito::json::*`）。不要添加直接的 serde/`serde_json` 用法。
- Norito 有效负载必须通告其布局（版本字节或标头标志），
  新格式需要相应的文档更新（例如，
  `norito.md`、`docs/source/da/*.md`）。
- 创世数据、清单和网络有效负载应保持确定性
  因此具有相同输入的两个对等点会产生相同的哈希值。

### 配置和运行时行为
- 与新的环境变量相比，更喜欢位于 `crates/iroha_config` 中的旋钮。
  通过构造函数或依赖项注入显式线程化值。
- 切勿对 IVM 系统调用或操作码行为进行门控 - ABI v1 随处可见。
- 添加新的配置选项时，更新默认值、文档和任何相关的
  模板（`peer.template.toml`、`docs/source/configuration*.md` 等）。### ABI、系统调用和指针类型
- 将 ABI 政策视为无条件。添加/删除系统调用或指针类型
  需要更新：
  - `ivm::syscalls::abi_syscall_list` 和 `crates/ivm/tests/abi_syscall_list_golden.rs`
  - `ivm::pointer_abi::PointerType` 加上黄金测试
  - 每当 ABI 哈希发生变化时，`crates/ivm/tests/abi_hash_versions.rs`
- 未知的系统调用必须映射到 `VMError::UnknownSyscall`，并且清单必须
  在入学考试中保留签署的 `abi_hash` 平等检查。

### 硬件加速和确定性
- 新的加密原语或繁重的数学必须通过硬件加速来实现
  路径（METAL/NEON/SIMD/CUDA），同时保持确定性回退。
- 避免非确定性并行归约；优先级是相同的输出
  即使硬件不同，每个对等点也是如此。
- 保持 Norito 和 FASTPQ 装置的可重复性，以便 SRE 可以审核整个车队
  遥测。

### 文件和证据
- 镜像门户中任何面向公众的文档更改 (`docs/portal/...`)
  适用，以便文档网站与 Markdown 源保持最新。
- 引入新工作流程时，添加运行手册、治理说明或
  检查表解释如何排练、回滚和捕获证据。
- 将内容翻译成阿卡德语时，提供书面的语义渲染
  用楔形文字而不是音译。

### 测试和工具期望
- 在本地运行相关测试套件（`cargo test`、`swift test`、
  集成工具）并在 PR 测试部分记录命令。
- 使 CI 防护脚本 (`ci/*.sh`) 和仪表板与新遥测保持同步。
- 对于 proc-macros，将单元测试与 `trybuild` UI 测试配对以锁定诊断。

## 准备发货清单

1. 代码编译，`cargo fmt` 没有产生差异。
2.更新的文档（工作区 Markdown 加上门户镜像）描述了新的内容
   行为、新的 CLI 标志或配置旋钮。
3. 测试覆盖每一个新的代码路径，并且在回归时会确定性地失败
   出现。
4. 遥测、仪表板和警报定义引用任何新指标或
   错误代码。
5. `status.md` 包括引用相关文件的简短摘要以及
   路线图部分。

遵循此清单可以使路线图执行可审核并确保每个
代理提供其他团队可以信任的证据。