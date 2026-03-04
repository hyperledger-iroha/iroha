---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/examples/register-and-mint
título: Registrar un dominio y frapper des actifs
descripción: Démontre la creación de dominios con autorizaciones, el registro de activos y el frappe determinado.
fuente: crates/ivm/docs/examples/13_register_and_mint.ko
---

Démontre la creación de dominios con autorizaciones, el registro de activos y el frappe determinado.

## Rutas del registro

- Asegúrese de que la cuenta de destino (por ejemplo, `ih58...`) exista, reflejando la fase de instalación en cada SDK de inicio rápido.
- Invoquez le point d'entrée `register_and_mint` pour créer la definición de actif ROSE et frapper 250 unités pour Alice en una única transacción.
- Verifique las ventas a través de `client.request(FindAccountAssets)` o `iroha_cli ledger assets list --account ih58...` para confirmar que el frappe a réussi.

## Guías SDK asociadas

- [Inicio rápido SDK Rust](/sdks/rust)
- [Inicio rápido SDK Python](/sdks/python)
- [Inicio rápido SDK JavaScript](/sdks/javascript)

[Descargar la fuente Kotodama](/norito-snippets/register-and-mint.ko)

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