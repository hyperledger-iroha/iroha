---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a91fc8841580a836c80129942df7f79f5bc5dd5f6a72dccf1394b740d02536a5
source_last_modified: "2025-11-23T15:30:33.687233+00:00"
translation_last_reviewed: 2026-01-30
---

---
slug: /norito/examples/call-transfer-asset
title: Invoquer le transfert hôte depuis Kotodama
description: Démontre comment un point d'entrée Kotodama peut appeler l'instruction hôte `transfer_asset` avec validation des métadonnées en ligne.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Démontre comment un point d'entrée Kotodama peut appeler l'instruction hôte `transfer_asset` avec validation des métadonnées en ligne.

## Parcours du registre

- Approvisionnez l'autorité du contrat (par exemple `<i105-account-id>`) avec l'actif qu'elle transférera et accordez-lui le rôle `CanTransfer` ou une permission équivalente.
- Appelez le point d'entrée `call_transfer_asset` pour transférer 5 unités du compte du contrat vers `<i105-account-id>`, en reflétant la manière dont l'automatisation on-chain peut encapsuler des appels hôte.
- Vérifiez les soldes via `FindAccountAssets` ou `iroha_cli ledger assets list --account <i105-account-id>` et inspectez les événements pour confirmer que le garde de métadonnées a journalisé le contexte du transfert.

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
      account!("<i105-account-id>"),
      account!("<i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```
