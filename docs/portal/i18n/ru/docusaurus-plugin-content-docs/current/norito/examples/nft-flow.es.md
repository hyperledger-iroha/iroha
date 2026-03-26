---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
пуля: /norito/examples/nft-flow
Название: Acuñar, Transferir y quemar un NFT
описание: Восстановить цикл жизни NFT в крайнем и крайнем случае: понимание собственности, передача, этикет метаданных и quema.
источник: crates/ivm/docs/examples/12_nft_flow.ko
---

Восстановите цикл жизни NFT в крайнем и крайнем случае: ознакомление с собственностью, перенос, этикет метаданных и quema.

## Запись мэра библиотеки

- Убедитесь, что определение NFT (например, `n0#wonderland`) существует вместе с используемыми объектами владельца/рецептора на фрагменте (`<katakana-i105-account-id>`, `<katakana-i105-account-id>`).
- Вызовите точку входа `nft_issue_and_transfer`, чтобы включить NFT, передать Алису Бобу и добавить группу метаданных для описания выбросов.
- Проверьте состояние библиотеки NFT с номером `iroha_cli ledger nfts list --account <id>` или эквивалентами SDK для проверки передачи, а затем подтвердите, что активация устраняет то, что выдает инструкция по этому вопросу.

## Руководство по настройке SDK

- [Краткий запуск SDK Rust](/sdks/rust)
- [Краткий запуск SDK Python] (/sdks/python)
- [Краткий запуск SDK JavaScript](/sdks/javascript)

[Удалить ссылку Kotodama](/norito-snippets/nft-flow.ko)

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