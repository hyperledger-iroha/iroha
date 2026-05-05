---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d7cdc798ae9f1b69608c2b287a76a06a3d8d51116185ff6a5326e755b7b05bd
source_last_modified: "2026-04-08T09:19:38.794575+00:00"
translation_last_reviewed: 2026-04-08
---

---
slug: /norito/examples/call-transfer-asset
title: Вызвать перенос с хоста из Kotodama
description: Показывает, как точка входа Kotodama может вызвать инструкцию хоста `transfer_asset` с встроенной проверкой метаданных.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Показывает, как точка входа Kotodama может вызвать инструкцию хоста `transfer_asset` с встроенной проверкой метаданных.

## Пошаговый обход реестра

- Пополните полномочия контракта (например `<i105-account-id>`) активом, который он будет переводить, и выдайте полномочию роль `CanTransfer` или эквивалентное разрешение.
- Вызовите точку входа `call_transfer_asset`, чтобы перевести 5 единиц с аккаунта контракта на `<i105-account-id>`, отражая то, как ончейн-автоматизация может оборачивать вызовы хоста.
- Проверьте балансы через `FindAccountAssets` или `iroha ledger asset list all --verbose` и просмотрите события, чтобы подтвердить, что guard метаданных записал контекст перевода.

## Связанные руководства SDK

- [Quickstart Rust SDK](/sdks/rust)
- [Quickstart Python SDK](/sdks/python)
- [Quickstart JavaScript SDK](/sdks/javascript)

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
