---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b066f95804af834930008b4a7c654778f32f6467bcecbdee47d09997cbd35122
source_last_modified: "2025-11-09T11:46:26.108135+00:00"
translation_last_reviewed: 2026-01-30
---

---
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
      account!("soraカタカナ..."),
      account!("soraカタカナ..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```
