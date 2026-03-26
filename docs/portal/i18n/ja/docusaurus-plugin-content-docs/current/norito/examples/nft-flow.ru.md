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

- Убедитесь、что определение NFT (например `n0#wonderland`) существует вместе с аккаунтами владельца/получателя, (`i105...`、`i105...`) を参照してください。
- Вызовите точку входа `nft_issue_and_transfer`、NFT のメッセージ、Alice と Bob のメッセージ、 описывающий выпуск。
- NFT-рестра через `iroha_cli ledger nfts list --account <id>` または эквиваленты SDK、чтобы подтвердить перевод、затем убедитесь、燃えます。

## Связанные руководства SDK

- [クイックスタート Rust SDK](/sdks/rust)
- [クイックスタート Python SDK](/sdks/python)
- [クイックスタート JavaScript SDK](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/nft-flow.ko)

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