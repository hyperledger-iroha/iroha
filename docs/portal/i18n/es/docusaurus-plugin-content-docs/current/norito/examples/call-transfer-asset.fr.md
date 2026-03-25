---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/call-transfer-asset
título: Invoquer le transfert hôte depuis Kotodama
Descripción: Démontre comment un punto de entrada Kotodama puede apelar a la instrucción de inicio `transfer_asset` con validación de metadonnées en línea.
fuente: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Démontre comment un punto de entrada Kotodama puede llamar a la instrucción de inicio `transfer_asset` con la validación de los metadonnées en línea.

## Rutas del registro

- Approvisionnez l'autorité du contrat (por ejemplo `i105...`) avec l'actif qu'elle transférera et Accordez-lui le rôle `CanTransfer` ou un permiso équivalente.
- Toque el punto de entrada `call_transfer_asset` para transferir 5 unidades de la cuenta del contrato frente a `i105...`, para reflejar la manera en que la automatización en cadena puede encapsular las llamadas telefónicas.
- Verifique las ventas a través de `FindAccountAssets` o `iroha_cli ledger assets list --account i105...` e inspeccione los eventos para confirmar que la guardia de métadonnées a periodisé le contexte du transfert.

## Guías SDK asociadas

- [Inicio rápido SDK Rust](/sdks/rust)
- [Inicio rápido SDK Python](/sdks/python)
- [Inicio rápido SDK JavaScript](/sdks/javascript)

[Descargar la fuente Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("i105..."),
      account!("i105..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```