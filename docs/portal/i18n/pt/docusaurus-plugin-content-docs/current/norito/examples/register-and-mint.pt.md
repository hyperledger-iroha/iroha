---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/register-and-mint
título: Registrador de domínio e cunhador ativo
descrição: Demonstra a criação de domínios com permissão, o registro de ativos e a cunhagem determinística.
fonte: crates/ivm/docs/examples/13_register_and_mint.ko
---

Demonstra a criação de domínios com permissão, o registro de ativos e a cunhagem determinística.

## Roteiro do livro razão

- Garanta que a conta de destino (por exemplo `i105...`) exista, espelhando a fase de configuração em cada quickstart do SDK.
- Invoque o ponto de entrada `register_and_mint` para criar a definição do ativo ROSE e cunhar 250 unidades para Alice em uma transação única.
- Verifique os saldos via `client.request(FindAccountAssets)` ou `iroha_cli ledger assets list --account i105...` para confirmar que a cunhagem foi bem-sucedida.

## Guias de SDK relacionados

- [Início rápido do SDK Rust](/sdks/rust)
- [Início rápido do SDK Python](/sdks/python)
- [Início rápido do SDK JavaScript](/sdks/javascript)

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
    let to = account!("i105...");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```