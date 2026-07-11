---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/register-and-mint
title: Enregistrer un domaine et frapper des actifs
description: Démontre la création de domaines avec autorisations, l'enregistrement d'actifs et la frappe déterministe.
source: crates/ivm/docs/examples/13_register_and_mint.ko
---

Démontre la création de domaines avec autorisations, l'enregistrement d'actifs et la frappe déterministe.

## Parcours du registre

- Assurez-vous que le compte de destination (par exemple `<i105-account-id>`) existe, en reflétant la phase de mise en place dans chaque quickstart SDK.
- Invoquez le point d'entrée `register_and_mint` pour créer la définition d'actif ROSE et frapper 250 unités pour Alice en une seule transaction.
- Vérifiez les soldes via `client.request(FindAccountAssets)` ou `iroha_cli ledger assets list --account <i105-account-id>` pour confirmer que la frappe a réussi.

## Guides SDK associés

- [Quickstart SDK Rust](/sdks/rust)
- [Quickstart SDK Python](/sdks/python)
- [Quickstart SDK JavaScript](/sdks/javascript)

[Télécharger la source Kotodama](/norito-snippets/register-and-mint.ko)

```kotodama
// Register a new asset and mint some to the specified account.
seiyaku RegisterAndMint {
    kotoage fn register_and_mint() authorize("AssetManager") {
        // name, symbol, quantity (precision or supply depending on host), mintable flag
        let asset = AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
        let symbol = "ROSE";
        let qty = 1000;
        // interpretation depends on data model (example only)
        let mintable = 1;
        // 1 = mintable, 0 = fixed
        ledger::asset::register(
            asset_definition: asset,
            name: symbol,
            scale: qty,
            mintable: mintable,
        );
        // Mint 250 ROSE to Alice
        let to = AccountId::parse(
            "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
        );
        ledger::asset::mint(account: to, asset_definition: asset, amount: 250);
    }
}
```
