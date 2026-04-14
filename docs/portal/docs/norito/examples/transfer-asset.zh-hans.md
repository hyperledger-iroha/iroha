<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 479d552d0f641875518c62059be1084af6ddf99213662a753c73ea57512b8e5f
source_last_modified: "2026-04-02T18:24:28.189405+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/transfer-asset
title: 在账户之间转移资产
description: 简单的资产转移工作流程，反映了 SDK 快速入门和账本演练。
source: examples/transfer/transfer.ko
---

简单的资产转移工作流程，反映了 SDK 快速入门和账本演练。

## 账本演练

- 使用目标资产为 Alice 预先提供资金（例如通过“注册和铸造”片段或 SDK 快速启动流程）。
- 执行 `do_transfer` 入口点，将 10 个单位从 Alice 移动到 Bob，满足 `AssetTransferRole` 权限。
- 查询余额（`FindAccountAssets`、`iroha_cli ledger asset list`）或订阅管道事件以观察传输结果。

## 相关SDK指南

- [Rust SDK 快速入门](/sdks/rust)
- [Python SDK 快速入门](/sdks/python)
- [JavaScript SDK 快速入门](/sdks/javascript)

[下载Kotodama源码](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of the canonical Base58 asset definition between canonical I105 accounts
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("sorauロ1QG1シタ3vN7ヒzトヘcミLKDCAイ5クエjヤリ2uトユmキユルeJBJW7X2N7"),
      account!("sorauロ1NksツJZミLツスjヨrUphCSホ8Wノスマチモr3ムLセヌヒYqwフノFTMDQE"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```