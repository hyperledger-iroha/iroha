---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
limace : /norito/exemples/nft-flow
titre : Frapper, transférer et brûler un NFT
description : Parcourt le cycle de vie d'un NFT de bout en bout : frappe au propriétaire, transfert, ajout de métadonnées et destruction.
source : crates/ivm/docs/examples/12_nft_flow.ko
---

Parcourt le cycle de vie d'un NFT de bout en bout : frappe au propriétaire, transfert, ajout de métadonnées et destruction.

## Parcours du registre

- Assurez-vous que la définition du NFT (par exemple `n0#wonderland`) existe avec les comptes propriétaire/destinataire utilisés dans le snippet (`i105...`, `i105...`).
- Invoquez le point d'entrée `nft_issue_and_transfer` pour frapper le NFT, le transfert d'Alice vers Bob et attacher un indicateur de métadonnées décrivant l'émission.
- Inspectez l'état du registre NFT avec `iroha_cli ledger nfts list --account <id>` ou les équivalents SDK pour vérifier le transfert, puis confirmez que l'actif est supprimé une fois que l'instruction de burn s'exécute.

## Guides SDK associés

- [SDK de démarrage rapide Rust](/sdks/rust)
- [SDK de démarrage rapide Python](/sdks/python)
- [Démarrage rapide SDK JavaScript](/sdks/javascript)

[Télécharger la source Kotodama](/norito-snippets/nft-flow.ko)

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