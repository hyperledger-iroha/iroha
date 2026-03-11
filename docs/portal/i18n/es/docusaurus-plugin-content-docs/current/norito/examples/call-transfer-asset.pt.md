---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/call-transfer-asset
título: Invocar transferencia do host a partir de Kotodama
descripción: Mostra como un punto de entrada Kotodama puede marcar las instrucciones del host `transfer_asset` con validación en línea de metadados.
fuente: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Mostra como un punto de entrada Kotodama puede seguir las instrucciones del host `transfer_asset` con validación de metadados en línea.

## Roteiro do livro razao

- Financie a autoridade do contrato (por ejemplo `i105...`) con o activo que ela transferira e conceda a autoridade o papel `CanTransfer` o permiso equivalente.
- Chame o Entrypoint `call_transfer_asset` para transferir 5 unidades del contacto del contrato para `i105...`, reflejando como un automático en cadena que puede involucrar chamadas do host.
- Verifique saldos vía `FindAccountAssets` o `iroha_cli ledger assets list --account i105...` e inspeccione eventos para confirmar que o guard de metadados registrou o contexto da transferencia.

## Guías de SDK relacionadas

- [Inicio rápido de SDK Rust](/sdks/rust)
- [Inicio rápido del SDK Python](/sdks/python)
- [Inicio rápido del SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("i105..."),
      account!("i105..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```