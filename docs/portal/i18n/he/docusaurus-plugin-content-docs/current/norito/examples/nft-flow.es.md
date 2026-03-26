---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/nft-flow.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/nft-flow
כותרת: Acuñar, transferir y quemar un NFT
תיאור: Recorre el ciclo de vida de un NFT de extremo a extremo: acuñación al propietario, transferencia, etiquetado de metadatos y quema.
מקור: crates/ivm/docs/examples/12_nft_flow.ko
---

Recorre el ciclo de vida de un NFT de extremo a extremo: acuñación al propietario, transferencia, etiquetado de metadatos y quema.

## ראש העיר Recorrido del libro

- Asegúrate de que exista la definición del NFT (for ejemplo `n0#wonderland`) junto con las cuentas de propietario/receptor usadas en el fragmento (`<i105-account-id>`, `<i105-account-id>`).
- Invoca el entrypoint `nft_issue_and_transfer` para acuñar el NFT, transferirlo de Alice and Bob y adjuntar una bandera de metadatos que scriba la emisión.
- Inspecciona el estado del libro Mayor de NFT con `iroha_cli ledger nfts list --account <id>` o los equivalentes del SDK para verificar la transferencia, luego confirma que el activo se elimina una vez que se ejecuta la instrucción de quema.

## Guías de SDK relacionadas

- [Quickstart del SDK de Rust](/sdks/rust)
- [Quickstart del SDK de Python](/sdks/python)
- [Quickstart del SDK de JavaScript](/sdks/javascript)

[Descarga la fuente de Kotodama](/norito-snippets/nft-flow.ko)

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