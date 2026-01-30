---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/norito/examples/nft-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5906fee104ca51f7f212a5f7a1dc326038545936c73be1daa7d0af73314b213b
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/nft-flow
title: Cunhar, transferir e queimar um NFT
description: Percorre o ciclo de vida de um NFT do inicio ao fim: cunhagem para o dono, transferencia, marcacao de metadados e queima.
source: crates/ivm/docs/examples/12_nft_flow.ko
---

Percorre o ciclo de vida de um NFT do inicio ao fim: cunhagem para o dono, transferencia, marcacao de metadados e queima.

## Roteiro do livro razao

- Garanta que a definicao do NFT (por exemplo `n0#wonderland`) exista junto com as contas de dono/destinatario usadas no trecho (`ih58...`, `ih58...`).
- Invoque o entrypoint `nft_issue_and_transfer` para cunhar o NFT, transferi-lo de Alice para Bob e anexar um sinal de metadados que descreva a emissao.
- Inspecione o estado do livro razao de NFT com `iroha_cli ledger nfts list --account <id>` ou os equivalentes do SDK para verificar a transferencia, depois confirme que o ativo e removido quando a instrucao de queima roda.

## Guias de SDK relacionados

- [Quickstart do SDK Rust](/sdks/rust)
- [Quickstart do SDK Python](/sdks/python)
- [Quickstart do SDK JavaScript](/sdks/javascript)

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
