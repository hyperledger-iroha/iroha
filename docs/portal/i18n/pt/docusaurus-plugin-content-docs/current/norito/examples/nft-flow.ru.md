---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/nft-flow
título: Выпустить, перевести e сжечь NFT
description: Проводит по жизненному циклу NFT от начала до конца: выпуск владельцу, перевод, добавление метаданных и сжигание.
fonte: crates/ivm/docs/examples/12_nft_flow.ko
---

Проводит по жизненному циклу NFT от начала до конца: выпуск владельцу, перевод, добавление метаданных и сжигание.

## Пошаговый обход реестра

- Убедитесь, что определение NFT (por exemplo `n0#wonderland`) существует вместе аккаунтами владельца/получателя, usado no trecho (`<i105-account-id>`, `<i105-account-id>`).
- Вызовите точку входа `nft_issue_and_transfer`, чтобы выпустить NFT, перевести его от Alice к Bob e прикрепить флаг метаданных, описывающий выпуск.
- Verifique a configuração NFT do `iroha_cli ledger nfts list --account <id>` ou SDK brilhante, isso pode ser feito por meio de download убедитесь, что актив удаляется после выполнения инструкции queimar.

## Como usar o SDK

- [Início rápido Rust SDK](/sdks/rust)
- [Início rápido do SDK do Python](/sdks/python)
- [SDK JavaScript de início rápido](/sdks/javascript)

[Kotodama](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("<i105-account-id>");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("<i105-account-id>");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```