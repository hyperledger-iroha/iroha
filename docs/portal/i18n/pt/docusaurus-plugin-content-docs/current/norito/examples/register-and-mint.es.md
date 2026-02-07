---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/register-and-mint
título: Registrador de domínio e aquisição de ativos
descrição: Demonstra a criação de domínios com permissões, o registro de ativos e a acuñación determinista.
fonte: crates/ivm/docs/examples/13_register_and_mint.ko
---

Demonstra a criação de domínios com permissões, o registro de ativos e a acumulação determinista.

## Recorrido do livro prefeito

- Certifique-se de que existe a conta de destino (por exemplo, `ih58...`), refletindo a fase de configuração em cada início rápido do SDK.
- Invoque o ponto de entrada `register_and_mint` para criar a definição de ativo ROSE e acumular 250 unidades para Alice em uma única transação.
- Verifique os saldos através de `client.request(FindAccountAssets)` ou `iroha_cli ledger assets list --account ih58...` para confirmar que sua acuñación foi exitosa.

## Guias do SDK relacionados

- [Início rápido do SDK de Rust](/sdks/rust)
- [Início rápido do SDK de Python](/sdks/python)
- [Início rápido do SDK de JavaScript](/sdks/javascript)

[Descarregue a fonte de Kotodama](/norito-snippets/register-and-mint.ko)

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