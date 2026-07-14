---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c00f9054efaa3e657b07033da99a6f6e700f7bad64325c2f1f6621b27469bef
source_last_modified: "2026-04-08T09:19:38.795735+00:00"
translation_last_reviewed: 2026-04-08
---

---
slug: /norito/examples/nft-flow
title: Выпустить, перевести и сжечь NFT
description: Проводит по жизненному циклу NFT от начала до конца: выпуск владельцу, перевод, добавление метаданных и сжигание.
source: crates/ivm/docs/examples/12_nft_flow.ko
---

Проводит по жизненному циклу NFT от начала до конца: выпуск владельцу, перевод, добавление метаданных и сжигание.

## Пошаговый обход реестра

- Убедитесь, что определение NFT (например `n0#wonderland`) существует вместе с аккаунтами владельца/получателя, используемыми в сниппете (`<i105-account-id>`, `<i105-account-id>`).
- Вызовите точку входа `nft_issue_and_transfer`, чтобы выпустить NFT, перевести его от Alice к Bob и прикрепить флаг метаданных, описывающий выпуск.
- Проверьте состояние NFT-реестра через `iroha ledger nft list all --verbose` или эквиваленты SDK, чтобы подтвердить перевод, затем убедитесь, что актив удаляется после выполнения инструкции burn.

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
