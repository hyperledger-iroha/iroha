---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug : /norito/examples/transfer-asset
titre : Transférer actif entre comptes
description : Flux direct de transfert d'actifs qui reflètent les démarrages rapides du SDK et les enregistrements du livre principal.
source : exemples/transfert/transfer.ko
---

Flux direct de transfert d'actifs qui reflètent les démarrages rapides du SDK et les enregistrements du livre principal.

## Recorrido del libro mayor

- Pré-fondez Alice avec l'objet actif (par exemple entre le fragment `register and mint` ou les flux de démarrage rapide du SDK).
- Exécutez le point d'entrée `do_transfer` pour déplacer 10 unités d'Alice à Bob, en complétant l'autorisation `AssetTransferRole`.
- Consultez les soldes (`FindAccountAssets`, `iroha_cli ledger assets list`) ou abonnez-vous aux événements du pipeline pour observer le résultat du transfert.

## Guides relatifs au SDK

- [Démarrage rapide du SDK de Rust](/sdks/rust)
- [Démarrage rapide du SDK de Python](/sdks/python)
- [Démarrage rapide du SDK de JavaScript](/sdks/javascript)

[Télécharger la source de Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of 62Fk4FPcMuLvW5QjDGNF2a4jAmjM from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("soraカタカナ..."),
      account!("soraカタカナ..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```