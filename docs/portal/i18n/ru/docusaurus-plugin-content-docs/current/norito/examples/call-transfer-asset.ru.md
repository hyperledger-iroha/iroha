---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
пул: /norito/examples/call-transfer-asset
title: Вы называете перенос с хоста из Kotodama
описание: Показывает, как точка входа Kotodama может вызвать хост-программу `transfer_asset` со встроенной проверкой метаданных.
источник: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Показывает, как точка входа Kotodama может вызвать команду хоста `transfer_asset` со встроенной проверкой метаданных.

## Пошаговый обход реестра

- Пополните полномочия контракта (например, `soraカタカナ...`) активом, который он будет переводить, и выдайте полную роль `CanTransfer` или эквивалентное решение.
- Вызовите точку входа `call_transfer_asset`, чтобы перевести 5 единиц с контрактом аккаунта на `soraカタカナ...`, учитывая, что ончейн-автоматизация может оборачивать вызовы хоста.
- Проверьте балансы через `FindAccountAssets` или `iroha_cli ledger assets list --account soraカタカナ...` и просмотрите события, чтобы убедиться, что защита метаданных записала преобразование контекста.

## Связанные управления SDK

- [Быстрый запуск Rust SDK](/sdks/rust)
- [Быстрый запуск Python SDK] (/sdks/python)
- [Quickstart JavaScript SDK](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/call-transfer-asset.ko)

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