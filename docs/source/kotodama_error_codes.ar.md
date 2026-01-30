---
lang: ar
direction: rtl
source: docs/source/kotodama_error_codes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e0e4f16000f6a578fe9c9d6e204c01087e987ac3b46d70537a15b072df48a13
source_last_modified: "2026-01-03T18:08:01.373878+00:00"
translation_last_reviewed: 2026-01-30
---

# Kotodama Compiler Error Codes

The Kotodama compiler emits stable error codes so that tooling and CLI users can
quickly understand the cause of a failure. Use `koto_compile --explain <code>`
to print the corresponding hint.

| Code  | Description | Typical Fix |
|-------|-------------|-------------|
| `E0001` | Branch target is out of range for the IVM jump encoding. | Split very large functions or reduce inlining so basic block distances stay within ±1 MiB. |
| `E0002` | Call sites reference a function that was never defined. | Check for typos, visibility modifiers, or feature flags that removed the callee. |
| `E0003` | Durable state syscalls were emitted without ABI v1 enabled. | Set `CompilerOptions::abi_version = 1` or add `meta { abi_version: 1 }` inside the `seiyaku` contract. |
| `E0004` | Asset-related syscalls received non-literal pointers. | Use `account_id(...)`, `asset_definition(...)`, etc., or pass 0 sentinels for host defaults. |
| `E0005` | `for`-loop initializer is more complex than supported today. | Move complex setup before the loop; only simple `let`/expression initialisers are currently accepted. |
| `E0006` | `for`-loop step clause is more complex than supported today. | Update the loop counter with a simple expression (e.g. `i = i + 1`). |
