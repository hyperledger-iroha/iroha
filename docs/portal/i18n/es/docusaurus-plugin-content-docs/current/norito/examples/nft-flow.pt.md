---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/ejemplos/nft-flow
título: Cunhar, transferir y quemar un NFT
descripción: Percorre o ciclo de vida de un NFT do inicio ao fim: cunhagem para o dono, transferencia, marcacao de metadados e queima.
fuente: crates/ivm/docs/examples/12_nft_flow.ko
---

Percorre el ciclo de vida de un NFT de inicio ao fim: cunhagem para o dono, transferencia, marcacao de metadados e queima.

## Roteiro do livro razao

- Garantía que a definicao do NFT (por ejemplo `n0#wonderland`) existe junto com as contas de dono/destinatario usadas no trecho (`<i105-account-id>`, `<i105-account-id>`).
- Invoque el punto de entrada `nft_issue_and_transfer` para Cunhar o NFT, transferi-lo de Alice para Bob y anexar una señal de metadados que descreva a emissao.
- Inspeccione el estado del libro razao de NFT con `iroha_cli ledger nfts list --account <id>` o sus equivalentes del SDK para verificar una transferencia, después de confirmar que o activo y eliminado cuando a instrucciones de queima roda.

## Guías de SDK relacionadas

- [Inicio rápido de SDK Rust](/sdks/rust)
- [Inicio rápido del SDK Python](/sdks/python)
- [Inicio rápido del SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/nft-flow.ko)

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