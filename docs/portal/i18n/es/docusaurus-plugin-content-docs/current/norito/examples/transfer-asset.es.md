---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/ejemplos/transfer-asset
título: Transferir activo entre cuentas
descripción: Flujo directo de transferencia de activos que refleja los inicios rápidos de los SDK y los recorridos del libro mayor.
fuente: ejemplos/transfer/transfer.ko
---

Flujo directo de transferencia de activos que refleja los inicios rápidos de los SDK y los recorridos del libro mayor.

## Recorrido del libro mayor

- Pre-fondea a Alice con el activo objetivo (por ejemplo mediante el fragmento `register and mint` o los flujos de inicio rápido del SDK).
- Ejecuta el punto de entrada `do_transfer` para mover 10 unidades de Alice a Bob, cumpliendo con el permiso `AssetTransferRole`.
- Consulta saldos (`FindAccountAssets`, `iroha_cli ledger assets list`) o suscríbete a eventos del pipeline para observar el resultado de la transferencia.

## Guías de SDK relacionadas

- [Inicio rápido del SDK de Rust](/sdks/rust)
- [Inicio rápido del SDK de Python](/sdks/python)
- [Inicio rápido del SDK de JavaScript](/sdks/javascript)

[Descarga la fuente de Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of 62Fk4FPcMuLvW5QjDGNF2a4jAmjM from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("i105..."),
      account!("i105..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```