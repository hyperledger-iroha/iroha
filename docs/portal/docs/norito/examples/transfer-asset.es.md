---
lang: es
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
      account!("i105..."),
      account!("i105..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```
