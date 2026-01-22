<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a91fc8841580a836c80129942df7f79f5bc5dd5f6a72dccf1394b740d02536a5
source_last_modified: "2025-11-23T15:30:33.687233+00:00"
translation_last_reviewed: 2025-12-30
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
