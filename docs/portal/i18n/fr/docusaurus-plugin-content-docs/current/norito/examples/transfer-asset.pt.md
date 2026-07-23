---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug : /norito/examples/transfer-asset
titre : Transférer ativo entre contas
description : Flux direct de transfert d'actifs qui exécutent les démarrages rapides du SDK et les rotations du livre razao.
source : exemples/transfert/transfer.ko
---

Flux direct de transfert des activités qui exécutent les démarrages rapides du SDK et les rotations du livre razao.

## Roteiro do livro razão

- Pré-financer Alice avec ou d'autre (par exemple via le trecho `register and mint` ou les flux de démarrage rapide du SDK).
- Exécutez le point d'entrée `do_transfer` pour déplacer 10 unités d'Alice pour Bob, en attendant l'autorisation `AssetTransferRole`.
- Consultez les saldos (`FindAccountAssets`, `iroha_cli ledger assets list`) ou assistez aux événements du pipeline pour observer le résultat du transfert.

## Guides des utilisateurs du SDK

- [Démarrage rapide du SDK Rust](/sdks/rust)
- [Démarrage rapide du SDK Python](/sdks/python)
- [Démarrage rapide du SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/transfer-asset.ko)

```kotodama
// Transfer example: uses typed pointer constructors and transfer_asset syscall
seiyaku TransferDemo {
    // Public kotoage declaration to transfer 10 units of the canonical Base58 asset definition between canonical I105 accounts
    kotoage fn do_transfer() authorize("AssetTransferRole") {
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
