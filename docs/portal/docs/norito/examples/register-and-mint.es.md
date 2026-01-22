<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c30c710be94cd99f3c7a0484040155bf63ff4dc0d464d76237bddc8bf589ef26
source_last_modified: "2025-11-07T11:59:47.168250+00:00"
translation_last_reviewed: 2025-12-30
---

---
slug: /norito/examples/register-and-mint
title: Registrar dominio y acuñar activos
description: Demuestra la creación de dominios con permisos, el registro de activos y la acuñación determinista.
source: crates/ivm/docs/examples/13_register_and_mint.ko
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
