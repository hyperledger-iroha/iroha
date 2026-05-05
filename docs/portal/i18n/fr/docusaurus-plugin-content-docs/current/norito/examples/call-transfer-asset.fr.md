---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug : /norito/examples/call-transfer-asset
title: Invoquer le transfert hôte depuis Kotodama
description : Démontrer comment un point d'entrée Kotodama peut appeler l'instruction hôte `transfer_asset` avec validation des métadonnées en ligne.
source : crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Démontrer comment un point d'entrée Kotodama peut appeler l'instruction hôte `transfer_asset` avec validation des métadonnées en ligne.

## Parcours du registre

- Approvisionnez l'autorité du contrat (par exemple `<i105-account-id>`) avec l'actif qu'elle transfère et accordez-lui le rôle `CanTransfer` ou une permission équivalente.
- Appelez le point d'entrée `call_transfer_asset` pour transférer 5 unités du compte du contrat vers `<i105-account-id>`, en reflétant la manière dont l'automatisation en chaîne peut encapsuler des appels hôte.
- Vérifiez les ventes via `FindAccountAssets` ou `iroha_cli ledger assets list --account <i105-account-id>` et inspectez les événements pour confirmer que la garde de métadonnées a journalisé le contexte du transfert.

## Guides SDK associés

- [SDK de démarrage rapide Rust](/sdks/rust)
- [SDK de démarrage rapide Python](/sdks/python)
- [Démarrage rapide SDK JavaScript](/sdks/javascript)

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