---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
スラグ: /norito/examples/nft-flow
タイトル: アクーニャル、NFT の譲渡とケマル
説明: NFT の極端な情報を記録します: 独自の知識、転送、メタデータと問題の知識。
ソース: crates/ivm/docs/examples/12_nft_flow.ko
---

NFT の極端な情報を記録します: 独自の情報、転送、メタデータと問題の知識。

## 市長のレコリード

- NFT の定義を確認します (`n0#wonderland`) 断片化されたプロピエタリオ/受容体を使用する必要があります (`<katakana-i105-account-id>`、`<katakana-i105-account-id>`)。
- エントリポイント `nft_issue_and_transfer` を NFT から呼び出し、アリスからボブへの転送、メタデータの説明の追加を行います。
- `iroha_cli ledger nfts list --account <id>` の NFT 市長の図書館の検査は、転送された SDK の同等性を検証し、指示を解除するかどうかを確認します。

## SDK 関係に関する情報

- [SDK および Rust のクイックスタート](/sdks/rust)
- [Python の SDK クイックスタート](/sdks/python)
- [JavaScript の SDK のクイックスタート](/sdks/javascript)

[Kotodama](/norito-snippets/nft-flow.ko) をダウンロードしてください

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("<katakana-i105-account-id>");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("<katakana-i105-account-id>");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```