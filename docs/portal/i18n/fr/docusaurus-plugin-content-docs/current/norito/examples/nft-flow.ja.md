---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/norito/examples/nft-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c4a9ef6aabe9bb6a4f59c89bb903b711bf1fa11c69f3255dc1e88da099c7b2a8
source_last_modified: "2025-11-14T04:43:20.771294+00:00"
translation_last_reviewed: 2026-01-30
---

Parcourt le cycle de vie d'un NFT de bout en bout : frappe au propriétaire, transfert, ajout de métadonnées et destruction.

## Parcours du registre

- Assurez-vous que la définition du NFT (par exemple `n0#wonderland`) existe avec les comptes propriétaire/destinataire utilisés dans le snippet (`ih58...`, `ih58...`).
- Invoquez le point d'entrée `nft_issue_and_transfer` pour frapper le NFT, le transférer d'Alice vers Bob et attacher un indicateur de métadonnées décrivant l'émission.
- Inspectez l'état du registre NFT avec `iroha_cli ledger nfts list --account <id>` ou les équivalents SDK pour vérifier le transfert, puis confirmez que l'actif est supprimé une fois que l'instruction de burn s'exécute.

## Guides SDK associés

- [Quickstart SDK Rust](/sdks/rust)
- [Quickstart SDK Python](/sdks/python)
- [Quickstart SDK JavaScript](/sdks/javascript)

[Télécharger la source Kotodama](/norito-snippets/nft-flow.ko)

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
