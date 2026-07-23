---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
babosa: /norito/examples/register-and-mint
título: Registrador de dominio y cunhar ativos
descripción: Demostrar una criacao de dominios com permissao, o registro de ativos e a cunhagem deterministica.
fuente: crates/ivm/docs/examples/13_register_and_mint.ko
---

Demonstrar a criacao de dominios com permissao, o registro de ativos e a cunhagem deterministica.

## Roteiro do livro razao

- Garantía de que existe una cuenta de destino (por ejemplo, `<i105-account-id>`), activando una fase de configuración en cada inicio rápido del SDK.
- Invoque el punto de entrada `register_and_mint` para crear la definición del activo ROSE y cunhar 250 unidades para Alice en una única transacao.
- Verifique os saldos vía `client.request(FindAccountAssets)` o `iroha_cli ledger assets list --account <i105-account-id>` para confirmar que a cunhagem foi bem-sucedida.

## Guías de SDK relacionadas

- [Inicio rápido de SDK Rust](/sdks/rust)
- [Inicio rápido del SDK Python](/sdks/python)
- [Inicio rápido del SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/register-and-mint.ko)

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
        let to = AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV", );
        ledger::asset::mint(account: to, asset_definition: asset, amount: 250);
    }
}
```
