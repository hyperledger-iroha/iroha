---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 41a944c3e016d0dc96a0edb3559700670a7bd57b437751a777df8b35567b34fb
source_last_modified: "2025-11-23T15:30:33.687691+00:00"
translation_last_reviewed: 2026-01-30
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
- Проверьте состояние NFT-реестра через `iroha_cli ledger nfts list --account <id>` или эквиваленты SDK, чтобы подтвердить перевод, затем убедитесь, что актив удаляется после выполнения инструкции burn.

## Связанные руководства SDK

- [Quickstart Rust SDK](/sdks/rust)
- [Quickstart Python SDK](/sdks/python)
- [Quickstart JavaScript SDK](/sdks/javascript)

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
