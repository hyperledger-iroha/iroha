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

- NFT (`n0#wonderland`) を使用して、NFT を使用してください。 (`<i105-account-id>`、`<i105-account-id>`)。
- `nft_issue_and_transfer` と NFT とアリスとボブとの接続ああ。
- セキュリティ NFT セキュリティ `iroha_cli ledger nfts list --account <id>` セキュリティ SDK セキュリティ セキュリティ セキュリティテストを実行してください。

## SDK の開発

- [Rust SDK](/sdks/rust)
- [Python SDK](/sdks/python)
- [JavaScript SDK](/sdks/javascript)

[Kotodama](/norito-snippets/nft-flow.ko)

```kotodama
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
    kotoage fn nft_issue_and_transfer() authorize("NftAuthority") {
        let owner = AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV", );
        let nft = NftId::parse("n0$wonderland.universal");
        ledger::nft::mint(nft, owner);
        let to = AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76", );
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
