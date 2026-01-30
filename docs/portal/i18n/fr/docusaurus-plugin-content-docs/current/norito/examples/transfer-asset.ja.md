---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a2fbdfc5be37616559849fc023bd2627a373b591ea9fea21be2b00168144a16
source_last_modified: "2025-11-14T04:43:20.812073+00:00"
translation_last_reviewed: 2026-01-30
---

Flux de transfert d'actifs simple qui reflète les quickstarts SDK et les parcours du registre.

## Parcours du registre

- Préfinancez Alice avec l'actif cible (par exemple via le snippet `register and mint` ou les flux de quickstart SDK).
- Exécutez le point d'entrée `do_transfer` pour déplacer 10 unités d'Alice vers Bob, en satisfaisant la permission `AssetTransferRole`.
- Interrogez les soldes (`FindAccountAssets`, `iroha_cli ledger assets list`) ou abonnez-vous aux événements du pipeline pour observer le résultat du transfert.

## Guides SDK associés

- [Quickstart SDK Rust](/sdks/rust)
- [Quickstart SDK Python](/sdks/python)
- [Quickstart SDK JavaScript](/sdks/javascript)

[Télécharger la source Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of rose#wonderland from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("ih58..."),
      account!("ih58..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```
