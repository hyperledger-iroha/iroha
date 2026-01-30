---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/norito/examples/nft-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2c5e845a26e736e3e8620964fb6e416b9b96ce50fd836dca1d51a2538bf0bf6a
source_last_modified: "2025-11-14T04:43:20.770595+00:00"
translation_last_reviewed: 2026-01-30
---

Recorre el ciclo de vida de un NFT de extremo a extremo: acuñación al propietario, transferencia, etiquetado de metadatos y quema.

## Recorrido del libro mayor

- Asegúrate de que exista la definición del NFT (por ejemplo `n0#wonderland`) junto con las cuentas de propietario/receptor usadas en el fragmento (`ih58...`, `ih58...`).
- Invoca el entrypoint `nft_issue_and_transfer` para acuñar el NFT, transferirlo de Alice a Bob y adjuntar una bandera de metadatos que describa la emisión.
- Inspecciona el estado del libro mayor de NFT con `iroha_cli ledger nfts list --account <id>` o los equivalentes del SDK para verificar la transferencia, luego confirma que el activo se elimina una vez que se ejecuta la instrucción de quema.

## Guías de SDK relacionadas

- [Quickstart del SDK de Rust](/sdks/rust)
- [Quickstart del SDK de Python](/sdks/python)
- [Quickstart del SDK de JavaScript](/sdks/javascript)

[Descarga la fuente de Kotodama](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("ih58...");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("ih58...");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```
