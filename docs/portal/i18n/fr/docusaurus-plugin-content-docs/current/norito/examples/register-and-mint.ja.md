---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 769c27dec1cfa2212bb0b9bbd709c7f843a78fa2b72166fd8d20b5b35ed6dd10
source_last_modified: "2026-01-22T15:55:01+00:00"
translation_last_reviewed: 2026-01-30
---


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

- Assurez-vous que le compte de destination (par exemple `ih58...`) existe, en reflétant la phase de mise en place dans chaque quickstart SDK.
- Invoquez le point d'entrée `register_and_mint` pour créer la définition d'actif ROSE et frapper 250 unités pour Alice en une seule transaction.
- Vérifiez les soldes via `client.request(FindAccountAssets)` ou `iroha_cli ledger assets list --account ih58...` pour confirmer que la frappe a réussi.

## Guides SDK associés

- [Quickstart SDK Rust](/sdks/rust)
- [Quickstart SDK Python](/sdks/python)
- [Quickstart SDK JavaScript](/sdks/javascript)

[Télécharger la source Kotodama](/norito-snippets/register-and-mint.ko)

```text
// Register a new asset and mint some to the specified account.
seiyaku RegisterAndMint {
  kotoage fn register_and_mint() permission(AssetManager) {
    // name, symbol, quantity (precision or supply depending on host), mintable flag
    let name = "rose";
    let symbol = "ROSE";
    let qty = 1000;      // interpretation depends on data model (example only)
    let mintable = 1;    // 1 = mintable, 0 = fixed
    register_asset(name, symbol, qty, mintable);

    // Mint 250 ROSE to Alice
    let to = account!("ih58...");
    let asset = asset_definition!("rose#wonderland");
    mint_asset(to, asset, 250);
  }
}
```
