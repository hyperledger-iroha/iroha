---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0fed4a2a1427064a9d94d0fd08cd223f7e75fe3394d662f5716fd6b1405543d
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/transfer-asset
title: Transferir ativo entre contas
description: Fluxo direto de transferencia de ativos que espelha os quickstarts do SDK e os roteiros do livro razao.
source: examples/transfer/transfer.ko
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
  // Public entrypoint to transfer 10 units of 62Fk4FPcMuLvW5QjDGNF2a4jAmjM from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("<katakana-i105-account-id>"),
      account!("<katakana-i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```
