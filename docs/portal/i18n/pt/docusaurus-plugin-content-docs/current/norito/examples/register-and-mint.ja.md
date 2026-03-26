---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 77321a3ff75e7eca71874304ccc365d2b272ab038784c3fa34b6425f3b0660c2
source_last_modified: "2026-01-22T15:55:01+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/register-and-mint
title: Registrar dominio e cunhar ativos
description: Demonstra a criacao de dominios com permissao, o registro de ativos e a cunhagem deterministica.
source: crates/ivm/docs/examples/13_register_and_mint.ko
---

Demonstra a criacao de dominios com permissao, o registro de ativos e a cunhagem deterministica.

## Roteiro do livro razao

- Garanta que a conta de destino (por exemplo `<i105-account-id>`) exista, espelhando a fase de configuracao em cada quickstart do SDK.
- Invoque o entrypoint `register_and_mint` para criar a definicao do ativo ROSE e cunhar 250 unidades para Alice em uma unica transacao.
- Verifique os saldos via `client.request(FindAccountAssets)` ou `iroha_cli ledger assets list --account <i105-account-id>` para confirmar que a cunhagem foi bem-sucedida.

## Guias de SDK relacionados

- [Quickstart do SDK Rust](/sdks/rust)
- [Quickstart do SDK Python](/sdks/python)
- [Quickstart do SDK JavaScript](/sdks/javascript)

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
    let to = account!("<i105-account-id>");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```
