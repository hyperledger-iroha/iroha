---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
пуля: /norito/examples/nft-flow
Название: Frapper, Transferer et Brûler un NFT
описание: Parcourt le Cycle de vie d'un NFT de bout en bout: frappe au proprietaire, Transfert, ajout de métadonnées et Destruction.
источник: crates/ivm/docs/examples/12_nft_flow.ko
---

Парковый цикл жизни NFT в бою в бою: frappe au proprietaire, трансфер, метадоннеи и разрушение.

## Парк регистрации

- Убедитесь, что определение NFT (например, `n0#wonderland`) существует с использованием собственных/назначенных счетов в фрагменте (`<i105-account-id>`, `<i105-account-id>`).
- Вызовите точку входа `nft_issue_and_transfer` для перехвата NFT, передачи Алисы против Боба и присоединения указателя метадонных декривантов эмиссии.
- Проверьте состояние регистрации NFT с `iroha_cli ledger nfts list --account <id>` или эквивалентами SDK для проверки передачи, чтобы подтвердить, что действие подтверждено, что инструкция по записи выполняется.

## Руководства для партнеров SDK

- [Быстрый запуск SDK Rust](/sdks/rust)
- [Быстрый запуск SDK Python] (/sdks/python)
- [Быстрый запуск SDK JavaScript](/sdks/javascript)

[Зарядное устройство источника Kotodama](/norito-snippets/nft-flow.ko)

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
