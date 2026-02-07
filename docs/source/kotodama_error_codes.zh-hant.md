---
lang: zh-hant
direction: ltr
source: docs/source/kotodama_error_codes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e0e4f16000f6a578fe9c9d6e204c01087e987ac3b46d70537a15b072df48a13
source_last_modified: "2025-12-29T18:16:35.974178+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama 編譯器錯誤代碼

Kotodama 編譯器會發出穩定的錯誤代碼，以便工具和 CLI 用戶可以
快速了解故障原因。使用 `koto_compile --explain <code>`
打印相應的提示。

|代碼|描述 |典型修復|
|--------|-------------|-------------|
| `E0001` |分支目標超出 IVM 跳轉編碼的範圍。 |拆分非常大的函數或減少內聯，使基本塊距離保持在 ±1MiB 之內。 |
| `E0002` |調用站點引用了一個從未定義的函數。 |檢查是否存在拼寫錯誤、可見性修飾符或刪除了被調用者的功能標記。 |
| `E0003` |在未啟用 ABI v1 的情況下發出持久狀態系統調用。 |設置 `CompilerOptions::abi_version = 1` 或在 `seiyaku` 合約中添加 `meta { abi_version: 1 }`。 |
| `E0004` |與資產相關的系統調用收到非文字指針。 |使用 `account_id(...)`、`asset_definition(...)` 等，或傳遞 0 個標記作為主機默認值。 |
| `E0005` | `for` 循環初始值設定項比目前支持的更複雜。 |將復雜的設置移至循環之前；目前僅接受簡單的 `let`/表達式初始化程序。 |
| `E0006` | `for`-loop 步驟子句比現在支持的更複雜。 |使用簡單的表達式更新循環計數器（例如 `i = i + 1`）。 |