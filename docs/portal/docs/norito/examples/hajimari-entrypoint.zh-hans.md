<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
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
title: Hajimari 入口点骨架
description: 具有单个公共入口点和状态句柄的最小 Kotodama 合约脚手架。
source: crates/ivm/docs/examples/01_hajimari.ko
---

具有单个公共入口点和状态句柄的最小 Kotodama 合约脚手架。

## 账本演练

- 使用 `koto_compile --abi 1` 编译合约，如 [Norito 入门](/norito/getting-started#1-compile-a-kotodama-contract) 或通过 `cargo test -p ivm developer_portal_norito_snippets_compile` 所示。
- 使用 `ivm_run` / `developer_portal_norito_snippets_run` 在本地对字节码进行冒烟测试，以在接触节点之前验证 `info!` 日志和初始系统调用。
- 通过 `iroha_cli app contracts deploy` 部署工件，并使用 [Norito 入门](/norito/getting-started#4-deploy-via-iroha_cli) 中的步骤确认清单。

## 相关SDK指南

- [Rust SDK 快速入门](/sdks/rust)
- [Python SDK 快速入门](/sdks/python)
- [JavaScript SDK 快速入门](/sdks/javascript)

[下载Kotodama源码](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```