---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/transfer-asset
título: Transferir ativo entre contas
description: Fluxo direto de transferência de ativos que reflete os guias de início rápido do SDK e os corredores do livro prefeito.
fonte: exemplos/transfer/transfer.ko
---

Fluxo direto de transferência de ativos que reflete os inícios rápidos do SDK e os corredores do livro prefeito.

## Recorrido do livro prefeito

- Primeiro, funde Alice com o objetivo ativo (por exemplo, por meio do fragmento `register and mint` ou dos fluxos de início rápido do SDK).
- Execute o ponto de entrada `do_transfer` para mover 10 unidades de Alice para Bob, cumprindo a permissão `AssetTransferRole`.
- Consultar saldos (`FindAccountAssets`, `iroha_cli ledger assets list`) ou inscrever-se em eventos do pipeline para observar o resultado da transferência.

## Guias do SDK relacionados

- [Início rápido do SDK de Rust](/sdks/rust)
- [Início rápido do SDK de Python](/sdks/python)
- [Início rápido do SDK de JavaScript](/sdks/javascript)

[Descarregue a fonte de Kotodama](/norito-snippets/transfer-asset.ko)

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