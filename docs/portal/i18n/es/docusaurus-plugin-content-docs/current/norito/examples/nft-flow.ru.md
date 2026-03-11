---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/ejemplos/nft-flow
título: Выпустить, перевести и сжечь NFT
descripción: Проводит по жизненному циклу NFT от начала до конца: выпуск владельцу, перевод, добавление метаданных и сжигание.
fuente: crates/ivm/docs/examples/12_nft_flow.ko
---

Проводит по жизненному циклу NFT от начала до конца: выпуск владельцу, перевод, добавление метаданных и сжигание.

## Пошаговый обход реестра

- Tenga en cuenta que la propiedad NFT (nombre `n0#wonderland`) está conectada a cuentas/cuentas, используемыми в сниппете (`i105...`, `i105...`).
- Utilice el archivo `nft_issue_and_transfer`, para descargar NFT, para proteger el ego de Alice y Bob y activar la bandera metadana, описывающий выпуск.
- Guarde el archivo NFT con `iroha_cli ledger nfts list --account <id>` o SDK emergente, para poder descargarlo previamente, según sea necesario. актив удаляется после выполнения инструкции burn.

## Связанные руководства SDK

- [SDK de inicio rápido de Rust](/sdks/rust)
- [SDK de Python de inicio rápido](/sdks/python)
- [SDK de JavaScript de inicio rápido](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("i105...");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("i105...");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```