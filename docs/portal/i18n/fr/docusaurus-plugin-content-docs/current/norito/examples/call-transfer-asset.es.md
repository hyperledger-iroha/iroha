---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug : /norito/examples/call-transfer-asset
titre : Invoquer le transfert de l'hôte à partir de Kotodama
description : Vous pouvez voir comment un point d'entrée Kotodama peut appeler les instructions de l'hôte `transfer_asset` avec la validation des métadonnées en ligne.
source : crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Vous verrez comment un point d'entrée Kotodama peut appeler les instructions de l'hôte `transfer_asset` avec la validation des métadonnées en ligne.

## Recorrido del libro mayor

- Fondez l'autorité du contrat (par exemple `<i105-account-id>`) avec l'actif qui transfère et otorgale le rôle `CanTransfer` ou un permis équivalent.
- Appelez le point d'entrée `call_transfer_asset` pour transférer 5 unités du compte du contrat vers `<i105-account-id>`, en réfléchissant au formulaire dans lequel l'automatisation en chaîne peut affecter les appels de l'hôte.
- Vérifiez les soldes intermédiaires `FindAccountAssets` ou `iroha_cli ledger assets list --account <i105-account-id>` et inspectez les événements pour confirmer que la garde de métadonnées a enregistré le contexte du transfert.

## Guides relatifs au SDK

- [Démarrage rapide du SDK de Rust](/sdks/rust)
- [Démarrage rapide du SDK de Python](/sdks/python)
- [Démarrage rapide du SDK de JavaScript](/sdks/javascript)

[Télécharger la source de Kotodama](/norito-snippets/call-transfer-asset.ko)

```kotodama
// Direct builtin call (no raw call syntax) inside a seiyaku.
seiyaku TransferCall {
    kotoage fn pay() authorize("AssetTransferRole") {
        ledger::asset::transfer(
            source: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV", ),
            destination: AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76", ),
            asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
            amount: 10,
            dataspace: DataSpaceId::parse("0"),
        );
    }
}
```
