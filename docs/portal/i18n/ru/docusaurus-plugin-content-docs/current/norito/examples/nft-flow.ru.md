---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
пуля: /norito/examples/nft-flow
title: Выпустить, перевести и сжечь NFT
описание: Проводит по жизненному циклу NFT от начала до конца: выпуск владельца, перевод, добавление метаданных и сжигание.
источник: crates/ivm/docs/examples/12_nft_flow.ko
---

Проводит жизненный цикл NFT от начала до конца: выпуск владельца, перевод, добавление метаданных и сжигание.

## Пошаговый обход реестра

- Убедитесь, что определение NFT (например, `n0#wonderland`) существует вместе с аккаунтами владельца/получателя, используемыми в фрагменте (`<i105-account-id>`, `<i105-account-id>`).
- Вызовите точку входа `nft_issue_and_transfer`, чтобы выпустить NFT, переведите его от Алисы к Бобу и прикрепите флаг метаданных, описывающий выпуск.
- Проверьте состояние NFT-реестра через `iroha_cli ledger nfts list --account <id>` или эквиваленты SDK, чтобы обеспечить надежность перевода, а затем убедитесь, что активы удаляются после выполнения инструкции записи.

## Связанные управления SDK

- [Быстрый запуск Rust SDK](/sdks/rust)
- [Quickstart Python SDK](/sdks/python)
- [Быстрый запуск JavaScript SDK](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/nft-flow.ko)

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