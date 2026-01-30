---
lang: ar
direction: rtl
source: docs/portal/docs/norito/examples/transfer-asset.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
slug: /norito/examples/transfer-asset
title: Transferir activo entre cuentas
description: Flujo directo de transferencia de activos que refleja los quickstarts de los SDK y los recorridos del libro mayor.
source: examples/transfer/transfer.ko
---

Flujo directo de transferencia de activos que refleja los quickstarts de los SDK y los recorridos del libro mayor.

## Recorrido del libro mayor

- Pre-fondea a Alice con el activo objetivo (por ejemplo mediante el fragmento `register and mint` o los flujos de quickstart del SDK).
- Ejecuta el entrypoint `do_transfer` para mover 10 unidades de Alice a Bob, cumpliendo el permiso `AssetTransferRole`.
- Consulta balances (`FindAccountAssets`, `iroha_cli ledger assets list`) o suscríbete a eventos del pipeline para observar el resultado de la transferencia.

## Guías de SDK relacionadas

- [Quickstart del SDK de Rust](/sdks/rust)
- [Quickstart del SDK de Python](/sdks/python)
- [Quickstart del SDK de JavaScript](/sdks/javascript)

[Descarga la fuente de Kotodama](/norito-snippets/transfer-asset.ko)

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
