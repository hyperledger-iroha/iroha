---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c00f9054efaa3e657b07033da99a6f6e700f7bad64325c2f1f6621b27469bef
source_last_modified: "2026-04-08T09:19:38.795735+00:00"
translation_last_reviewed: 2026-04-08
---

---
slug: /norito/examples/nft-flow
title: Frapper, transférer et brûler un NFT
description: Parcourt le cycle de vie d'un NFT de bout en bout : frappe au propriétaire, transfert, ajout de métadonnées et destruction.
source: crates/ivm/docs/examples/12_nft_flow.ko
---

Parcourt le cycle de vie d'un NFT de bout en bout : frappe au propriétaire, transfert, ajout de métadonnées et destruction.

## Parcours du registre

- Assurez-vous que la définition du NFT (par exemple `n0#wonderland`) existe avec les comptes propriétaire/destinataire utilisés dans le snippet (`<i105-account-id>`, `<i105-account-id>`).
- Invoquez le point d'entrée `nft_issue_and_transfer` pour frapper le NFT, le transférer d'Alice vers Bob et attacher un indicateur de métadonnées décrivant l'émission.
- Inspectez l'état du registre NFT avec `iroha ledger nft list all --verbose` ou les équivalents SDK pour vérifier le transfert, puis confirmez que l'actif est supprimé une fois que l'instruction de burn s'exécute.

## Guides SDK associés

- [Quickstart SDK Rust](/sdks/rust)
- [Quickstart SDK Python](/sdks/python)
- [Quickstart SDK JavaScript](/sdks/javascript)

[Télécharger la source Kotodama](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("<i105-account-id>");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("<i105-account-id>");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, name!("issued"), json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```
