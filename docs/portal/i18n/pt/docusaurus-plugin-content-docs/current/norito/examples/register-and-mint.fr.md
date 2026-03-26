---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/register-and-mint
título: Registrar um domínio e frapper des actifs
descrição: Montar a criação de domínios com autorizações, registro de atividades e frappe déterministe.
fonte: crates/ivm/docs/examples/13_register_and_mint.ko
---

Implemente a criação de domínios com autorizações, o registro de atividades e o frappe déterminista.

## Parcours du registre

- Certifique-se de que a conta de destino (por exemplo, `soraカタカナ...`) existe, refletindo a fase de mise no local em cada SDK de início rápido.
- Invoque o ponto de entrada `register_and_mint` para criar a definição do ativo ROSE e frapper 250 unidades para Alice em uma única transação.
- Verifique as soldas via `client.request(FindAccountAssets)` ou `iroha_cli ledger assets list --account soraカタカナ...` para confirmar se o frappe está reusado.

## Guias SDK associados

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
    let to = account!("soraカタカナ...");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```