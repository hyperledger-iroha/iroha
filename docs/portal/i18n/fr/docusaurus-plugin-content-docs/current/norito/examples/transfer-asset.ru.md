---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug : /norito/examples/transfer-asset
titre : Перевести актив между аккаунтами
description : Voici les principales activités du scénario, le SDK de démarrage rapide et la procédure pas à pas.
source : exemples/transfert/transfer.ko
---

Les activités précédentes du scénario incluent le SDK de démarrage rapide et la procédure pas à pas.

## Пошаговый обход реестра

- Vous devez d'abord télécharger Alice comme activité (comme par exemple à partir du extrait `register and mint` ou du SDK de démarrage rapide).
- Vous avez choisi `do_transfer`, qui a écrit 10 éditions d'Alice et Bob, en passant par `AssetTransferRole`.
- Vérifiez les équilibres (`FindAccountAssets`, `iroha_cli ledger assets list`) ou installez-vous sur le pipeline pour obtenir les résultats souhaités.

## SDK de démarrage rapide

- [SDK de démarrage rapide Rust](/sdks/rust)
- [SDK Python de démarrage rapide](/sdks/python)
- [SDK JavaScript de démarrage rapide](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of 62Fk4FPcMuLvW5QjDGNF2a4jAmjM from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("<katakana-i105-account-id>"),
      account!("<katakana-i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```