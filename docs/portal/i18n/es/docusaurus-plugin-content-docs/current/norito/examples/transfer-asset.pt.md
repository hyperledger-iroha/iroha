---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/ejemplos/transfer-asset
título: Transferir activo entre cuentas
descripción: Flujo directo de transferencia de activos que transmite los inicios rápidos del SDK y los rotores del libro razao.
fuente: ejemplos/transfer/transfer.ko
---

Flujo directo de transferencia de activos que incluye los inicios rápidos del SDK y los rotores del libro razao.

## Roteiro do livro razao

- Prefinanciación Alice con activo alvo (por ejemplo, a través del trecho `register and mint` o los flujos de inicio rápido del SDK).
- Ejecute el punto de entrada `do_transfer` para mover 10 unidades de Alice para Bob, atendiendo al permiso `AssetTransferRole`.
- Consulte saldos (`FindAccountAssets`, `iroha_cli ledger assets list`) o todos los eventos de la tubería para observar el resultado de la transferencia.

## Guías de SDK relacionadas

- [Inicio rápido de SDK Rust](/sdks/rust)
- [Inicio rápido del SDK Python](/sdks/python)
- [Inicio rápido del SDK JavaScript](/sdks/javascript)

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