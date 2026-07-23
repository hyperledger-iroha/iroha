---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/nft-flow
כותרת: Выпустить, перевести и сжечь NFT
תיאור: Проводит по жизненному циклу NFT от начала до конца: выпуск владельцу, перевод, добавление метадианс.
מקור: crates/ivm/docs/examples/12_nft_flow.ko
---

Проводит по жизненному циклу NFT от начала до конца: выпуск владельцу, перевод, добавление метадиансы.

## Пошаговый обход реестра

- Убедитесь, что определение NFT (נוסח `n0#wonderland`) существует вместе с аккаунтами владельца/полич, в сниппете (`<i105-account-id>`, `<i105-account-id>`).
- צור קשר עם `nft_issue_and_transfer`, משתמש ב-NFT, פועל גם עם אליס עם בוב ו-Prикрепить флаг мет описывающий выпуск.
- התקן את ה-NFT-reestra через `iroha_cli ledger nfts list --account <id>` או эквиваленты SDK. удаляется после выполнения инструкции לשרוף.

## Связанные руководства SDK

- [Quickstart Rust SDK](/sdks/rust)
- [Quickstart Python SDK](/sdks/python)
- [Quickstart JavaScript SDK](/sdks/javascript)

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
