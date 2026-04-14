<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8367687fcf43fcb50ab43940a4ebeb8b8ba22a3ab8a6c3ed5088c52b1fdd7baf
source_last_modified: "2026-01-22T15:38:30.521640+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/hajimari-entrypoint
title: Hajimari 入口點骨架
description: 具有單一公共入口點和狀態句柄的最小 Kotodama 合約腳手架。
source: crates/ivm/docs/examples/01_hajimari.ko
---

具有單一公共入口點和狀態句柄的最小 Kotodama 合約腳手架。

## 帳本演練

- 使用 `koto_compile --abi 1` 編譯合約，如 [Norito 入門](/norito/getting-started#1-compile-a-kotodama-contract) 或透過 `cargo test -p ivm developer_portal_norito_snippets_compile` 所示。
- 使用 `ivm_run` / `developer_portal_norito_snippets_run` 在本機對字節碼進行冒煙測試，以在接觸節點之前驗證 `info!` 日誌和初始系統呼叫。
- 透過 `iroha_cli app contracts deploy` 部署工件，並使用 [Norito 入門](/norito/getting-started#4-deploy-via-iroha_cli) 中的步驟確認清單。

## 相關SDK指南

- [Rust SDK 快速入門](/sdks/rust)
- [Python SDK 快速入門](/sdks/python)
- [JavaScript SDK 快速入門](/sdks/javascript)

[下載Kotodama原始碼](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```