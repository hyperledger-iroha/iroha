---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/nft-flow
título: Frapper, transferir e quebrar um NFT
descrição: Parcourt o ciclo de vida de um NFT de luta em luta: frappe au propriétaire, transfert, ajout de métadonnées et destruição.
fonte: crates/ivm/docs/examples/12_nft_flow.ko
---

Parcourt o ciclo de vida de um NFT de luta em luta: frappe au propriétaire, transfert, ajout de métadonnées et destruição.

## Parcours du registre

- Certifique-se de que a definição de NFT (por exemplo, `n0#wonderland`) existe com as contas proprietárias/destinatárias utilizadas no snippet (`ih58...`, `ih58...`).
- Invoque o ponto de entrada `nft_issue_and_transfer` para frapper o NFT, transfira Alice para Bob e anexe um indicador de metadonées que descreva a emissão.
- Inspecione o estado do registro NFT com `iroha_cli ledger nfts list --account <id>` ou os SDKs equivalentes para verificar a transferência e depois confirme se o ativo foi suprimido quando a instrução de gravação foi executada.

## Guias SDK associados

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