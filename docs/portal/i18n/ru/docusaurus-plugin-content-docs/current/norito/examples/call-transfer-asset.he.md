---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 85d38764425843d13b1fe0fdb9b2b85cf04580fff2848210d57d570236210bdb
source_last_modified: "2026-01-22T15:55:00+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/call-transfer-asset
title: Вызвать перенос с хоста из Kotodama
description: Показывает, как точка входа Kotodama может вызвать инструкцию хоста `transfer_asset` с встроенной проверкой метаданных.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Показывает, как точка входа Kotodama может вызвать инструкцию хоста `transfer_asset` с встроенной проверкой метаданных.

## Пошаговый обход реестра

- Пополните полномочия контракта (например `i105...`) активом, который он будет переводить, и выдайте полномочию роль `CanTransfer` или эквивалентное разрешение.
- Вызовите точку входа `call_transfer_asset`, чтобы перевести 5 единиц с аккаунта контракта на `i105...`, отражая то, как ончейн-автоматизация может оборачивать вызовы хоста.
- Проверьте балансы через `FindAccountAssets` или `iroha_cli ledger assets list --account i105...` и просмотрите события, чтобы подтвердить, что guard метаданных записал контекст перевода.

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
      account!("i105..."),
      account!("i105..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```
