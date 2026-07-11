---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/norito/examples/nft-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 66e6f2298a1b8bcf752374482c6a5d6bb3774929dc1dc6793345eb44b7d3b797
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/nft-flow
title: Выпустить, перевести и сжечь NFT
description: Проводит по жизненному циклу NFT от начала до конца: выпуск владельцу, перевод, добавление метаданных и сжигание.
source: crates/ivm/docs/examples/12_nft_flow.ko
---

Проводит по жизненному циклу NFT от начала до конца: выпуск владельцу, перевод, добавление метаданных и сжигание.

## Пошаговый обход реестра

- Убедитесь, что определение NFT (например `n0#wonderland`) существует вместе с аккаунтами владельца/получателя, используемыми в сниппете (`<i105-account-id>`, `<i105-account-id>`).
- Вызовите точку входа `nft_issue_and_transfer`, чтобы выпустить NFT, перевести его от Alice к Bob и прикрепить флаг метаданных, описывающий выпуск.
- Проверьте состояние NFT-реестра через `iroha_cli ledger nfts list --account <id>` или эквиваленты SDK, чтобы подтвердить перевод, затем убедитесь, что актив удаляется после выполнения инструкции burn.

## Связанные руководства SDK

- [Quickstart Rust SDK](/sdks/rust)
- [Quickstart Python SDK](/sdks/python)
- [Quickstart JavaScript SDK](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/nft-flow.ko)

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
