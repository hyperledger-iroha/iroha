---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
limace : /norito/exemples/nft-flow
titre : Cunhar, transférer et faire un NFT
description : Percorre le cycle de vie d'un NFT au début du processus : gestion du don, du transfert, du marché des métadonnées et de l'argent.
source : crates/ivm/docs/examples/12_nft_flow.ko
---

Percorre le cycle de vie d'un NFT au début du programme : gestion du don, du transfert, du marché des métadonnées et de l'argent.

## Roteiro do livro razão

- Garantit que la définition du NFT (par exemple `n0#wonderland`) existe avec les données de don/destination utilisées pas (`soraカタカナ...`, `soraカタカナ...`).
- Appelez le point d'entrée `nft_issue_and_transfer` pour trouver NFT, transférez Alice à Bob et ajoutez le signal des métadonnées qui sont affichées à l'émission.
- Inspectez l'état du livre de NFT avec `iroha_cli ledger nfts list --account <id>` ou les équivalents du SDK pour vérifier le transfert, après avoir confirmé que l'activité est supprimée lorsque l'instruction de cette opération est effectuée.

## Guides des utilisateurs du SDK

- [Démarrage rapide du SDK Rust](/sdks/rust)
- [Démarrage rapide du SDK Python](/sdks/python)
- [Démarrage rapide du SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("soraカタカナ...");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("soraカタカナ...");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```