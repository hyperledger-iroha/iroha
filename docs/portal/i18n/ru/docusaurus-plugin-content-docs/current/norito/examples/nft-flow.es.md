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

- Убедитесь, что определение NFT (например, `n0#wonderland`) существует вместе с используемыми объектами владельца/рецептора на фрагменте (`<i105-account-id>`, `<i105-account-id>`).
- Вызовите точку входа `nft_issue_and_transfer`, чтобы включить NFT, передать Алису Бобу и добавить группу метаданных для описания выбросов.
- Проверьте состояние библиотеки NFT с номером `iroha_cli ledger nfts list --account <id>` или эквивалентами SDK для проверки передачи, а затем подтвердите, что активация устраняет то, что выдает инструкция по этому вопросу.

## Руководство по настройке SDK

- [Краткий запуск SDK Rust](/sdks/rust)
- [Краткий запуск SDK Python] (/sdks/python)
- [Краткий запуск SDK JavaScript](/sdks/javascript)

[Удалить ссылку Kotodama](/norito-snippets/nft-flow.ko)

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
