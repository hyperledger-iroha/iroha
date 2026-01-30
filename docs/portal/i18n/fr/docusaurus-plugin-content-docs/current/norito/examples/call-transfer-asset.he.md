---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f6c486027b7d6f7239bc0209b67bc8d2504f773c83da00317cd473ddb7fa5429
source_last_modified: "2025-11-14T04:43:20.645684+00:00"
translation_last_reviewed: 2026-01-30
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
