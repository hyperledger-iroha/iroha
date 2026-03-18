---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/transfer-asset
título: Transferir um ato entre contas
description: Fluxo de transferência de atividades simples que reflete o SDK de início rápido e as etapas de registro.
fonte: exemplos/transfer/transfer.ko
---

Fluxo de transferência de atividades simples que reflete o SDK de início rápido e os pacotes de registro.

## Parcours du registre

- Pré-financiar Alice com o ativo (por exemplo, por meio do snippet `register and mint` ou do fluxo do SDK de início rápido).
- Execute o ponto de entrada `do_transfer` para substituir 10 unidades de Alice de Bob, satisfazendo a permissão `AssetTransferRole`.
- Interrogue as soldas (`FindAccountAssets`, `iroha_cli ledger assets list`) ou conecte-se a eventos do pipeline para observar o resultado da transferência.

## Guias SDK associados

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
      account!("i105..."),
      account!("i105..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```