---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/transfer-asset
título: Перевести актив между аккаунтами
description: Este cenário está ativado, usando o SDK de início rápido e a restauração do passo a passo.
fonte: exemplos/transfer/transfer.ko
---

No cenário de ativação ativa, consulte o SDK de início rápido e a restauração do passo a passo.

## Пошаговый обход реестра

- Verifique se Alice está ativa (configure o trecho `register and mint` ou use o SDK de início rápido).
- Выполните точку входа `do_transfer`, чтобы перевести 10 единиц от Alice к Bob, удовлетворяя разрешению `AssetTransferRole`.
- Prover balanceamentos (`FindAccountAssets`, `iroha_cli ledger assets list`) ou ajustar o pipeline, obter o resultado desejado primeiro.

## Como usar o SDK

- [Início rápido Rust SDK](/sdks/rust)
- [Início rápido do SDK do Python](/sdks/python)
- [SDK JavaScript de início rápido](/sdks/javascript)

[Kotodama](/norito-snippets/transfer-asset.ko)

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