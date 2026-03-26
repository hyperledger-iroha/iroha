---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/register-and-mint
title: Registrar dominio y acuñar activos
description: Demuestra la creación de dominios con permisos, el registro de activos y la acuñación determinista.
source: crates/ivm/docs/examples/13_register_and_mint.ko
---

Demuestra la creación de dominios con permisos, el registro de activos y la acuñación determinista.

## Recorrido del libro mayor

- Asegúrate de que exista la cuenta de destino (por ejemplo `soraカタカナ...`), reflejando la fase de configuración en cada quickstart del SDK.
- Invoca el entrypoint `register_and_mint` para crear la definición de activo ROSE y acuñar 250 unidades para Alice en una sola transacción.
- Verifica los balances mediante `client.request(FindAccountAssets)` o `iroha_cli ledger assets list --account soraカタカナ...` para confirmar que la acuñación tuvo éxito.

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
    let to = account!("soraカタカナ...");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```
