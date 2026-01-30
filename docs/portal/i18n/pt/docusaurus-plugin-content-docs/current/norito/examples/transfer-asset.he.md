---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dc90249ee45cc5c8dc1f1cbdbf97647fe282258c4528ccb3b4d95d5a4bf9ff62
source_last_modified: "2025-11-14T04:43:20.811628+00:00"
translation_last_reviewed: 2026-01-30
---

Fluxo direto de transferencia de ativos que espelha os quickstarts do SDK e os roteiros do livro razao.

## Roteiro do livro razao

- Pre-financie Alice com o ativo alvo (por exemplo via o trecho `register and mint` ou os fluxos de quickstart do SDK).
- Execute o entrypoint `do_transfer` para mover 10 unidades de Alice para Bob, atendendo a permissao `AssetTransferRole`.
- Consulte saldos (`FindAccountAssets`, `iroha_cli ledger assets list`) ou assine eventos do pipeline para observar o resultado da transferencia.

## Guias de SDK relacionados

- [Quickstart do SDK Rust](/sdks/rust)
- [Quickstart do SDK Python](/sdks/python)
- [Quickstart do SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of rose#wonderland from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("ih58..."),
      account!("ih58..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```
