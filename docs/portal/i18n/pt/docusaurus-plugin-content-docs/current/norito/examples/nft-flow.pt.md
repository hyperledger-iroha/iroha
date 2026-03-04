---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/nft-flow
título: Cunhar, transferir e queimar um NFT
description: Percorre o ciclo de vida de um NFT do inicio ao fim: cunhagem para o dono, transferência, marcação de metadados e queima.
fonte: crates/ivm/docs/examples/12_nft_flow.ko
---

Percorre o ciclo de vida de um NFT do início ao fim: cunhagem para o dono, transferência, marcação de metadados e queima.

## Roteiro do livro razão

- Garanta que a definição do NFT (por exemplo `n0#wonderland`) exista junto com as contas de dono/destinatário usadas no trecho (`ih58...`, `ih58...`).
- Invoque o ponto de entrada `nft_issue_and_transfer` para cunhar o NFT, transfira-o de Alice para Bob e fixe um sinal de metadados que descreva a emissão.
- Inspecione o estado do livro razão de NFT com `iroha_cli ledger nfts list --account <id>` ou os equivalentes do SDK para verificar a transferência, depois confirme que o ativo e removido quando a instrução de queima de roda.

## Guias de SDK relacionados

- [Início rápido do SDK Rust](/sdks/rust)
- [Início rápido do SDK Python](/sdks/python)
- [Início rápido do SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/nft-flow.ko)

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