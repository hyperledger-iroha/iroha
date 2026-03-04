---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/examples/register-and-mint
título: Registrador de dominio y acuñar activos
descripción: Demuestra la creación de dominios con permisos, el registro de activos y la acuñación determinista.
fuente: crates/ivm/docs/examples/13_register_and_mint.ko
---

Demuestra la creación de dominios con permisos, el registro de activos y la acuñación determinista.

## Recorrido del libro mayor

- Asegúrese de que exista la cuenta de destino (por ejemplo `ih58...`), reflejando la fase de configuración en cada inicio rápido del SDK.
- Invoca el punto de entrada `register_and_mint` para crear la definición de activo ROSE y acuñar 250 unidades para Alice en una sola transacción.
- Verifica los saldos mediante `client.request(FindAccountAssets)` o `iroha_cli ledger assets list --account ih58...` para confirmar que la acuñación tuvo éxito.

## Guías de SDK relacionadas

- [Inicio rápido del SDK de Rust](/sdks/rust)
- [Inicio rápido del SDK de Python](/sdks/python)
- [Inicio rápido del SDK de JavaScript](/sdks/javascript)

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