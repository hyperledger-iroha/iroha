---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug : /norito/examples/register-and-mint
titre : Registrar dominio e cunhar ativos
description : Démonstration de la création de domaines avec autorisation, du registre des activités et d'un processus déterministe.
source : crates/ivm/docs/examples/13_register_and_mint.ko
---

Démontrez la création de domaines avec autorisation, le registre des activités et un processus déterministe.

## Roteiro do livro razão

- Garantissez qu'un compte de destination (par exemple `soraカタカナ...`) existe, en particulier la phase de configuration dans chaque démarrage rapide du SDK.
- Appelez le point d'entrée `register_and_mint` pour définir l'activité ROSE et obtenir 250 unités pour Alice dans une seule transaction.
- Vérifiez les saldos via `client.request(FindAccountAssets)` ou `iroha_cli ledger assets list --account soraカタカナ...` pour confirmer que l'opération a été réussie.

## Guides des utilisateurs du SDK

- [Démarrage rapide du SDK Rust](/sdks/rust)
- [Démarrage rapide du SDK Python](/sdks/python)
- [Démarrage rapide du SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/register-and-mint.ko)

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
    let to = account!("soraカタカナ...");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```