---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ca547bc673684a35eaa1ba6d2d96616bcc453a0baa5ece4c8fd0a629deb1ca84
source_last_modified: "2026-01-22T15:55:01+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/call-transfer-asset
title: Invoquer le transfert hôte depuis Kotodama
description: Démontre comment un point d'entrée Kotodama peut appeler l'instruction hôte `transfer_asset` avec validation des métadonnées en ligne.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Démontre comment un point d'entrée Kotodama peut appeler l'instruction hôte `transfer_asset` avec validation des métadonnées en ligne.

## Parcours du registre

- Approvisionnez l'autorité du contrat (par exemple `ih58...`) avec l'actif qu'elle transférera et accordez-lui le rôle `CanTransfer` ou une permission équivalente.
- Appelez le point d'entrée `call_transfer_asset` pour transférer 5 unités du compte du contrat vers `ih58...`, en reflétant la manière dont l'automatisation on-chain peut encapsuler des appels hôte.
- Vérifiez les soldes via `FindAccountAssets` ou `iroha_cli ledger assets list --account ih58...` et inspectez les événements pour confirmer que le garde de métadonnées a journalisé le contexte du transfert.

## Guides SDK associés

- [Quickstart SDK Rust](/sdks/rust)
- [Quickstart SDK Python](/sdks/python)
- [Quickstart SDK JavaScript](/sdks/javascript)

[Télécharger la source Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("ih58..."),
      account!("ih58..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```
