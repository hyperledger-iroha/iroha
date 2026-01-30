---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8526c81175a83a1376dacb11d9928992f477a8324cbb530829369063adabf3fc
source_last_modified: "2025-11-14T04:43:20.811115+00:00"
translation_last_reviewed: 2026-01-30
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
