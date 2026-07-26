---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
スラグ: /norito/examples/nft-flow
タイトル: Выпустить、перевести и сжечь NFT
説明: Проводит по жизненному циклу NFT от начала до конца: выпуск владельцу, перевод, добавление метаданных иそうですね。
ソース: crates/ivm/docs/examples/12_nft_flow.ko
---

Проводит по жизненному циклу NFT от начала до конца: выпуск владельцу, перевод, добавление метаданных иそうですね。

## Полаговый обход рестра

- Убедитесь、что определение NFT (например `n0#wonderland`) существует вместе с аккаунтами владельца/получателя, (`<i105-account-id>`、`<i105-account-id>`) を参照してください。
- Вызовите точку входа `nft_issue_and_transfer`、NFT のメッセージ、Alice と Bob のメッセージ、 описывающий выпуск。
- NFT-рестра через `iroha_cli ledger nfts list --account <id>` または эквиваленты SDK、чтобы подтвердить перевод、затем убедитесь、燃えます。

## Связанные руководства SDK

- [クイックスタート Rust SDK](/sdks/rust)
- [クイックスタート Python SDK](/sdks/python)
- [クイックスタート JavaScript SDK](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/nft-flow.ko)

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
