---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/ejemplos/nft-flow
título: Frapper, transférer et brûler un NFT
descripción: Parcourt le Cycle de vie d'un NFT de bout en bout: frappe au propriétaire, transfert, ajout de métadonnées et destrucción.
fuente: crates/ivm/docs/examples/12_nft_flow.ko
---

Parcourt le ciclo de vida de un NFT de combate en combate: frappe au propriétaire, transfert, ajout de métadonnées et destrucción.

## Rutas del registro

- Asegúrese de que la definición de NFT (por ejemplo, `n0#wonderland`) existe con las cuentas propietarias/destinatarias utilizadas en el fragmento (`<i105-account-id>`, `<i105-account-id>`).
- Invoquez le point d'entrée `nft_issue_and_transfer` pour frapper le NFT, le transférer d'Alice vers Bob y adjuntar un indicador de metadonnées decrivant l'émission.
- Inspeccione el estado del registro NFT con `iroha_cli ledger nfts list --account <id>` o los equivalentes SDK para verificar la transferencia, luego confirme que la actividad está suprimida una vez que la instrucción de grabación se ejecuta.

## Guías SDK asociadas

- [Inicio rápido SDK Rust](/sdks/rust)
- [Inicio rápido SDK Python](/sdks/python)
- [Inicio rápido SDK JavaScript](/sdks/javascript)

[Descargar la fuente Kotodama](/norito-snippets/nft-flow.ko)

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