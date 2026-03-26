---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/ejemplos/transfer-asset
título: Transferer un actif entre cuentas
Descripción: Flujo de transferencia de acciones simple que refleja los inicios rápidos del SDK y los recorridos del registro.
fuente: ejemplos/transfer/transfer.ko
---

El flujo de transferencia de acciones es simple y refleja los inicios rápidos del SDK y los recorridos del registro.

## Rutas del registro

- Prefinancez Alice con el actif cible (por ejemplo a través del fragmento `register and mint` o el flujo de inicio rápido SDK).
- Ejecute el punto de entrada `do_transfer` para colocar 10 unidades de Alice frente a Bob, y cumpla con el permiso `AssetTransferRole`.
- Interrogue las soldaduras (`FindAccountAssets`, `iroha_cli ledger assets list`) o abra los eventos de la tubería para observar el resultado de la transferencia.

## Guías SDK asociadas

- [Inicio rápido SDK Rust](/sdks/rust)
- [Inicio rápido SDK Python](/sdks/python)
- [Inicio rápido SDK JavaScript](/sdks/javascript)

[Descargar la fuente Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of 62Fk4FPcMuLvW5QjDGNF2a4jAmjM from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("soraカタカナ..."),
      account!("soraカタカナ..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```