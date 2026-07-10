---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
スラグ: /norito/examples/nft-flow
タイトル: Cunhar、NFT の転送
説明: NFT の最初のアクションを実行します: パラオ ドノ、トランスファーレンシア、マルカカオ デ メタダドス、キーマ。
ソース: crates/ivm/docs/examples/12_nft_flow.ko
---

NFT は最初のビデオを実行します: コンテンツ、トランスファーレンシア、マルカソン デ メタダドス、およびキーマ。

## ロテイロ・ド・リヴロ・ラザオ

- NFT を定義することができます (`n0#wonderland` など) は、管理対象外/目的地として米国にアクセスできません (`<i105-account-id>`、`<i105-account-id>`)。
- NFT からエントリポイント `nft_issue_and_transfer` を呼び出し、ボブからアリスに転送し、メタデータの要求を要求します。
- NFT com `iroha_cli ledger nfts list --account <id>` の同等の SDK を検証し、転送および削除の指示を確認するためのデポジットを確認します。

## SDK 関係に関する情報

- [SDK Rust のクイックスタート](/sdks/rust)
- [SDK Python のクイックスタート](/sdks/python)
- [SDK JavaScript のクイックスタート](/sdks/javascript)

[バイシェ ア フォンテ Kotodama](/norito-snippets/nft-flow.ko)

```kotodama
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
    kotoage fn nft_issue_and_transfer() authorize("NftAuthority") {
        let owner = AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB");
        let nft = NftId::parse("n0$wonderland.universal");
        ledger::nft::mint(nft, owner);
        let to = AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76");
        ledger::nft::transfer(owner, nft, to);
        ledger::nft::set_metadata(nft, Name::parse("issued"), Json::parse("{\"issued\":\"demo\"}"));
        ledger::nft::burn(nft);
    }
}
```
