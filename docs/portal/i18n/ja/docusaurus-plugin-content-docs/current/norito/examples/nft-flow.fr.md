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

- NFT の定義 (`n0#wonderland` など) は、スニペット (`<i105-account-id>`、`<i105-account-id>`) によって所有権/目的地が使用されることを保証します。
- NFT のポイント ポイント `nft_issue_and_transfer` を呼び出し、アリスとボブの転送を指示し、メッセージの送信を指示します。
- NFT の登録情報を検査し、`iroha_cli ledger nfts list --account <id>` と同等の SDK を検証して転送し、書き込み命令の実行を確認します。

## SDK アソシエをガイドします

- [クイックスタート SDK Rust](/sdks/rust)
- [クイックスタート SDK Python](/sdks/python)
- [クイックスタート SDK JavaScript](/sdks/javascript)

[情報源からの電話番号 Kotodama](/norito-snippets/nft-flow.ko)

```kotodama
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
    kotoage fn nft_issue_and_transfer() authorize("NftAuthority") {
        let owner = AccountId::parse(
            "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
        );
        let nft = NftId::parse("n0$wonderland.universal");
        ledger::nft::mint(nft, owner);
        let to = AccountId::parse(
            "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
        );
        ledger::nft::transfer(source: owner, nft: nft, destination: to);
        ledger::nft::set_metadata(
            nft: nft,
            key: Name::parse("issued"),
            value: Json::parse("{\"issued\":\"demo\"}"),
        );
        ledger::nft::burn(nft);
    }
}
```
