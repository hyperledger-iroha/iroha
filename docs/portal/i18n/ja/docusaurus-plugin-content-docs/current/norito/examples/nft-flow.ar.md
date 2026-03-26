---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
スラグ: /norito/examples/nft-flow
タイトル: NFT のタイトル
説明: يسرد دورة حياة NFT من البداية إلى النهاية: السك للمالك، النقل، ووسم بيانات التعريف،やあ。
ソース: crates/ivm/docs/examples/12_nft_flow.ko
---

NFT は、NFT を使用します。

## جولة دفتر الأستاذ

- NFT (`n0#wonderland`) を使用して、NFT を使用してください。 (`i105...`、`i105...`)。
- `nft_issue_and_transfer` と NFT とアリスとボブとの接続ああ。
- セキュリティ NFT セキュリティ `iroha_cli ledger nfts list --account <id>` セキュリティ SDK セキュリティ セキュリティ セキュリティテストを実行してください。

## SDK の開発

- [Rust SDK](/sdks/rust)
- [Python SDK](/sdks/python)
- [JavaScript SDK](/sdks/javascript)

[Kotodama](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("soraカタカナ...");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("soraカタカナ...");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```