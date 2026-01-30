---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
slug: /norito/examples/call-transfer-asset
title: Invocar transferencia del host desde Kotodama
description: Demuestra cómo un entrypoint de Kotodama puede llamar a la instrucción de host `transfer_asset` con validación de metadatos en línea.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Demuestra cómo un entrypoint de Kotodama puede llamar a la instrucción de host `transfer_asset` con validación de metadatos en línea.

## Recorrido del libro mayor

- Fondea la autoridad del contrato (por ejemplo `ih58...`) con el activo que transferirá y otórgale el rol `CanTransfer` o un permiso equivalente.
- Llama al entrypoint `call_transfer_asset` para transferir 5 unidades desde la cuenta del contrato a `ih58...`, reflejando la forma en que la automatización on-chain puede envolver llamadas del host.
- Verifica los balances mediante `FindAccountAssets` o `iroha_cli ledger assets list --account ih58...` e inspecciona los eventos para confirmar que la guardia de metadatos registró el contexto de la transferencia.

## Guías de SDK relacionadas

- [Quickstart del SDK de Rust](/sdks/rust)
- [Quickstart del SDK de Python](/sdks/python)
- [Quickstart del SDK de JavaScript](/sdks/javascript)

[Descarga la fuente de Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("ih58..."),
      account!("ih58..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```
