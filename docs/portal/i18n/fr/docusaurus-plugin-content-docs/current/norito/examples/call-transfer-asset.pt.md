---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug : /norito/examples/call-transfer-asset
titre : Invoquer le transfert de l'hôte à partir de Kotodama
description : La démonstration du point d'entrée Kotodama peut indiquer les instructions de l'hôte `transfer_asset` avec la validation en ligne des métadonnées.
source : crates/ivm/docs/examples/08_call_transfer_asset.ko
---

En montrant le point d'entrée Kotodama, vous pouvez demander aux instructions de l'hôte `transfer_asset` de valider les métadonnées en ligne.

## Roteiro do livro razão

- Financer l'autorisation du contrat (par exemple `i105...`) avec l'objectif du transfert et la concession de l'autorisation du papier `CanTransfer` ou autorisation équivalente.
- Choisissez le point d'entrée `call_transfer_asset` pour transférer 5 unités du contrat pour `i105...`, reflétant la possibilité d'envoyer automatiquement des commandes en chaîne à l'hôte.
- Vérifiez les saldos via `FindAccountAssets` ou `iroha_cli ledger assets list --account i105...` et inspectez les événements pour confirmer que le garde de métadonnées enregistré ou le contexte du transfert.

## Guides des utilisateurs du SDK

- [Démarrage rapide du SDK Rust](/sdks/rust)
- [Démarrage rapide du SDK Python](/sdks/python)
- [Démarrage rapide du SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("i105..."),
      account!("i105..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```