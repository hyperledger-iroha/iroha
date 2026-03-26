---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
スラグ: /norito/examples/nft-flow
タイトル: フラッパー、転送者、および NFT のブリュラー
説明: 試合中の NFT サイクル サイクル パルクール: フラッペ・オ・プロプリエテール、転送、メタドンネの一時停止および破壊。
ソース: crates/ivm/docs/examples/12_nft_flow.ko
---

NFT の試合中のパルクール サイクル ドゥ ヴィー ダン : フラッペ オー プロプリエテール、転送、メタドンネの一時停止、および破壊。

## 登録公園

- NFT の定義 (`n0#wonderland` など) は、スニペット (`<katakana-i105-account-id>`、`<katakana-i105-account-id>`) によって所有権/目的地が使用されることを保証します。
- NFT のポイント ポイント `nft_issue_and_transfer` を呼び出し、アリスとボブの転送を指示し、メッセージの送信を指示します。
- NFT の登録情報を検査し、`iroha_cli ledger nfts list --account <id>` と同等の SDK を検証して転送し、書き込み命令の実行を確認します。

## SDK アソシエをガイドします

- [クイックスタート SDK Rust](/sdks/rust)
- [クイックスタート SDK Python](/sdks/python)
- [クイックスタート SDK JavaScript](/sdks/javascript)

[情報源からの電話番号 Kotodama](/norito-snippets/nft-flow.ko)

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