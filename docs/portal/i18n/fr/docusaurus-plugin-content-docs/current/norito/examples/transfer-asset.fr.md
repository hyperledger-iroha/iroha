---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug : /norito/examples/transfer-asset
titre : Transférer un actif entre comptes
description : Flux de transfert d'actifs simple qui reflète les quickstarts SDK et les parcours du registre.
source : exemples/transfert/transfer.ko
---

Flux de transfert d'actifs simple qui reflète les quickstarts SDK et les parcours du registre.

## Parcours du registre

- Préfinancez Alice avec l'actif cible (par exemple via le snippet `register and mint` ou les flux de quickstart SDK).
- Exécutez le point d'entrée `do_transfer` pour déplacer 10 unités d'Alice vers Bob, en satisfaisant la permission `AssetTransferRole`.
- Interrogez les ventes (`FindAccountAssets`, `iroha_cli ledger assets list`) ou abonnez-vous aux événements du pipeline pour observer le résultat du transfert.

## Guides SDK associés

- [SDK de démarrage rapide Rust](/sdks/rust)
- [SDK de démarrage rapide Python](/sdks/python)
- [Démarrage rapide SDK JavaScript](/sdks/javascript)

[Télécharger la source Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of rose#wonderland from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("i105..."),
      account!("i105..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```