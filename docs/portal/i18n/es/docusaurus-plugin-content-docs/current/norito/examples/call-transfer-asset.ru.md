---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/call-transfer-asset
título: Вызвать перенос с хоста из Kotodama
descripción: Coloque el cable de alimentación Kotodama en la instalación del servidor `transfer_asset` con un controlador de metadano integrado.
fuente: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Coloque el cable de alimentación Kotodama en la instalación del servidor `transfer_asset` con un controlador de metadano integrado.

## Пошаговый обход реестра

- Пополните полномочия контракта (например `<i105-account-id>`) активом, который он будет переводить, и выдайте полномочию роль `CanTransfer` или эквивалентное разрешение.
- Utilice este vídeo `call_transfer_asset`, antes de 5 ediciones del contrato de cuenta con `<i105-account-id>`, además de eso. ончейн-автоматизация может оборачивать вызовы хоста.
- Guarde los equilibrios entre `FindAccountAssets` y `iroha_cli ledger assets list --account <i105-account-id>` y promueva la seguridad, la protección y la protección de metadanos. записал контекст перевода.

## Связанные руководства SDK

- [SDK de inicio rápido de Rust](/sdks/rust)
- [SDK de Python de inicio rápido](/sdks/python)
- [SDK de JavaScript de inicio rápido](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("<i105-account-id>"),
      account!("<i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```