---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a91fc8841580a836c80129942df7f79f5bc5dd5f6a72dccf1394b740d02536a5
source_last_modified: "2025-11-23T15:30:33.687233+00:00"
translation_last_reviewed: 2026-01-30
---

---
slug: /norito/examples/call-transfer-asset
title: Вызвать перенос с хоста из Kotodama
description: Показывает, как точка входа Kotodama может вызвать инструкцию хоста `transfer_asset` с встроенной проверкой метаданных.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Показывает, как точка входа Kotodama может вызвать инструкцию хоста `transfer_asset` с встроенной проверкой метаданных.

## Пошаговый обход реестра

- Пополните полномочия контракта (например `ih58...`) активом, который он будет переводить, и выдайте полномочию роль `CanTransfer` или эквивалентное разрешение.
- Вызовите точку входа `call_transfer_asset`, чтобы перевести 5 единиц с аккаунта контракта на `ih58...`, отражая то, как ончейн-автоматизация может оборачивать вызовы хоста.
- Проверьте балансы через `FindAccountAssets` или `iroha_cli ledger assets list --account ih58...` и просмотрите события, чтобы подтвердить, что guard метаданных записал контекст перевода.

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
      account!("ih58..."),
      account!("ih58..."),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```
