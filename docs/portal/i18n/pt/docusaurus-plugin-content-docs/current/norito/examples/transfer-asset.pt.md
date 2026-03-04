---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/transfer-asset
título: Transferir ativo entre contas
description: Fluxo direto de transferência de ativos que reflete os quickstarts do SDK e os roteiros do livro razão.
fonte: exemplos/transfer/transfer.ko
---

Fluxo direto de transferência de ativos que reflete os quickstarts do SDK e os roteiros do livro razão.

## Roteiro do livro razão

- Pré-financie Alice com o alvo ativo (por exemplo via o trecho `register and mint` ou os fluxos de quickstart do SDK).
- Execute o ponto de entrada `do_transfer` para mover 10 unidades de Alice para Bob, atendendo a permissão `AssetTransferRole`.
- Consulte saldos (`FindAccountAssets`, `iroha_cli ledger assets list`) ou assine eventos do pipeline para observar o resultado da transferência.

## Guias de SDK relacionados

- [Início rápido do SDK Rust](/sdks/rust)
- [Início rápido do SDK Python](/sdks/python)
- [Início rápido do SDK JavaScript](/sdks/javascript)

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