---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0541f1f5775744c518f4f102326e725d73043b1756bb62a979f8eed4cc9472e6
source_last_modified: "2026-04-08T09:19:38.795296+00:00"
translation_last_reviewed: 2026-04-08
---

---
slug: /norito/examples/transfer-asset
title: Transférer un actif entre comptes
description: Flux de transfert d'actifs simple qui reflète les quickstarts SDK et les parcours du registre.
source: examples/transfer/transfer.ko
---

Flux de transfert d'actifs simple qui reflète les quickstarts SDK et les parcours du registre.

## Parcours du registre

- Préfinancez Alice avec l'actif cible (par exemple via le snippet `register and mint` ou les flux de quickstart SDK).
- Exécutez le point d'entrée `do_transfer` pour déplacer 10 unités d'Alice vers Bob, en satisfaisant la permission `AssetTransferRole`.
- Interrogez les soldes (`FindAccountAssets`, `iroha ledger asset list all --verbose`) ou abonnez-vous aux événements du pipeline pour observer le résultat du transfert.

## Guides SDK associés

- [Quickstart SDK Rust](/sdks/rust)
- [Quickstart SDK Python](/sdks/python)
- [Quickstart SDK JavaScript](/sdks/javascript)

[Télécharger la source Kotodama](/norito-snippets/transfer-asset.ko)

```kotodama
// Transfer example: uses typed pointer constructors and transfer_asset syscall
seiyaku TransferDemo {
    // Public entrypoint to transfer 10 units of the canonical Base58 asset definition between canonical I105 accounts
    kotoage fn do_transfer() authorize("AssetTransferRole") {
        ledger::asset::transfer(
            source: AccountId::parse(
                "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            ),
            destination: AccountId::parse(
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
            ),
            asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
            amount: 10,
            dataspace: DataSpaceId::parse("0"),
        );
    }
}
```
