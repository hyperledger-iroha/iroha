---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/norito/examples/nft-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ab9ab484b2586e01e1af65bf125de14be97fe5b613f8036882928494942b82fb
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/nft-flow
title: NFT のミント、移転、バーン
description: NFT のライフサイクルを端から端までたどります: オーナーへのミント、移転、メタデータのタグ付け、バーン。
source: crates/ivm/docs/examples/12_nft_flow.ko
---

NFT のライフサイクルを端から端までたどります: オーナーへのミント、移転、メタデータのタグ付け、バーン。

## 台帳ウォークスルー

- NFT 定義（例: `n0#wonderland`）が存在し、スニペットで使用する所有者/受領者アカウント (`ih58...`, `ih58...`) が用意されていることを確認します。
- `nft_issue_and_transfer` エントリポイントを呼び出して NFT をミントし、Alice から Bob へ移転し、発行内容を示すメタデータフラグを付与します。
- `iroha_cli ledger nfts list --account <id>` または SDK の同等機能で NFT 台帳の状態を確認して移転を検証し、その後バーン命令が実行されると資産が削除されることを確かめます。

## 関連 SDK ガイド

- [Rust SDK クイックスタート](/sdks/rust)
- [Python SDK クイックスタート](/sdks/python)
- [JavaScript SDK クイックスタート](/sdks/javascript)

[Kotodama ソースをダウンロード](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("ih58...");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("ih58...");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```
