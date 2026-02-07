---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug : /norito/examples/call-transfer-asset
titre : Recherchez votre hôte à partir de Kotodama
description: Показывает, как точка входа Kotodama может вызвать инструкцию хоста `transfer_asset` с встроенной проверкой метаданных.
source : crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Il est possible que, pour votre Kotodama, vous puissiez utiliser les instructions de l'hôte `transfer_asset` selon les méthodes officielles.

## Пошаговый обход реестра

- Activez le contrat de maintenance (par exemple `ih58...`) en cliquant sur le bouton de commande et activez le rôle de maintenance `CanTransfer`. ou une résolution équivalente.
- Vous avez besoin de votre `call_transfer_asset` pour avoir 5 éditions de contrat de compte sur `ih58...`, par exemple L'automation peut alors s'occuper de votre hôte.
- Vérifiez les soldes de `FindAccountAssets` ou `iroha_cli ledger assets list --account ih58...` et activez le système qui met à jour le système de garde du contexte. avant.

## SDK de démarrage rapide

- [SDK de démarrage rapide Rust](/sdks/rust)
- [SDK Python de démarrage rapide](/sdks/python)
- [SDK JavaScript de démarrage rapide](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/call-transfer-asset.ko)

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