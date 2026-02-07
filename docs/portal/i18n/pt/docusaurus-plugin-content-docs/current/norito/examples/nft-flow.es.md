---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/nft-flow
título: Acuñar, transferir e querer um NFT
descrição: Repita o ciclo de vida de um NFT de extremo a extremo: conhecimento do proprietário, transferência, etiqueta de metadados e pergunta.
fonte: crates/ivm/docs/examples/12_nft_flow.ko
---

Repita o ciclo de vida de um NFT de extremo a extremo: conhecimento do proprietário, transferência, etiqueta de metadados e pergunta.

## Recorrido do livro prefeito

- Certifique-se de que existe a definição do NFT (por exemplo, `n0#wonderland`) junto com as informações do proprietário/receptor usadas no fragmento (`ih58...`, `ih58...`).
- Invoque o ponto de entrada `nft_issue_and_transfer` para obter o NFT, transfira-o de Alice para Bob e adicione uma faixa de metadados que descreve a emissão.
- Inspecione o estado do livro maior de NFT com `iroha_cli ledger nfts list --account <id>` ou os equivalentes do SDK para verificar a transferência, depois confirme que o ativo foi eliminado uma vez que a instrução de pergunta foi executada.

## Guias do SDK relacionados

- [Início rápido do SDK de Rust](/sdks/rust)
- [Início rápido do SDK de Python](/sdks/python)
- [Início rápido do SDK de JavaScript](/sdks/javascript)

[Descarregue a fonte de Kotodama](/norito-snippets/nft-flow.ko)

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