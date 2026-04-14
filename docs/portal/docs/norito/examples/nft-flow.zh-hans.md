<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09ff44a2df8cbcb9f57017239070a16f5287cbfc59a8289ce54933e84f90a5e8
source_last_modified: "2026-03-26T13:01:47.374572+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/nft-flow
title: 铸造、转移和销毁 NFT
description: 端到端地遍历 NFT 生命周期：铸造给所有者、传输、标记元数据和刻录。
source: crates/ivm/docs/examples/12_nft_flow.ko
---

端到端地遍历 NFT 生命周期：铸造给所有者、传输、标记元数据和刻录。

## 账本演练

- Ensure the NFT definition (for example `n0#wonderland`) exists alongside the owner/recipient accounts used in the snippet (`<i105-account-id>` for Alice, `<i105-account-id>` for Bob).
- 调用 `nft_issue_and_transfer` 入口点来铸造 NFT，将其从 Alice 转移到 Bob，并附加描述发行的元数据标志。
- Inspect the NFT ledger state with `iroha_cli ledger nft list --account <id>` or the SDK equivalents to verify the transfer, then confirm the asset is removed once the burn instruction runs.

## 相关SDK指南

- [Rust SDK 快速入门](/sdks/rust)
- [Python SDK 快速入门](/sdks/python)
- [JavaScript SDK 快速入门](/sdks/javascript)

[下载Kotodama源码](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  #[access(read="*", write="*")]
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```