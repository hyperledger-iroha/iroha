---
lang: zh-hans
direction: ltr
source: docs/source/kotodama_error_codes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e0e4f16000f6a578fe9c9d6e204c01087e987ac3b46d70537a15b072df48a13
source_last_modified: "2025-12-29T18:16:35.974178+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama 编译器错误代码

Kotodama 编译器会发出稳定的错误代码，以便工具和 CLI 用户可以
快速了解故障原因。使用 `koto_compile --explain <code>`
打印相应的提示。

|代码|描述 |典型修复|
|--------|-------------|-------------|
| `E0001` |分支目标超出 IVM 跳转编码的范围。 |拆分非常大的函数或减少内联，使基本块距离保持在 ±1MiB 之内。 |
| `E0002` |调用站点引用了一个从未定义的函数。 |检查是否存在拼写错误、可见性修饰符或删除了被调用者的功能标记。 |
| `E0003` |在未启用 ABI v1 的情况下发出持久状态系统调用。 |设置 `CompilerOptions::abi_version = 1` 或在 `seiyaku` 合约中添加 `meta { abi_version: 1 }`。 |
| `E0004` |与资产相关的系统调用收到非文字指针。 |使用 `account_id(...)`、`asset_definition(...)` 等，或传递 0 个标记作为主机默认值。 |
| `E0005` | `for` 循环初始值设定项比目前支持的更复杂。 |将复杂的设置移至循环之前；目前仅接受简单的 `let`/表达式初始化程序。 |
| `E0006` | `for`-loop 步骤子句比现在支持的更复杂。 |使用简单的表达式更新循环计数器（例如 `i = i + 1`）。 |