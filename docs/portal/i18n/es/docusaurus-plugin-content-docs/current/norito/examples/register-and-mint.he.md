---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 080ad72682ecd7a36331df33e86fbe4dc39d5ec0bed1da89872db6a60a312a53
source_last_modified: "2025-11-14T04:43:20.790675+00:00"
translation_last_reviewed: 2026-01-30
---

Demuestra la creación de dominios con permisos, el registro de activos y la acuñación determinista.

## Recorrido del libro mayor

- Asegúrate de que exista la cuenta de destino (por ejemplo `ih58...`), reflejando la fase de configuración en cada quickstart del SDK.
- Invoca el entrypoint `register_and_mint` para crear la definición de activo ROSE y acuñar 250 unidades para Alice en una sola transacción.
- Verifica los balances mediante `client.request(FindAccountAssets)` o `iroha_cli ledger assets list --account ih58...` para confirmar que la acuñación tuvo éxito.

## Guías de SDK relacionadas

- [Quickstart del SDK de Rust](/sdks/rust)
- [Quickstart del SDK de Python](/sdks/python)
- [Quickstart del SDK de JavaScript](/sdks/javascript)

[Descarga la fuente de Kotodama](/norito-snippets/register-and-mint.ko)

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
