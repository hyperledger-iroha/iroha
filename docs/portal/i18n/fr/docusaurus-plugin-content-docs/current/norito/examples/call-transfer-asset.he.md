---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ca547bc673684a35eaa1ba6d2d96616bcc453a0baa5ece4c8fd0a629deb1ca84
source_last_modified: "2026-01-22T15:55:01+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/call-transfer-asset
title: Invoquer le transfert hôte depuis Kotodama
description: Démontre comment un point d'entrée Kotodama peut appeler l'instruction hôte `transfer_asset` avec validation des métadonnées en ligne.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Démontre comment un point d'entrée Kotodama peut appeler l'instruction hôte `transfer_asset` avec validation des métadonnées en ligne.

## Parcours du registre

- Approvisionnez l'autorité du contrat (par exemple `<i105-account-id>`) avec l'actif qu'elle transférera et accordez-lui le rôle `CanTransfer` ou une permission équivalente.
- Appelez le point d'entrée `call_transfer_asset` pour transférer 5 unités du compte du contrat vers `<i105-account-id>`, en reflétant la manière dont l'automatisation on-chain peut encapsuler des appels hôte.
- Vérifiez les soldes via `FindAccountAssets` ou `iroha_cli ledger assets list --account <i105-account-id>` et inspectez les événements pour confirmer que le garde de métadonnées a journalisé le contexte du transfert.

## Guides SDK associés

- [Quickstart SDK Rust](/sdks/rust)
- [Quickstart SDK Python](/sdks/python)
- [Quickstart SDK JavaScript](/sdks/javascript)

[Télécharger la source Kotodama](/norito-snippets/call-transfer-asset.ko)

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
