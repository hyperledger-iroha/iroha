---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
пуля: /norito/examples/nft-flow
титул: Сэнсэй Уоррен NFT
Описание: Создано в NFT в честь события: السك للمالك، النقل, ووسم بيانات التعريف, والحرق.
источник: crates/ivm/docs/examples/12_nft_flow.ko
---

Создан для NFT в Новом году: Стоун-Луи, Нью-Йорк, Нью-Йорк التعريف, والحرق.

## جولة دفتر الأستاذ

- Создано для NFT (с кодом `n0#wonderland`) и используется для восстановления/разгрузки. Установите флажок (`<i105-account-id>`, `<i105-account-id>`).
- استدعِ نقطة الدخول `nft_issue_and_transfer` لسك NFT ونقله من من من إلى Bob علامة بيانات تعريف تصف الإصدار.
- Создайте NFT-файл `iroha_cli ledger nfts list --account <id>` и установите SDK в нужном месте. Он сказал, что хочет сделать это.

## Использование SDK

- [Загрузка в Rust SDK](/sdks/rust)
- [Просмотр Python SDK](/sdks/python)
- [Загрузка JavaScript SDK](/sdks/javascript)

[نزّل مصدر Kotodama](/norito-snippets/nft-flow.ko)

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
