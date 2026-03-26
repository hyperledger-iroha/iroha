---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/call-transfer-asset
כותרת: Invocar transferencia del host desde Kotodama
תיאור: Demuestra cómo un entrypoint de Kotodama puede llamar a la instrucción de host `transfer_asset` con validación de metadatos in linea.
מקור: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Demuestra cómo un entrypoint de Kotodama puede llamar a la instrucción de host `transfer_asset` con validación de metadatos in linea.

## ראש העיר Recorrido del libro

- Fondea la autoridad del contrato (por ejemplo `soraカタカナ...`) con el activo que transferirá y otórgale el roll `CanTransfer` o un permiso equivalente.
- Llama al entrypoint `call_transfer_asset` עבור העברה של 5 יחידות בעלות קואנטה דל קונטרה א `soraカタカナ...`.
- Verifica los balances mediaante `FindAccountAssets` o `iroha_cli ledger assets list --account soraカタカナ...` e inspecciona los eventos para confirmar que la guardia de metadatos registró el contexto de la transferencia.

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
      account!("soraカタカナ..."),
      account!("soraカタカナ..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```