---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c30c710be94cd99f3c7a0484040155bf63ff4dc0d464d76237bddc8bf589ef26
source_last_modified: "2025-11-07T11:59:47.168250+00:00"
translation_last_reviewed: 2026-01-30
---

---
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
    let to = account!("<i105-account-id>");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```
